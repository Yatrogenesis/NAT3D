// SOTA 8: Streaming Asset Encryption
// Functional XOR cipher acting as a foundation for AES-GCM streaming

pub fn encrypt_asset_stream(data: &mut [u8], key: &[u8]) {
    if key.is_empty() {
        return;
    }
    let key_len = key.len();
    for i in 0..data.len() {
        data[i] ^= key[i % key_len];
    }
}

pub fn decrypt_asset_stream(data: &mut [u8], key: &[u8]) {
    // XOR is symmetric
    encrypt_asset_stream(data, key);
}
