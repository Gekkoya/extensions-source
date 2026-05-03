use gekkoya_lib::html_parser::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Maximum number of items per page returned by MMRCMS API
const MAX_ITEMS_PER_PAGE: usize = 50;

/// Errors that can occur during MMRCMS operations
#[derive(Error, Debug)]
pub enum MMRCMSError {
    #[error("Failed to parse manga list: {0}")]
    ParseMangaList(String),
    
    #[error("Failed to parse manga details: {0}")]
    ParseMangaDetails(String),
    
    #[error("Failed to parse chapter list: {0}")]
    ParseChapterList(String),
    
    #[error("Failed to parse page list: {0}")]
    ParsePageList(String),
    
    #[error("Failed to parse JSON: {0}")]
    JsonParse(#[from] serde_json::Error),
    
    #[error("No manga URL found in element")]
    NoMangaUrl,
    
    #[error("No manga title found in element")]
    NoMangaTitle,
    
    #[error("No pages found in chapter")]
    NoPagesFound,
    
    #[error("Unknown search response format")]
    UnknownSearchFormat,
}

/// Manga status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MangaStatus {
    Unknown = 0,
    Ongoing = 1,
    Completed = 2,
    Licensed = 3,
    PublishingFinished = 4,
    Cancelled = 5,
    OnHiatus = 6,
}

/// Update strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateStrategy {
    AlwaysUpdate = 0,
    OnlyFetchOnce = 1,
}

/// Manga metadata structure (internal type)
#[derive(Debug, Clone)]
pub struct Manga {
    pub url: String,
    pub title: String,
    pub artist: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub genre: Option<String>,
    pub status: MangaStatus,
    pub thumbnail_url: Option<String>,
    pub update_strategy: UpdateStrategy,
    pub initialized: bool,
}

/// Chapter metadata structure (internal type)
#[derive(Debug, Clone)]
pub struct Chapter {
    pub url: String,
    pub name: String,
    pub date_upload: i64,
    pub chapter_number: f32,
    pub scanlator: Option<String>,
}

/// Page structure (internal type)
#[derive(Debug, Clone)]
pub struct Page {
    pub index: u32,
    pub url: String,
    pub image_url: Option<String>,
}

/// Manga page response with pagination (internal type)
#[derive(Debug, Clone)]
pub struct MangaPage {
    pub mangas: Vec<Manga>,
    pub has_next_page: bool,
}

/// Chapter list response (internal type)
#[derive(Debug, Clone)]
pub struct ChapterList {
    pub chapters: Vec<Chapter>,
}

/// Page list response (internal type)
#[derive(Debug, Clone)]
pub struct PageList {
    pub pages: Vec<Page>,
}

/// Filter option for select-type filters (internal type)
#[derive(Debug, Clone)]
pub struct FilterOption {
    pub name: String,
    pub value: String,
}

/// Filter definition (internal type)
#[derive(Debug, Clone)]
pub struct Filter {
    pub filter_type: String,
    pub name: String,
    pub key: String,
    pub options: Option<Vec<FilterOption>>,
}

/// Filter list response (internal type)
#[derive(Debug, Clone)]
pub struct FilterList {
    pub filters: Vec<Filter>,
}

/// Supported languages for MMRCMS sources
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Spanish,
    French,
    English,
    Portuguese,
    Italian,
    German,
}

impl Language {
    /// Get the ISO 639-1 language code
    pub fn code(&self) -> &'static str {
        match self {
            Language::Spanish => "es",
            Language::French => "fr",
            Language::English => "en",
            Language::Portuguese => "pt",
            Language::Italian => "it",
            Language::German => "de",
        }
    }
    
    /// Get the word for "Chapter" in this language
    pub fn chapter_word(&self) -> &'static str {
        match self {
            Language::Spanish => "Capítulo",
            Language::French => "Chapitre",
            Language::English => "Chapter",
            Language::Portuguese => "Capítulo",
            Language::Italian => "Capitolo",
            Language::German => "Kapitel",
        }
    }
}

/// Configuration for an MMRCMS-based source
pub struct MMRCMSConfig {
    pub base_url: String,
    pub lang: Language,
    pub item_path: String,
    pub supports_latest: bool,
}

impl MMRCMSConfig {
    pub fn new(base_url: String, lang: Language) -> Self {
        Self {
            base_url,
            lang,
            item_path: "manga".to_string(),
            supports_latest: true,
        }
    }
}

/// Search suggestion DTO
#[derive(Debug, Deserialize, Serialize)]
pub struct SuggestionDto {
    pub value: String,
    pub data: String,
}

/// Base MMRCMS implementation
pub struct MMRCMS {
    config: MMRCMSConfig,
}

impl MMRCMS {
    pub fn new(config: MMRCMSConfig) -> Self {
        Self { config }
    }

    /// Build URL for popular manga list
    pub fn build_popular_url(&self, page: u32) -> String {
        format!(
            "{}/filterList?page={}&sortBy=views&asc=false",
            self.config.base_url, page
        )
    }

    /// Build URL for latest updates
    pub fn build_latest_url(&self, page: u32) -> String {
        format!("{}/latest-release?page={}", self.config.base_url, page)
    }

    /// Build URL for search
    pub fn build_search_url(&self, query: &str, page: u32) -> String {
        if query.is_empty() {
            self.build_popular_url(page)
        } else {
            format!(
                "{}/search?query={}",
                self.config.base_url,
                url_encode(query)
            )
        }
    }

    /// Build URL for manga details
    pub fn build_manga_url(&self, manga_path: &str) -> String {
        format!("{}{}", self.config.base_url, manga_path)
    }

    /// Build URL for chapter pages
    pub fn build_chapter_url(&self, chapter_path: &str) -> String {
        format!("{}{}", self.config.base_url, chapter_path)
    }

    /// Parse popular manga from HTML
    pub fn parse_popular(&self, body: &str) -> Result<MangaPage, MMRCMSError> {
        self.parse_manga_list(body)
    }

    /// Parse latest updates from HTML
    pub fn parse_latest(&self, body: &str) -> Result<MangaPage, MMRCMSError> {
        self.parse_manga_list(body)
    }

    /// Parse search results
    /// Can handle both HTML and JSON responses
    pub fn parse_search(&self, body: &str) -> Result<MangaPage, MMRCMSError> {
        let trimmed = body.trim_start();
        let first_char = trimmed.chars().next().unwrap_or(' ');

        if first_char == '[' {
            // JSON array of suggestions
            self.parse_search_json(body)
        } else if first_char == '<' {
            // HTML response
            self.parse_manga_list(body)
        } else {
            Err(MMRCMSError::UnknownSearchFormat)
        }
    }

    /// Parse manga list from HTML
    fn parse_manga_list(&self, body: &str) -> Result<MangaPage, MMRCMSError> {
        let mut mangas = Vec::new();

        // Split by <div class="media" to get each manga block
        let parts: Vec<&str> = body.split("<div class=\"media\"").collect();

        // Skip first part (before first media div), limit to MAX_ITEMS_PER_PAGE
        for part in parts.iter().skip(1).take(MAX_ITEMS_PER_PAGE) {
            if let Some(manga) = self.parse_manga_from_element(part) {
                mangas.push(manga);
            }
        }

        // Check for next page
        let has_next_page = body.contains("rel=\"next\"") || body.contains("rel='next'");

        Ok(MangaPage {
            mangas,
            has_next_page,
        })
    }

    /// Parse a single manga from HTML element
    fn parse_manga_from_element(&self, element: &str) -> Option<Manga> {
        // Extract URL
        let url = self.extract_manga_url(element)?;

        // Extract title
        let title = self.extract_manga_title(element)?;

        // Extract thumbnail
        let thumbnail_url = self.extract_manga_thumbnail(element, &url);

        Some(Manga {
            url,
            title,
            thumbnail_url,
            artist: None,
            author: None,
            description: None,
            genre: None,
            status: MangaStatus::Unknown,
            update_strategy: UpdateStrategy::AlwaysUpdate,
            initialized: false,
        })
    }

    /// Extract manga URL from element using attribute extraction
    fn extract_manga_url(&self, element: &str) -> Option<String> {
        // Use extract_attribute for safer parsing
        let href = extract_attribute(element, "href")?;
        
        // Check if it's an absolute URL with our base_url
        if let Some(path_start) = href.find(&format!("/{}/", self.config.item_path)) {
            return Some(href[path_start..].to_string());
        }
        
        // Check if it's already a relative URL starting with /manga/
        if href.starts_with(&format!("/{}/", self.config.item_path)) {
            return Some(href);
        }
        
        None
    }

    /// Extract manga title from element
    fn extract_manga_title(&self, element: &str) -> Option<String> {
        // Try <strong> tag first
        if let Some(title) = extract_between(element, "<strong>", "</strong>") {
            return Some(clean_html(&title));
        }

        // Try alt attribute
        if let Some(title) = extract_attribute(element, "alt") {
            return Some(clean_html(&title));
        }

        // Try title attribute
        if let Some(title) = extract_attribute(element, "title") {
            return Some(clean_html(&title));
        }

        None
    }

    /// Extract manga thumbnail from element
    fn extract_manga_thumbnail(&self, element: &str, manga_url: &str) -> Option<String> {
        // Try various image attributes
        let img_url = extract_attribute(element, "data-background-image")
            .or_else(|| extract_attribute(element, "data-cfsrc"))
            .or_else(|| extract_attribute(element, "data-lazy-src"))
            .or_else(|| extract_attribute(element, "data-src"))
            .or_else(|| extract_attribute(element, "src"));

        if let Some(url) = img_url {
            if url.ends_with("no-image.png") || url.is_empty() {
                return Some(self.guess_cover(manga_url));
            }
            return Some(url);
        }

        Some(self.guess_cover(manga_url))
    }

    /// Guess cover URL from manga URL
    fn guess_cover(&self, manga_url: &str) -> String {
        let slug = manga_url
            .trim_start_matches('/')
            .trim_start_matches(&format!("{}/", self.config.item_path))
            .trim_end_matches('/');

        format!(
            "{}/uploads/{}/{}/cover/cover_250x350.jpg",
            self.config.base_url, self.config.item_path, slug
        )
    }

    /// Parse search JSON response
    fn parse_search_json(&self, body: &str) -> Result<MangaPage, MMRCMSError> {
        let suggestions: Vec<SuggestionDto> = serde_json::from_str(body)?;

        let mangas: Vec<Manga> = suggestions
            .into_iter()
            .map(|item| {
                let url = format!("/{}/{}", self.config.item_path, item.data);
                Manga {
                    url: url.clone(),
                    title: item.value,
                    thumbnail_url: Some(self.guess_cover(&url)),
                    artist: None,
                    author: None,
                    description: None,
                    genre: None,
                    status: MangaStatus::Unknown,
                    update_strategy: UpdateStrategy::AlwaysUpdate,
                    initialized: false,
                }
            })
            .collect();

        Ok(MangaPage {
            mangas,
            has_next_page: false,
        })
    }

    /// Parse manga details from HTML
    pub fn parse_manga_details(&self, body: &str, url: &str) -> Result<Manga, MMRCMSError> {
        let slug = url
            .trim_start_matches('/')
            .trim_start_matches(&format!("{}/", self.config.item_path))
            .trim_end_matches('/');

        // Extract title
        let title = extract_between(body, "<h2 class=\"widget-title\"", "</h2>")
            .and_then(|s| extract_between(&s, ">", ""))
            .or_else(|| extract_between(body, "<h1>", "</h1>"))
            .unwrap_or_else(|| slug.replace('-', " "));

        // Extract description
        let description = extract_between(body, "<div class=\"well\">", "</div>")
            .map(|s| clean_html(&s));

        // Extract author
        let author = extract_dl_value(body, "Author(s)")
            .or_else(|| extract_dl_value(body, "Autor(es)"))
            .or_else(|| extract_dl_value(body, "Autor"));

        // Extract artist
        let artist = extract_dl_value(body, "Artist(s)")
            .or_else(|| extract_dl_value(body, "Artista(s)"))
            .or_else(|| extract_dl_value(body, "Artista"));

        // Extract genres
        let genre = extract_between(body, "<dt>Categories</dt>", "</dd>")
            .or_else(|| extract_between(body, "<dt>Categorías</dt>", "</dd>"))
            .or_else(|| extract_between(body, "<dt>Género</dt>", "</dd>"))
            .map(|s| extract_link_texts(&s));

        // Extract status
        let status_text = extract_between(body, "<dt>Status</dt>", "</dd>")
            .or_else(|| extract_between(body, "<dt>Estado</dt>", "</dd>"))
            .map(|s| clean_html(&s).to_lowercase());

        let status = match status_text.as_deref() {
            Some(s) if s.contains("complete") || s.contains("completo") => MangaStatus::Completed,
            Some(s) if s.contains("ongoing") || s.contains("curso") => MangaStatus::Ongoing,
            Some(s) if s.contains("dropped") => MangaStatus::Cancelled,
            _ => MangaStatus::Unknown,
        };

        // Extract thumbnail
        let thumbnail_url = extract_between(body, "<img class=\"img-responsive\" src=\"", "\"")
            .or_else(|| Some(self.guess_cover(url)));

        Ok(Manga {
            url: url.to_string(),
            title: clean_html(&title),
            artist,
            author,
            description,
            genre,
            status,
            thumbnail_url,
            update_strategy: UpdateStrategy::AlwaysUpdate,
            initialized: true,
        })
    }

    /// Parse chapter list from HTML
    pub fn parse_chapter_list(&self, body: &str, _manga_url: &str) -> Result<ChapterList, MMRCMSError> {
        let mut chapters = Vec::new();
        let mut remaining = body;

        // Extract manga title for cleaning chapter names
        let manga_title = extract_between(body, "<h2 class=\"widget-title\"", "</h2>")
            .and_then(|s| extract_between(&s, ">", ""))
            .or_else(|| extract_between(body, "<h1>", "</h1>"))
            .unwrap_or_default();

        // Find all chapter links
        while let Some(start) = remaining.find("<li") {
            remaining = &remaining[start..];

            // Look for chapter link using extract_attribute for safer parsing
            if let Some(link_start) = remaining.find("<a ") {
                let link_section = &remaining[link_start..];
                
                // Find the end of the <a> tag
                if let Some(tag_end) = link_section.find('>') {
                    let tag = &link_section[..tag_end];
                    
                    // Extract href using attribute extraction
                    if let Some(chapter_url) = extract_attribute(tag, "href") {
                        // Extract chapter name
                        let after_tag = &link_section[tag_end + 1..];
                        if let Some(name_end) = after_tag.find("</a>") {
                            let name = clean_html(&after_tag[..name_end]);
                            let cleaned_name = self.clean_chapter_name(&manga_title, &name);
                            let chapter_number = extract_chapter_number(&cleaned_name);

                            chapters.push(Chapter {
                                url: chapter_url,
                                name: cleaned_name,
                                date_upload: 0,
                                chapter_number,
                                scanlator: None,
                            });
                        }
                    }
                }
            }

            // Move to next <li>
            if let Some(next) = remaining.find("</li>") {
                remaining = &remaining[next + 5..];
            } else {
                break;
            }
        }

        Ok(ChapterList { chapters })
    }

    /// Clean chapter name by removing redundant manga title
    fn clean_chapter_name(&self, manga_title: &str, name: &str) -> String {
        let chapter_word = self.config.lang.chapter_word();

        let initial_name = name.replace(manga_title, chapter_word);

        // Use split_once for better performance and clarity
        match initial_name.split_once(':') {
            Some((first, second)) => {
                let first = first.trim();
                let second = second.trim();
                
                // If both parts are the same, return only one
                if first == second {
                    first.to_string()
                } else {
                    format!("{}: {}", first, second)
                }
            }
            None => initial_name,
        }
    }

    /// Parse page list from HTML
    pub fn parse_page_list(&self, body: &str) -> Result<PageList, MMRCMSError> {
        let mut pages = Vec::new();
        let mut index = 0;

        // Find #all container
        if let Some(all_start) = body.find("id=\"all\"") {
            let remaining = &body[all_start..];

            // Find all img.img-responsive tags
            let mut search = remaining;
            while let Some(img_start) = search.find("<img") {
                search = &search[img_start..];

                let img_tag_end = search.find('>').unwrap_or(search.len());
                let img_tag = &search[..img_tag_end];

                if !img_tag.contains("img-responsive") {
                    search = &search[1..];
                    continue;
                }

                // Extract image URL using imgAttr() logic with extract_attribute
                let url = extract_attribute(img_tag, "data-background-image")
                    .or_else(|| extract_attribute(img_tag, "data-cfsrc"))
                    .or_else(|| extract_attribute(img_tag, "data-lazy-src"))
                    .or_else(|| extract_attribute(img_tag, "data-src"))
                    .or_else(|| extract_attribute(img_tag, "src"));

                if let Some(mut image_url) = url {
                    // Handle protocol-relative URLs
                    if image_url.starts_with("//") {
                        image_url = format!("https:{}", image_url);
                    } else if image_url.starts_with('/') {
                        image_url = format!("{}{}", self.config.base_url, image_url);
                    }

                    // Skip loading placeholders
                    if !image_url.contains("loading.gif") {
                        pages.push(Page {
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
            return Err(MMRCMSError::NoPagesFound);
        }

        Ok(PageList { pages })
    }

    /// Get default filters
    pub fn get_filters(&self) -> FilterList {
        FilterList {
            filters: vec![Filter {
                filter_type: "text".to_string(),
                name: "Search".to_string(),
                key: "query".to_string(),
                options: None,
            }],
        }
    }
}
