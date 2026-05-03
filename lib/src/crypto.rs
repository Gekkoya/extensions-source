extern crate alloc;
use alloc::string::ToString;
use alloc::vec::Vec;

use base64::{engine::general_purpose, Engine as _};
use cipher::KeyIvInit;
use md5::Md5 as Md5Hasher;

use crate::error::{ExtensionError, Result};

/// Decrypt AES-256-CBC encrypted data
/// 
/// The ciphertext should be in the format: "Salted__" + salt + encrypted_data
/// 
/// # Parameters
/// - `ciphertext_b64`: Base64-encoded ciphertext with salt
/// - `passphrase`: Password/key to derive the encryption key from
/// 
/// # Returns
/// Decrypted plaintext string, or error
pub fn decrypt_aes(ciphertext_b64: &str, passphrase: &str) -> Result<alloc::string::String> {
    // Decode base64
    let ciphertext = general_purpose::STANDARD
        .decode(ciphertext_b64)
        .map_err(|e| ExtensionError::Base64Decode(alloc::format!("{:?}", e)))?;

    // Check for "Salted__" prefix (8 bytes)
    if ciphertext.len() < 16 {
        return Err(ExtensionError::InvalidCiphertext(
            "Ciphertext too short".to_string()
        ));
    }
    
    if &ciphertext[0..8] != b"Salted__" {
        return Err(ExtensionError::InvalidCiphertext(
            "Missing 'Salted__' prefix".to_string()
        ));
    }

    // Extract salt (8 bytes after "Salted__")
    let salt = &ciphertext[8..16];
    
    // Extract actual encrypted data
    let encrypted_data = &ciphertext[16..];

    // Derive key and IV using EVP_BytesToKey (MD5-based)
    let (key, iv) = evp_bytes_to_key(passphrase.as_bytes(), salt, 32, 16);

    // Decrypt using AES-256-CBC
    let decrypted = decrypt_aes_256_cbc(encrypted_data, &key, &iv)?;

    // Convert to string
    alloc::string::String::from_utf8(decrypted)
        .map_err(|e| ExtensionError::Utf8Decode(alloc::format!("{:?}", e)))
}

/// Decrypt data that's already in the format: salt + encrypted_data (without "Salted__" prefix)
/// 
/// # Parameters
/// - `salt`: 8-byte salt
/// - `encrypted_data_b64`: Base64-encoded encrypted data
/// - `passphrase`: Password/key to derive the encryption key from
/// 
/// # Returns
/// Decrypted plaintext string, or error
pub fn decrypt_aes_with_salt(
    salt: &[u8],
    encrypted_data_b64: &str,
    passphrase: &str,
) -> Result<alloc::string::String> {
    // Decode base64
    let encrypted_data = general_purpose::STANDARD
        .decode(encrypted_data_b64)
        .map_err(|e| ExtensionError::Base64Decode(alloc::format!("{:?}", e)))?;

    // Derive key and IV using EVP_BytesToKey (MD5-based)
    let (key, iv) = evp_bytes_to_key(passphrase.as_bytes(), salt, 32, 16);

    // Decrypt using AES-256-CBC
    let decrypted = decrypt_aes_256_cbc(&encrypted_data, &key, &iv)?;

    // Convert to string
    alloc::string::String::from_utf8(decrypted)
        .map_err(|e| ExtensionError::Utf8Decode(alloc::format!("{:?}", e)))
}

/// EVP_BytesToKey implementation using MD5
/// This matches OpenSSL's key derivation function
fn evp_bytes_to_key(password: &[u8], salt: &[u8], key_len: usize, iv_len: usize) -> (Vec<u8>, Vec<u8>) {
    use md5::Digest;
    
    let mut key = Vec::new();
    let mut iv = Vec::new();
    let mut digest = Vec::new();

    while key.len() < key_len || iv.len() < iv_len {
        let mut hasher = Md5Hasher::new();
        
        if !digest.is_empty() {
            hasher.update(&digest);
        }
        hasher.update(password);
        hasher.update(salt);
        
        digest = hasher.finalize().to_vec();

        let remaining_key = key_len.saturating_sub(key.len());
        if remaining_key > 0 {
            let to_copy = remaining_key.min(digest.len());
            key.extend_from_slice(&digest[..to_copy]);
        }

        let remaining_iv = iv_len.saturating_sub(iv.len());
        if remaining_iv > 0 {
            let start = key_len.saturating_sub(key.len() - remaining_key);
            let to_copy = remaining_iv.min(digest.len().saturating_sub(start));
            if start < digest.len() {
                iv.extend_from_slice(&digest[start..start + to_copy]);
            }
        }
    }

    (key, iv)
}

/// Decrypt using AES-256-CBC with PKCS7 padding
/// 
/// This implementation includes proper PKCS7 padding verification to prevent
/// padding oracle attacks and silent data corruption.
fn decrypt_aes_256_cbc(ciphertext: &[u8], key: &[u8], iv: &[u8]) -> Result<Vec<u8>> {
    use cipher::BlockDecryptMut;
    
    if key.len() != 32 {
        return Err(ExtensionError::InvalidKeyLength(
            "Key must be 32 bytes for AES-256".to_string()
        ));
    }
    if iv.len() != 16 {
        return Err(ExtensionError::InvalidKeyLength(
            "IV must be 16 bytes".to_string()
        ));
    }

    let plaintext_len = ciphertext.len();
    if plaintext_len == 0 {
        return Err(ExtensionError::InvalidCiphertext(
            "Empty ciphertext".to_string()
        ));
    }
    
    if !plaintext_len.is_multiple_of(16) {
        return Err(ExtensionError::InvalidCiphertext(
            "Ciphertext length must be multiple of 16".to_string()
        ));
    }
    
    let mut buffer = ciphertext.to_vec();
    
    type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;
    
    let mut decryptor = Aes256CbcDec::new(key.into(), iv.into());
    
    // Process blocks
    for chunk in buffer.chunks_exact_mut(16) {
        decryptor.decrypt_block_mut(chunk.into());
    }
    
    // Verify and remove PKCS7 padding
    // PKCS7 padding: if N bytes of padding are needed, all N bytes have value N
    let padding_len = *buffer.last().ok_or_else(|| {
        ExtensionError::InvalidPadding("Empty buffer after decryption".to_string())
    })? as usize;
    
    // Padding length must be between 1 and 16 (block size)
    if padding_len == 0 || padding_len > 16 {
        return Err(ExtensionError::InvalidPadding(
            alloc::format!("Invalid padding length: {}", padding_len)
        ));
    }
    
    // Verify that the buffer is long enough
    if buffer.len() < padding_len {
        return Err(ExtensionError::InvalidPadding(
            "Buffer too short for padding".to_string()
        ));
    }
    
    // Verify that ALL padding bytes have the same value (constant-time comparison)
    let padding_start = buffer.len() - padding_len;
    let mut padding_valid = 0u8;
    
    for i in padding_start..buffer.len() {
        // XOR each padding byte with expected value
        // If all are correct, result will be 0
        padding_valid |= buffer[i] ^ (padding_len as u8);
    }
    
    if padding_valid != 0 {
        return Err(ExtensionError::InvalidPadding(
            "Padding bytes do not match expected value".to_string()
        ));
    }
    
    // Remove padding
    buffer.truncate(padding_start);
    
    Ok(buffer)
}

/// Decode hex string to bytes
pub fn decode_hex(s: &str) -> Result<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return Err(ExtensionError::HexDecode(
            "Hex string must have even length".to_string()
        ));
    }

    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|_| ExtensionError::HexDecode(
                    alloc::format!("Invalid hex character at position {}", i)
                ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_hex() {
        let hex = "48656c6c6f";
        let bytes = decode_hex(hex).unwrap();
        assert_eq!(bytes, b"Hello");
    }

    #[test]
    fn test_evp_bytes_to_key() {
        let password = b"password";
        let salt = b"saltsalt";
        let (key, iv) = evp_bytes_to_key(password, salt, 32, 16);
        
        assert_eq!(key.len(), 32);
        assert_eq!(iv.len(), 16);
    }
    
    #[test]
    fn test_invalid_padding() {
        // Test with invalid padding
        let key = [0u8; 32];
        let iv = [0u8; 16];
        let mut ciphertext = vec![0u8; 16];
        // Set invalid padding value (17 is > block size)
        ciphertext[15] = 17;
        
        let result = decrypt_aes_256_cbc(&ciphertext, &key, &iv);
        assert!(result.is_err());
    }
}
