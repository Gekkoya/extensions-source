#![no_std]

extern crate alloc;

pub mod error;
pub mod html_parser;
pub mod crypto;

#[cfg(all(feature = "deobfuscator", target_arch = "wasm32"))]
pub mod deobfuscator;

// Re-export error types for convenience
pub use error::{ExtensionError, Result};
