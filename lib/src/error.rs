extern crate alloc;
use alloc::string::String;
use thiserror::Error;

/// Main error type for the library
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ExtensionError {
    /// Base64 decoding failed
    #[error("Base64 decode error: {0}")]
    Base64Decode(String),
    
    /// Invalid ciphertext format
    #[error("Invalid ciphertext format: {0}")]
    InvalidCiphertext(String),
    
    /// Cryptographic operation failed
    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),
    
    /// UTF-8 decoding failed
    #[error("UTF-8 decode error: {0}")]
    Utf8Decode(String),
    
    /// Invalid padding detected
    #[error("Invalid PKCS7 padding: {0}")]
    InvalidPadding(String),
    
    /// Invalid key or IV length
    #[error("Invalid key/IV length: {0}")]
    InvalidKeyLength(String),
    
    /// Hex decoding failed
    #[error("Hex decode error: {0}")]
    HexDecode(String),
    
    /// JavaScript deobfuscation failed
    #[error("Deobfuscation failed: {0}")]
    DeobfuscationFailed(String),
    
    /// Variable not found in script
    #[error("Variable '{0}' not found in script")]
    VariableNotFound(String),
    
    /// HTML parsing error
    #[error("HTML parsing error: {0}")]
    HtmlParse(String),
}

// Implement conversion to String for compatibility with existing code
impl From<ExtensionError> for alloc::string::String {
    fn from(err: ExtensionError) -> Self {
        alloc::format!("{}", err)
    }
}

/// Result type alias for convenience
pub type Result<T> = core::result::Result<T, ExtensionError>;
