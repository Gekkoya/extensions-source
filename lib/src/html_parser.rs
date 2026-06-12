extern crate alloc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Extract text between two delimiters
///
/// # Example
/// ```
/// let html = r#"<div>Hello</div>"#;
/// let result = extract_between(html, "<div>", "</div>");
/// assert_eq!(result, Some("Hello".to_string()));
/// ```
pub fn extract_between(text: &str, start: &str, end: &str) -> Option<String> {
    let start_idx = text.find(start)? + start.len();
    let remaining = &text[start_idx..];

    if end.is_empty() {
        return Some(remaining.to_string());
    }

    let end_idx = remaining.find(end)?;
    Some(remaining[..end_idx].to_string())
}

/// Extract HTML attribute value
/// Handles both single and double quotes
///
/// # Example
/// ```
/// let tag = r#"<img src="image.jpg" alt='test'>"#;
/// assert_eq!(extract_attribute(tag, "src"), Some("image.jpg".to_string()));
/// assert_eq!(extract_attribute(tag, "alt"), Some("test".to_string()));
/// ```
pub fn extract_attribute(tag: &str, attr_name: &str) -> Option<String> {
    // Try double quotes first: attr="value"
    let pattern_double = alloc::format!("{}=\"", attr_name);
    if let Some(start) = tag.find(&pattern_double) {
        let after = &tag[start + pattern_double.len()..];
        if let Some(end) = after.find('"') {
            return Some(after[..end].to_string());
        }
    }

    // Try single quotes: attr='value'
    let pattern_single = alloc::format!("{}='", attr_name);
    if let Some(start) = tag.find(&pattern_single) {
        let after = &tag[start + pattern_single.len()..];
        if let Some(end) = after.find('\'') {
            return Some(after[..end].to_string());
        }
    }

    None
}

/// Clean HTML by removing tags and decoding entities
///
/// This implementation uses a single-pass algorithm with O(n) complexity
/// instead of repeatedly calling replace_range which would be O(n²).
///
/// # Example
/// ```
/// let html = "<p>Hello &amp; <b>World</b></p>";
/// assert_eq!(clean_html(html), "Hello & World");
/// ```
pub fn clean_html(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut inside_tag = false;

    // Single-pass algorithm: O(n) complexity
    for ch in text.chars() {
        match ch {
            '<' => {
                inside_tag = true;
            }
            '>' => {
                inside_tag = false;
            }
            _ if !inside_tag => {
                result.push(ch);
            }
            _ => {
                // Inside tag, skip character
            }
        }
    }

    // Decode HTML entities in a single pass
    // Using const strings to avoid allocations
    const ENTITIES: &[(&str, &str)] = &[
        ("&nbsp;", " "),
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#39;", "'"),
        ("&#x27;", "'"),
        ("&apos;", "'"),
    ];

    for (entity, replacement) in ENTITIES {
        if result.contains(entity) {
            result = result.replace(entity, replacement);
        }
    }

    result.trim().to_string()
}

/// Extract all text content from anchor tags in a block
/// Returns comma-separated values
pub fn extract_link_texts(text: &str) -> String {
    let mut values = Vec::new();
    let mut remaining = text;

    while let Some(start) = remaining.find('>') {
        remaining = &remaining[start + 1..];
        if let Some(end) = remaining.find('<') {
            let value = remaining[..end].trim();
            if !value.is_empty() {
                values.push(value);
            }
            remaining = &remaining[end..];
        }
    }

    values.join(", ")
}

/// Extract value from definition list (dt/dd pairs)
///
/// # Example
/// ```
/// let html = "<dt>Author</dt><dd>John Doe</dd>";
/// assert_eq!(extract_dl_value(html, "Author"), Some("John Doe".to_string()));
/// ```
pub fn extract_dl_value(body: &str, key: &str) -> Option<String> {
    let dt_tag = alloc::format!("<dt>{}</dt>", key);
    let start = body.find(&dt_tag)?;
    let remaining = &body[start + dt_tag.len()..];

    extract_between(remaining, "<dd>", "</dd>")
        .map(|s| clean_html(&s))
        .filter(|s| !s.is_empty())
}

/// Extract chapter number from chapter name
/// Tries to find patterns like "Chapter X", "Capítulo X", etc.
pub fn extract_chapter_number(name: &str) -> f32 {
    let lower = name.to_lowercase();

    // Try to find "capítulo", "capitulo", or "chapter"
    if let Some(cap_idx) = lower
        .find("capítulo")
        .or_else(|| lower.find("capitulo"))
        .or_else(|| lower.find("chapter"))
    {
        let after = &name[cap_idx..];
        let mut num_str = String::new();
        let mut found_digit = false;

        for ch in after.chars() {
            if ch.is_ascii_digit() || (ch == '.' && found_digit) {
                num_str.push(ch);
                found_digit = true;
            } else if found_digit {
                break;
            }
        }

        if let Ok(num) = num_str.parse::<f32>() {
            return num;
        }
    }

    // Try to find any number in the string
    let mut num_str = String::new();
    let mut found_digit = false;

    for ch in name.chars() {
        if ch.is_ascii_digit() || (ch == '.' && found_digit) {
            num_str.push(ch);
            found_digit = true;
        } else if found_digit && !ch.is_whitespace() {
            break;
        }
    }

    num_str.parse::<f32>().unwrap_or(-1.0)
}

/// URL encode a string
pub fn url_encode(s: &str) -> String {
    let mut result = String::new();

    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            b' ' => {
                result.push('+');
            }
            _ => {
                result.push('%');
                result.push_str(&alloc::format!("{:02X}", byte));
            }
        }
    }

    result
}

/// URL decode a string
pub fn url_decode(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '%' => {
                let hex: String = chars.by_ref().take(2).collect();
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    result.push(byte as char);
                } else {
                    result.push('%');
                    result.push_str(&hex);
                }
            }
            '+' => result.push(' '),
            _ => result.push(ch),
        }
    }

    result
}

/// Decode Base64 string
pub fn decode_base64_string(encoded: &str) -> Option<String> {
    use base64::{engine::general_purpose, Engine as _};

    let decoded = general_purpose::STANDARD.decode(encoded).ok()?;
    String::from_utf8(decoded).ok()
}

/// Unescape Java-style unicode escapes (\uXXXX)
pub fn unescape_java(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                match next {
                    'u' => {
                        // Unicode escape: \uXXXX
                        let hex: String = chars.by_ref().take(4).collect();
                        if let Ok(code) = u32::from_str_radix(&hex, 16) {
                            if let Some(unicode_char) = char::from_u32(code) {
                                result.push(unicode_char);
                            } else {
                                result.push('\\');
                                result.push('u');
                                result.push_str(&hex);
                            }
                        } else {
                            result.push('\\');
                            result.push('u');
                            result.push_str(&hex);
                        }
                    }
                    'n' => result.push('\n'),
                    'r' => result.push('\r'),
                    't' => result.push('\t'),
                    '\\' => result.push('\\'),
                    '"' => result.push('"'),
                    '\'' => result.push('\''),
                    _ => {
                        result.push('\\');
                        result.push(next);
                    }
                }
            } else {
                result.push('\\');
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Unescape backslash escapes
pub fn unescape_backslashes(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                result.push(next);
            } else {
                result.push('\\');
            }
        } else {
            result.push(ch);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_between() {
        let html = r#"<div>Hello</div>"#;
        assert_eq!(
            extract_between(html, "<div>", "</div>"),
            Some("Hello".to_string())
        );
    }

    #[test]
    fn test_clean_html() {
        let html = "<p>Hello &amp; <b>World</b></p>";
        assert_eq!(clean_html(html), "Hello & World");
    }

    #[test]
    fn test_extract_chapter_number() {
        assert_eq!(extract_chapter_number("Capítulo 123"), 123.0);
        assert_eq!(extract_chapter_number("Chapter 45.5"), 45.5);
        assert_eq!(extract_chapter_number("No number here"), -1.0);
    }

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("hello world"), "hello+world");
        assert_eq!(url_encode("test@example.com"), "test%40example.com");
    }
}
