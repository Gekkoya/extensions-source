use std::cell::RefCell;
use std::collections::BTreeSet;

use gekkoya_lib::crypto::*;
use gekkoya_lib::deobfuscator::{deobfuscate_script as deobfuscate_js_script, extract_variable};
use gekkoya_lib::html_parser::*;
use mmrcms::{MMRCMSConfig, MMRCMS};
use serde::Deserialize;

// Generate WIT bindings
wit_bindgen::generate!({
    world: "extension",
    path: "../../../core/wit",
});

// The bindings are generated under ikisaku::gekkoya_extension
use ikisaku::gekkoya_extension::{host_tools, types};

// Conversion helpers between mmrcms types and WIT types
fn convert_manga_status(status: mmrcms::MangaStatus) -> types::MangaStatus {
    match status {
        mmrcms::MangaStatus::Unknown => types::MangaStatus::Unknown,
        mmrcms::MangaStatus::Ongoing => types::MangaStatus::Ongoing,
        mmrcms::MangaStatus::Completed => types::MangaStatus::Completed,
        mmrcms::MangaStatus::Licensed => types::MangaStatus::Licensed,
        mmrcms::MangaStatus::PublishingFinished => types::MangaStatus::PublishingFinished,
        mmrcms::MangaStatus::Cancelled => types::MangaStatus::Cancelled,
        mmrcms::MangaStatus::OnHiatus => types::MangaStatus::OnHiatus,
    }
}

fn convert_update_strategy(strategy: mmrcms::UpdateStrategy) -> types::UpdateStrategy {
    match strategy {
        mmrcms::UpdateStrategy::AlwaysUpdate => types::UpdateStrategy::AlwaysUpdate,
        mmrcms::UpdateStrategy::OnlyFetchOnce => types::UpdateStrategy::OnlyFetchOnce,
    }
}

fn convert_manga(manga: mmrcms::Manga) -> types::Manga {
    types::Manga {
        url: manga.url,
        title: manga.title,
        artist: manga.artist,
        author: manga.author,
        description: manga.description,
        genre: manga.genre,
        status: convert_manga_status(manga.status),
        thumbnail_url: manga.thumbnail_url,
        update_strategy: convert_update_strategy(manga.update_strategy),
        initialized: manga.initialized,
    }
}

fn convert_chapter(chapter: mmrcms::Chapter) -> types::Chapter {
    types::Chapter {
        url: chapter.url,
        name: chapter.name,
        date_upload: chapter.date_upload,
        chapter_number: chapter.chapter_number,
        scanlator: chapter.scanlator,
    }
}

fn convert_filter_option(option: mmrcms::FilterOption) -> types::FilterOption {
    types::FilterOption {
        name: option.name,
        value: option.value,
    }
}

fn convert_filter(filter: mmrcms::Filter) -> types::Filter {
    types::Filter {
        filter_type: filter.filter_type,
        name: filter.name,
        key: filter.key,
        options: filter
            .options
            .map(|opts| opts.into_iter().map(convert_filter_option).collect()),
    }
}

/// Encrypted chapter data structure from MangasIn
#[derive(Deserialize)]
struct EncryptedChapterData {
    ct: String,
    s: String,
}

/// Chapter data from decrypted JSON
#[derive(Deserialize)]
struct ChapterData {
    slug: String,
    name: String,
    number: String,
    #[serde(rename = "created_at")]
    created_at: String,
}

/// Latest manga response from API
#[derive(Deserialize)]
struct LatestManga {
    #[serde(rename = "manga_name")]
    name: String,
    #[serde(rename = "manga_slug")]
    slug: String,
}

#[derive(Deserialize)]
struct LatestUpdateResponse {
    data: Vec<LatestManga>,
    #[serde(rename = "totalPages")]
    total_pages: u32,
}

/// Search suggestion from API
#[derive(Deserialize)]
struct SuggestionDto {
    value: String, // Title
    data: String,  // Slug
}

/// MangasIn extension implementation
struct MangasIn {
    mmrcms: MMRCMS,
    base_url: String,
    /// Cached AES key to avoid fetching ads2.js multiple times
    cached_aes_key: RefCell<Option<String>>,
    /// Cache for latest updates deduplication (like Kotlin's latestTitles)
    latest_titles: RefCell<BTreeSet<String>>,
}

impl MangasIn {
    fn new() -> Self {
        let base_url = "https://m440.in".to_string();
        let config = MMRCMSConfig::new(base_url.clone(), mmrcms::Language::Spanish);

        Self {
            mmrcms: MMRCMS::new(config),
            base_url,
            cached_aes_key: RefCell::new(None),
            latest_titles: RefCell::new(BTreeSet::new()),
        }
    }

    /// Parse latest updates JSON response
    /// Implements deduplication like Kotlin version
    fn parse_latest_json(&self, body: &str, page: u32) -> Result<types::MangaPage, String> {
        let response: LatestUpdateResponse =
            serde_json::from_str(body).map_err(|_| "Failed to parse latest JSON".to_string())?;

        // Clear cache on page 1 (like Kotlin)
        if page == 1 {
            self.latest_titles.borrow_mut().clear();
        }

        let mangas: Vec<types::Manga> = response
            .data
            .into_iter()
            .filter_map(|item| {
                let url = format!("/manga/{}", item.slug);

                // Deduplication: skip if already seen
                let mut cache = self.latest_titles.borrow_mut();
                if cache.contains(&url) {
                    return None;
                }
                cache.insert(url.clone());

                Some(types::Manga {
                    url: url.clone(),
                    title: item.name,
                    thumbnail_url: Some(self.guess_cover(&url)),
                    artist: None,
                    author: None,
                    description: None,
                    genre: None,
                    status: types::MangaStatus::Unknown,
                    update_strategy: types::UpdateStrategy::AlwaysUpdate,
                    initialized: false,
                })
            })
            .collect();

        let has_next_page = page < response.total_pages;

        Ok(types::MangaPage {
            mangas,
            has_next_page,
        })
    }

    /// Parse search suggestions from JSON response
    fn parse_search_json(&self, body: &str, page: u32) -> Result<types::MangaPage, String> {
        let suggestions: Vec<SuggestionDto> =
            serde_json::from_str(body).map_err(|_| "Failed to parse search JSON".to_string())?;

        // Paginate like Kotlin: 24 items per page
        let start = ((page - 1) * 24) as usize;
        let end = (page * 24) as usize;
        let total = suggestions.len();

        let mangas: Vec<types::Manga> = suggestions
            .into_iter()
            .skip(start)
            .take(24)
            .map(|item| {
                let url = format!("/manga/{}", item.data);
                types::Manga {
                    url: url.clone(),
                    title: item.value,
                    thumbnail_url: Some(self.guess_cover(&url)),
                    artist: None,
                    author: None,
                    description: None,
                    genre: None,
                    status: types::MangaStatus::Unknown,
                    update_strategy: types::UpdateStrategy::AlwaysUpdate,
                    initialized: false,
                }
            })
            .collect();

        let has_next_page = end < total;

        Ok(types::MangaPage {
            mangas,
            has_next_page,
        })
    }

    /// Guess cover URL from manga path
    fn guess_cover(&self, manga_url: &str) -> String {
        let slug = manga_url
            .trim_start_matches("/manga/")
            .trim_end_matches('/');

        format!(
            "{}/uploads/manga/{}/cover/cover_250x350.jpg",
            self.base_url, slug
        )
    }

    /// Parse status from manga details HTML
    /// EXACTLY like Kotlin: div.manga-name span.label
    fn parse_status(&self, body: &str) -> mmrcms::MangaStatus {
        // Look for: <div class="manga-name"><span class="label">Status Text</span></div>
        if let Some(manga_name_start) = body.find("div.manga-name") {
            let remaining = &body[manga_name_start..];
            if let Some(label_start) = remaining.find("span.label") {
                let label_section = &remaining[label_start..];
                if let Some(content_start) = label_section.find('>') {
                    let content = &label_section[content_start + 1..];
                    if let Some(content_end) = content.find("</span>") {
                        let status_text = content[..content_end].to_lowercase();

                        // Match Kotlin's detailStatusComplete, detailStatusOngoing, detailStatusDropped
                        if status_text.contains("complete")
                            || status_text.contains("completo")
                            || status_text.contains("completado")
                        {
                            return mmrcms::MangaStatus::Completed;
                        } else if status_text.contains("ongoing")
                            || status_text.contains("en curso")
                            || status_text.contains("activo")
                            || status_text.contains("publicándose")
                        {
                            return mmrcms::MangaStatus::Ongoing;
                        } else if status_text.contains("dropped")
                            || status_text.contains("cancelado")
                            || status_text.contains("abandonado")
                        {
                            return mmrcms::MangaStatus::Cancelled;
                        }
                    }
                }
            }
        }

        mmrcms::MangaStatus::Unknown
    }

    /// Get AES key by fetching and deobfuscating ads2.js
    /// Caches the key after first fetch (like Kotlin with retry logic)
    fn get_aes_key(&self) -> Result<String, String> {
        // Return cached key if available
        if let Some(ref key) = *self.cached_aes_key.borrow() {
            return Ok(key.clone());
        }

        // 1. Fetch the JavaScript file using host tools
        let script_url = format!("{}/js/ads2.js", self.base_url);

        let request = host_tools::HttpRequest {
            url: script_url,
            method: "GET".to_string(),
            headers: vec![("Referer".to_string(), format!("{}/", self.base_url))],
            body: None,
            rate_limit_millis: Some(1000), // 1 request per second
        };

        let response = host_tools::fetch(&request)?;

        if response.status != 200 {
            return Err(format!("HTTP error: {}", response.status));
        }

        let script = response.body;

        // Extract the key inside the extension. The app remains a generic host.
        // This matches the original deobfuscateScript + KEY_REGEX behavior.
        let deobfuscate_script = deobfuscate_js_script(&script)
            .map_err(|_| "No se pudo desofuscar el script".to_string())?;
        let key = extract_decrypt_key(&deobfuscate_script)?;

        /*
                // Extract key using Kotlin's KEY_REGEX: decrypt\(.*?,\s*(.*?)\s*,.*\)
                const keyRegex = /decrypt\(.*?,\s*(.*?)\s*,.*\)/;
                const match = deobfuscated.match(keyRegex);

                if (!match) throw new Error('No se pudo encontrar la clave');

                const variable = match[1];

                // If it's a string literal, return it
                if (variable.startsWith("'")) return variable.slice(1, -1);
                if (variable.startsWith('"')) return variable.slice(1, -1);

                // Find the variable definition: (?:let|var|const)\s+variable\s*=\s*['"](.*)['"]
                const varRegex = new RegExp(`(?:let|var|const)\\\\s+${{variable}}\\\\s*=\\\\s*['"](.*)['"]`);
                const varMatch = deobfuscated.match(varRegex);
                if (!varMatch) throw new Error('No se pudo encontrar la clave');
                return varMatch[1];
            }})();
            "#,
            script.replace('`', r"\`").replace('\\', r"\\")
        );

        // 3. Extract the key inside the extension. The app remains a generic host.
        let deobfuscate_script = deobfuscate_js_script(&script).unwrap_or_else(|_| script.clone());
        let key = extract_decrypt_key(&deobfuscate_script)?;
        */

        // 4. Cache the key
        *self.cached_aes_key.borrow_mut() = Some(key.clone());

        Ok(key)
    }

    /// Parse encrypted chapter list
    /// Uses host tools to obtain the AES key
    /// EXACTLY like Kotlin's chapterListParse
    fn parse_encrypted_chapters(
        &self,
        body: &str,
        manga_url: &str,
    ) -> Result<types::ChapterList, String> {
        // Find encrypted chapter data in HTML using Kotlin's CHAPTER_DATA_REGEX
        // Pattern: \{(?=.*\\"ct\\")(?=.*\\"iv\\")(?=.*\\"s\\").*?\}
        let encrypted_json = extract_between(body, r#"{"ct":"#, r#"}"#)
            .map(|s| format!(r#"{{"ct":"{}"}}"#, s))
            .ok_or("No se pudo encontrar la lista de capítulos".to_string())?;

        // Parse encrypted data
        let encrypted: EncryptedChapterData = serde_json::from_str(&encrypted_json)
            .map_err(|_| "Failed to parse encrypted data".to_string())?;

        // Get AES key using host tools (with cache and retry like Kotlin)
        let aes_key = self.get_aes_key()?;

        // Decode salt from hex (like Kotlin's decodeHex())
        let salt = decode_hex(&encrypted.s)?;

        // Prepend "Salted__" to ciphertext (CryptoJS format)
        // EXACTLY like Kotlin: val cipherText = SALTED + salt + unsaltedCipherText
        let salted_prefix = b"Salted__";
        let mut full_ciphertext = Vec::new();
        full_ciphertext.extend_from_slice(salted_prefix);
        full_ciphertext.extend_from_slice(&salt);

        // Decode base64 ciphertext
        use base64::{engine::general_purpose, Engine as _};
        let ct_bytes = general_purpose::STANDARD
            .decode(&encrypted.ct)
            .map_err(|_| "Failed to decode ciphertext".to_string())?;
        full_ciphertext.extend_from_slice(&ct_bytes);

        // Encode back to base64 for decrypt_aes
        let full_ciphertext_b64 = general_purpose::STANDARD.encode(&full_ciphertext);

        // Decrypt using CryptoAES (matches Kotlin's CryptoAES.decrypt)
        let decrypted = decrypt_aes(&full_ciphertext_b64, &aes_key)
            .map_err(|_| "No se pudo desencriptar los capítulos".to_string())?;

        // Unescape (matches Kotlin's unescapeJava() and unescape())
        let unescaped = unescape_java(&decrypted);
        let trimmed = unescaped.trim_matches('"');
        let final_json = unescape_backslashes(trimmed);

        // Parse chapter data
        let chapters_data: Vec<ChapterData> = serde_json::from_str(&final_json)
            .map_err(|_| "Failed to parse chapter JSON".to_string())?;

        // Convert to Chapter objects (EXACTLY like Kotlin)
        let base_url = manga_url.trim_end_matches('/');
        let chapters: Vec<types::Chapter> = chapters_data
            .into_iter()
            .map(|ch| {
                let chapter_number = ch.number.parse::<f32>().unwrap_or(-1.0);

                // Match Kotlin's name logic
                let name = if ch.name == format!("Capítulo {}", ch.number) {
                    ch.name
                } else {
                    format!("Capítulo {}: {}", ch.number, ch.name)
                };

                types::Chapter {
                    url: format!("{}/{}", base_url, ch.slug),
                    name,
                    date_upload: parse_date(&ch.created_at),
                    chapter_number,
                    scanlator: None,
                }
            })
            .collect();

        Ok(types::ChapterList { chapters })
    }

    /// Parse pages with Base64 decoding support
    /// EXACTLY like Kotlin: #all > img.img-responsive
    fn parse_pages_with_decoding(&self, body: &str) -> Result<types::PageList, String> {
        let mut pages = Vec::new();
        let mut index = 0;

        // Find #all container and parse img.img-responsive
        // This matches Kotlin's: document.select("#all > img.img-responsive")
        if let Some(all_start) = body.find("id=\"all\"") {
            let remaining = &body[all_start..];

            let mut search = remaining;
            while let Some(img_start) = search.find("<img") {
                search = &search[img_start..];

                let img_tag_end = search.find('>').unwrap_or(search.len());
                let img_tag = &search[..img_tag_end];

                // Only process img.img-responsive (like Kotlin selector)
                if !img_tag.contains("img-responsive") {
                    search = &search[1..];
                    continue;
                }

                // Extract image URL using imgAttr() logic
                // Priority: data-src, data-lazy-src, data-original, src
                let url = extract_attribute(img_tag, "data-cfsrc")
                    .or_else(|| extract_attribute(img_tag, "data-src"))
                    .or_else(|| extract_attribute(img_tag, "data-lazy-src"))
                    .or_else(|| extract_attribute(img_tag, "data-original"))
                    .or_else(|| extract_attribute(img_tag, "src"));

                if let Some(mut image_url) = url {
                    // Handle Base64 encoded URLs (Kotlin's logic)
                    // if (url.toHttpUrlOrNull() == null) { decode base64 }
                    if image_url.starts_with("://") {
                        if let Some(encoded) = image_url.strip_prefix("://") {
                            if let Some(decoded) = decode_base64_string(encoded) {
                                image_url = url_decode(&decoded);
                            }
                        }
                    } else if image_url.starts_with("//") {
                        image_url = format!("https:{}", image_url);
                    } else if image_url.starts_with('/') {
                        image_url = format!("{}{}", self.base_url, image_url);
                    }

                    // Skip loading placeholders
                    if !image_url.contains("loading.gif") {
                        pages.push(types::Page {
                            index,
                            url: image_url.clone(),
                            image_url: Some(image_url),
                        });
                        index += 1;
                    }
                }

                search = &search[img_tag_end..];
            }
        }

        if pages.is_empty() {
            return Err("No pages found".to_string());
        }

        Ok(types::PageList { pages })
    }
}

/// Parse date string to Unix timestamp (milliseconds)
fn parse_date(date_str: &str) -> i64 {
    let parts: Vec<&str> = date_str.split(' ').collect();
    if parts.len() != 2 {
        return 0;
    }

    let date_parts: Vec<&str> = parts[0].split('-').collect();
    let time_parts: Vec<&str> = parts[1].split(':').collect();

    if date_parts.len() != 3 || time_parts.len() != 3 {
        return 0;
    }

    let year = date_parts[0].parse::<i32>().unwrap_or(0);
    let month = date_parts[1].parse::<i32>().unwrap_or(0);
    let day = date_parts[2].parse::<i32>().unwrap_or(0);
    let hour = time_parts[0].parse::<i32>().unwrap_or(0);
    let minute = time_parts[1].parse::<i32>().unwrap_or(0);
    let second = time_parts[2].parse::<i32>().unwrap_or(0);

    if year < 1970 || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return 0;
    }
    if !(0..=23).contains(&hour) || !(0..=59).contains(&minute) || !(0..=59).contains(&second) {
        return 0;
    }

    let days_in_month = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut days = (year - 1970) * 365;
    days += (year - 1969) / 4;

    for &month_days in days_in_month.iter().take((month - 1) as usize) {
        days += month_days;
    }
    days += day - 1;

    let total_seconds =
        (days as i64) * 86400 + (hour as i64) * 3600 + (minute as i64) * 60 + (second as i64);
    total_seconds * 1000
}

fn extract_decrypt_key(script: &str) -> Result<String, String> {
    let expression = extract_decrypt_key_expression(script)
        .ok_or("No se pudo encontrar la clave".to_string())?;
    let expression = expression.trim();

    if let Some(value) = extract_string_literal(expression) {
        return Ok(value);
    }

    let variable = expression
        .trim_start_matches("var ")
        .trim_start_matches("let ")
        .trim_start_matches("const ")
        .trim();
    extract_variable(script, variable).map_err(|_| "No se pudo encontrar la clave".to_string())
}

fn extract_decrypt_key_expression(script: &str) -> Option<String> {
    let call_start = script.find("decrypt(")?;
    let args_start = call_start + "decrypt(".len();
    let args_end = find_matching_paren(script, args_start)?;
    let args = split_top_level_args(&script[args_start..args_end]);
    args.get(1).cloned()
}

fn find_matching_paren(script: &str, start: usize) -> Option<usize> {
    let mut depth = 1;
    let mut quote = None;
    let mut escaped = false;

    for (offset, ch) in script[start..].char_indices() {
        if let Some(quote_char) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote_char {
                quote = None;
            }
            continue;
        }

        match ch {
            '\'' | '"' | '`' => quote = Some(ch),
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(start + offset);
                }
            }
            _ => {}
        }
    }

    None
}

fn split_top_level_args(args: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current_start = 0;
    let mut depth = 0;
    let mut quote = None;
    let mut escaped = false;

    for (index, ch) in args.char_indices() {
        if let Some(quote_char) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote_char {
                quote = None;
            }
            continue;
        }

        match ch {
            '\'' | '"' | '`' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                result.push(args[current_start..index].trim().to_string());
                current_start = index + ch.len_utf8();
            }
            _ => {}
        }
    }

    result.push(args[current_start..].trim().to_string());
    result
}

fn extract_string_literal(expression: &str) -> Option<String> {
    let mut chars = expression.chars();
    let quote = chars.next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }

    let mut value = String::new();
    let mut escaped = false;
    for ch in chars {
        if escaped {
            value.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == quote {
            return Some(value);
        } else {
            value.push(ch);
        }
    }

    None
}

/// Implement the WIT interface
struct Component;

impl Guest for Component {
    fn get_source_info() -> Result<types::SourceInfo, String> {
        Ok(types::SourceInfo {
            id: 440100,
            name: "Mangas.in".to_string(),
            lang: "es".to_string(),
            base_url: "https://m440.in".to_string(),
            supports_latest: true,
        })
    }

    fn get_popular(_page: u32, body: String) -> Result<types::MangaPage, String> {
        let ext = MangasIn::new();
        let result = ext.mmrcms.parse_popular(&body).map_err(|e| e.to_string())?;

        Ok(types::MangaPage {
            mangas: result.mangas.into_iter().map(convert_manga).collect(),
            has_next_page: result.has_next_page,
        })
    }

    fn get_latest(page: u32, body: String) -> Result<types::MangaPage, String> {
        let ext = MangasIn::new();

        let trimmed = body.trim_start();
        let first_char = trimmed.chars().next().unwrap_or(' ');

        if first_char == '{' {
            // JSON response from /lasted endpoint
            ext.parse_latest_json(&body, page)
        } else {
            // HTML response (fallback)
            let result = ext.mmrcms.parse_latest(&body).map_err(|e| e.to_string())?;
            Ok(types::MangaPage {
                mangas: result.mangas.into_iter().map(convert_manga).collect(),
                has_next_page: result.has_next_page,
            })
        }
    }

    fn search(_query: String, page: u32, body: String) -> Result<types::MangaPage, String> {
        let ext = MangasIn::new();

        // Detect response type (JSON vs HTML)
        let trimmed = body.trim_start();
        let first_char = trimmed.chars().next().unwrap_or(' ');

        if first_char == '[' {
            // JSON array response from /search endpoint (suggestions)
            ext.parse_search_json(&body, page)
        } else {
            // HTML response from /filterList or /advanced-search
            let result = ext.mmrcms.parse_search(&body).map_err(|e| e.to_string())?;
            Ok(types::MangaPage {
                mangas: result.mangas.into_iter().map(convert_manga).collect(),
                has_next_page: result.has_next_page,
            })
        }
    }

    fn get_manga_details(body: String) -> Result<types::Manga, String> {
        let ext = MangasIn::new();

        // Extract URL from HTML
        let url = extract_between(&body, "<link rel=\"canonical\" href=\"", "\"")
            .or_else(|| extract_between(&body, "<meta property=\"og:url\" content=\"", "\""))
            .and_then(|full_url| {
                full_url
                    .find("/manga/")
                    .map(|start| full_url[start..].to_string())
            })
            .unwrap_or_else(|| "/manga/unknown".to_string());

        // Get base details from MMRCMS
        let mut result = ext
            .mmrcms
            .parse_manga_details(&body, &url)
            .map_err(|e| e.to_string())?;

        // Override status with MangasIn-specific selector (like Kotlin)
        result.status = ext.parse_status(&body);

        Ok(convert_manga(result))
    }

    fn get_chapter_list(body: String, manga_url: String) -> Result<types::ChapterList, String> {
        let ext = MangasIn::new();

        // Check if body contains encrypted chapter data (like Kotlin)
        if body.contains(r#""ct":"#) {
            // Encrypted chapters - use host tools to get key and decrypt
            ext.parse_encrypted_chapters(&body, &manga_url)
        } else {
            // Regular HTML parsing
            let result = ext
                .mmrcms
                .parse_chapter_list(&body, &manga_url)
                .map_err(|e| e.to_string())?;
            Ok(types::ChapterList {
                chapters: result.chapters.into_iter().map(convert_chapter).collect(),
            })
        }
    }

    fn get_pages(body: String) -> Result<types::PageList, String> {
        let ext = MangasIn::new();
        ext.parse_pages_with_decoding(&body)
    }

    fn get_filters() -> Result<types::FilterList, String> {
        let ext = MangasIn::new();
        let result = ext.mmrcms.get_filters();

        Ok(types::FilterList {
            filters: result.filters.into_iter().map(convert_filter).collect(),
        })
    }

    fn build_url(operation: String, params: String) -> Result<String, String> {
        let ext = MangasIn::new();

        match operation.as_str() {
            "popular" => {
                let page: u32 = params.parse().unwrap_or(1);
                Ok(ext.mmrcms.build_popular_url(page))
            }
            "latest" => {
                let page: u32 = params.parse().unwrap_or(1);
                Ok(format!("{}/lasted?p={}", ext.base_url, page))
            }
            "search" => {
                #[derive(Deserialize)]
                struct SearchParams {
                    query: String,
                    page: u32,
                }

                let search_params: SearchParams = serde_json::from_str(&params)
                    .map_err(|_| "Invalid search params".to_string())?;

                if search_params.query.is_empty() {
                    Ok(ext.mmrcms.build_popular_url(search_params.page))
                } else {
                    Ok(format!(
                        "{}/search?q={}",
                        ext.base_url,
                        url_encode(&search_params.query)
                    ))
                }
            }
            "manga" => Ok(ext.mmrcms.build_manga_url(&params)),
            "chapter" => Ok(ext.mmrcms.build_chapter_url(&params)),
            _ => Err(format!("Unknown operation: {}", operation)),
        }
    }
}

export!(Component);
