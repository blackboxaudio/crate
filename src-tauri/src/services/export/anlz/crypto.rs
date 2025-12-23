//! XOR encryption/decryption for PSSI (song structure) tags
//!
//! PSSI tags in Rekordbox 6+ use XOR encryption with a rotating mask.

#![allow(dead_code)]

/// XOR mask bytes used for PSSI encryption
const XOR_MASK: [u8; 19] = [
    0xCB, 0xE1, 0xEE, 0xFA, 0xE5, 0xEE, 0xAD, 0xEE, 0xE9, 0xD2, 0xE9, 0xEB, 0xE1, 0xE9, 0xF3, 0xE8,
    0xE9, 0xF4, 0xE1,
];

/// XOR encrypt/decrypt data for PSSI tags
///
/// The encryption is symmetric (encrypt and decrypt are the same operation).
/// Each byte is XOR'd with a mask byte that depends on:
/// - The position in the data (cycling through the 19-byte mask)
/// - The number of entries in the structure
///
/// # Arguments
/// * `data` - The data to encrypt/decrypt (modified in place)
/// * `len_entries` - The number of entries in the PSSI structure
pub fn xor_crypt(data: &mut [u8], len_entries: u16) {
    for (i, byte) in data.iter_mut().enumerate() {
        let mask = XOR_MASK[i % 19].wrapping_add(len_entries as u8);
        *byte ^= mask;
    }
}

/// XOR encrypt data for PSSI tags, returning a new vector
pub fn xor_encrypt(data: &[u8], len_entries: u16) -> Vec<u8> {
    let mut result = data.to_vec();
    xor_crypt(&mut result, len_entries);
    result
}

/// XOR decrypt data for PSSI tags, returning a new vector
pub fn xor_decrypt(data: &[u8], len_entries: u16) -> Vec<u8> {
    // Decryption is the same as encryption (XOR is symmetric)
    xor_encrypt(data, len_entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xor_roundtrip() {
        let original = vec![0x01, 0x02, 0x03, 0x04, 0x05];
        let encrypted = xor_encrypt(&original, 5);
        let decrypted = xor_decrypt(&encrypted, 5);
        assert_eq!(original, decrypted);
    }

    #[test]
    fn test_xor_different_len_entries() {
        let data = vec![0x00; 10];
        let enc1 = xor_encrypt(&data, 1);
        let enc2 = xor_encrypt(&data, 2);
        // Different len_entries should produce different results
        assert_ne!(enc1, enc2);
    }
}
