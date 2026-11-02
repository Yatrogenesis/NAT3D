//! Authenticated streaming asset payload operations.
//! In-place XOR was removed because it offered no confidentiality or integrity.
//! The encrypted format uses NAT3D's versioned AES-256-GCM payload format.
//! Encryption may change the buffer length because it adds a header, nonce and tag.

/// Encrypt an owned buffer, replacing plaintext only after successful encryption.
pub fn encrypt_asset_stream(data: &mut Vec<u8>, key: &[u8; 32]) -> Result<(), String> {
    let encrypted = crate::secure_save(data, key)?;
    *data = encrypted;
    Ok(())
}

/// Decrypt and authenticate an owned buffer, leaving it unchanged if verification fails.
pub fn decrypt_asset_stream(data: &mut Vec<u8>, key: &[u8; 32]) -> Result<(), String> {
    let decrypted = crate::secure_load(data, key)?;
    *data = decrypted;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authenticated_stream_roundtrip_and_tamper_rejection() {
        let key = [42u8; 32];
        let plain = b"scene asset".to_vec();
        let mut payload = plain.clone();
        encrypt_asset_stream(&mut payload, &key).unwrap();
        assert_ne!(payload, plain);
        let mut tampered = payload.clone();
        *tampered.last_mut().unwrap() ^= 1;
        assert!(decrypt_asset_stream(&mut tampered, &key).is_err());
        decrypt_asset_stream(&mut payload, &key).unwrap();
        assert_eq!(payload, plain);
    }
}
