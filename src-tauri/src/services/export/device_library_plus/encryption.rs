//! SQLCipher key deobfuscation for Device Library Plus databases.
//!
//! Device Library Plus databases are encrypted with SQLCipher using a universal key
//! that is the same for all devices. The key is stored in obfuscated form and must
//! be decoded using the following steps:
//! 1. Base85 decode
//! 2. XOR with BLOB_KEY (cycling through the key bytes)
//! 3. Zlib decompress
//! 4. UTF-8 decode
//!
//! Reference: pyrekordbox/utils.py and pyrekordbox/devicelib_plus/database.py

use flate2::read::ZlibDecoder;
use std::io::Read;

use crate::error::{CrateError, Result};

/// Obfuscated blob containing the SQLCipher key (from pyrekordbox)
const BLOB: &str = "PN_1dH8$oLJY)16j_RvM6qphWw`476>;C1cWmI#se(PG`j}~xAjlufj?`#0i{;=glh(SkW)y0>n?YEiD`l%t(";

/// XOR key used for deobfuscation
const BLOB_KEY: &[u8] = b"657f48f84c437cc1";

/// Derives the SQLCipher key from the obfuscated blob.
///
/// The key is universal for all Device Library Plus databases (not machine or license dependent).
/// The resulting key starts with "r8gd" when properly deobfuscated.
pub fn get_sqlcipher_key() -> Result<String> {
    // Step 1: Base85 decode
    let decoded = base85::decode(BLOB).map_err(|e| {
        CrateError::Export(format!("Failed to decode Device Library Plus key: {e}"))
    })?;

    // Step 2: XOR with BLOB_KEY (cycling through key bytes)
    let xored: Vec<u8> = decoded
        .iter()
        .enumerate()
        .map(|(i, &byte)| byte ^ BLOB_KEY[i % BLOB_KEY.len()])
        .collect();

    // Step 3: Zlib decompress
    let mut decoder = ZlibDecoder::new(&xored[..]);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed).map_err(|e| {
        CrateError::Export(format!(
            "Failed to decompress Device Library Plus key: {e}"
        ))
    })?;

    // Step 4: UTF-8 decode
    let key = String::from_utf8(decompressed).map_err(|e| {
        CrateError::Export(format!(
            "Failed to decode Device Library Plus key as UTF-8: {e}"
        ))
    })?;

    // Validate the key starts with expected prefix
    if !key.starts_with("r8gd") {
        return Err(CrateError::Export(
            "Device Library Plus key validation failed: unexpected prefix".to_string(),
        ));
    }

    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_sqlcipher_key() {
        let key = get_sqlcipher_key().expect("Should successfully derive key");
        assert!(key.starts_with("r8gd"), "Key should start with 'r8gd'");
        assert!(!key.is_empty(), "Key should not be empty");
    }
}
