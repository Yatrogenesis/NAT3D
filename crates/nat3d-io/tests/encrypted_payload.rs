use nat3d_io::crypto::stream_cipher::{decrypt_asset_stream, encrypt_asset_stream};
use nat3d_io::{secure_load, secure_save};

#[test]
fn empty_payload_roundtrip() {
    let key = [7; 32];
    let encrypted = secure_save(&[], &key).unwrap();
    assert_eq!(encrypted.len(), 37);
    assert_eq!(secure_load(&encrypted, &key).unwrap(), Vec::<u8>::new());
}

#[test]
fn every_truncation_is_rejected() {
    let key = [7; 32];
    let encrypted = secure_save(b"review payload", &key).unwrap();
    for end in 0..encrypted.len() {
        assert!(
            secure_load(&encrypted[..end], &key).is_err(),
            "prefix length {end}"
        );
    }
}

#[test]
fn every_single_byte_mutation_is_rejected() {
    let key = [7; 32];
    let encrypted = secure_save(b"review payload", &key).unwrap();
    for index in 0..encrypted.len() {
        let mut corrupted = encrypted.clone();
        corrupted[index] ^= 1;
        assert!(secure_load(&corrupted, &key).is_err(), "byte {index}");
    }
}

#[test]
fn extra_bytes_are_rejected() {
    let key = [7; 32];
    let mut encrypted = secure_save(b"review payload", &key).unwrap();
    encrypted.push(0);
    assert!(secure_load(&encrypted, &key).is_err());
}

#[test]
fn failed_wrapper_keeps_input() {
    let key = [7; 32];
    let mut encrypted = secure_save(b"review payload", &key).unwrap();
    let original = encrypted.clone();
    assert!(decrypt_asset_stream(&mut encrypted, &[8; 32]).is_err());
    assert_eq!(encrypted, original);

    encrypted[21] ^= 1;
    let corrupted = encrypted.clone();
    assert!(decrypt_asset_stream(&mut encrypted, &key).is_err());
    assert_eq!(encrypted, corrupted);

    let mut plain = b"plaintext".to_vec();
    let original = plain.clone();
    assert!(decrypt_asset_stream(&mut plain, &key).is_err());
    assert_eq!(plain, original);
}

#[test]
fn wrappers_interoperate_with_payload_api() {
    let key = [7; 32];
    let original = vec![0, 255, 128, 1, 0];
    let mut encrypted = original.clone();
    encrypt_asset_stream(&mut encrypted, &key).unwrap();
    assert_eq!(secure_load(&encrypted, &key).unwrap(), original);

    let mut encrypted = secure_save(&original, &key).unwrap();
    decrypt_asset_stream(&mut encrypted, &key).unwrap();
    assert_eq!(encrypted, original);
}
