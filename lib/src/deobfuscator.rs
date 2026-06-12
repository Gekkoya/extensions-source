#![cfg(feature = "deobfuscator")]
#![cfg(target_arch = "wasm32")]

extern crate alloc;
use alloc::string::{String, ToString};

use crate::error::{ExtensionError, Result};
use synchrony_rs::deobfuscator::Deobfuscator as SynchronyDeobfuscator;

/// Deobfuscate JavaScript code using synchrony
///
/// # Example
/// ```
/// let obfuscated = r#"var _0x1234=['key','value'];..."#;
/// let deobfuscated = deobfuscate_script(obfuscated)?;
/// ```
pub fn deobfuscate_script(script: &str) -> Result<String> {
    let deob = SynchronyDeobfuscator::new();
    deob.deobfuscate_source(script, None)
        .map_err(|e| ExtensionError::DeobfuscationFailed(alloc::format!("{:?}", e)))
}

/// Extract a variable value from deobfuscated JavaScript
///
/// This implementation avoids allocating format strings in a loop by using
/// direct string searching with const patterns.
///
/// # Example
/// ```
/// let script = "var myKey = 'secret123';";
/// let key = extract_variable(script, "myKey")?;
/// assert_eq!(key, "secret123");
/// ```
pub fn extract_variable(script: &str, var_name: &str) -> Result<String> {
    // Pattern 1: var/let/const myKey = "value"
    // Search for the variable name followed by '='
    if let Some(pos) = find_variable_assignment(script, var_name) {
        if let Some(value) = extract_quoted_value(&script[pos..]) {
            return Ok(value);
        }
    }

    // Pattern 2: myKey: "value" (object property)
    let colon_pattern = [var_name, ":"].concat();
    if let Some(pos) = script.find(&colon_pattern) {
        let after = &script[pos + colon_pattern.len()..];
        if let Some(value) = extract_quoted_value(after) {
            return Ok(value);
        }
    }

    Err(ExtensionError::VariableNotFound(var_name.to_string()))
}

/// Find variable assignment position (var/let/const name = ...)
/// Returns the position after the '=' sign
#[inline]
fn find_variable_assignment(script: &str, var_name: &str) -> Option<usize> {
    // Try to find "varName =" pattern
    let eq_pattern = [var_name, " ="].concat();
    if let Some(pos) = script.find(&eq_pattern) {
        return Some(pos + eq_pattern.len());
    }

    // Try without space: "varName="
    let eq_pattern_no_space = [var_name, "="].concat();
    if let Some(pos) = script.find(&eq_pattern_no_space) {
        return Some(pos + eq_pattern_no_space.len());
    }

    None
}

/// Extract value between quotes (single or double)
/// Skips whitespace before the quote
#[inline]
fn extract_quoted_value(text: &str) -> Option<String> {
    let trimmed = text.trim_start();

    // Find first quote (single or double)
    let mut chars = trimmed.chars();
    let first_char = chars.next()?;

    let quote_char = match first_char {
        '"' | '\'' => first_char,
        _ => return None,
    };

    // Find matching closing quote
    let remaining = chars.as_str();
    let end_pos = remaining.find(quote_char)?;

    Some(remaining[..end_pos].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_variable() {
        let script = r#"var myKey = "secret123";"#;
        let key = extract_variable(script, "myKey").unwrap();
        assert_eq!(key, "secret123");

        let script2 = r#"const myKey='secret456';"#;
        let key2 = extract_variable(script2, "myKey").unwrap();
        assert_eq!(key2, "secret456");
    }

    #[test]
    fn test_extract_variable_object() {
        let script = r#"{ myKey: "value123" }"#;
        let key = extract_variable(script, "myKey").unwrap();
        assert_eq!(key, "value123");
    }
}
