/*
 * NAT3D - Next-generation Advanced Technology for 3D
 * Professional 3D Modeling, CAD, Physics Simulation and Rendering Suite
 *
 * Copyright (C) 2023-2026 Francisco Molina <pako.molina@gmail.com>
 *
 * This software is dual-licensed:
 * 1. Open Source: GNU Affero General Public License v3.0 or later (AGPL-3.0-or-later)
 * 2. Commercial: For commercial use, please contact <fmolina@avermex.com>
 *
 * For research information, visit: https://research.avermex.com
 * For collaborations, contact: <pako.molina@gmail.com>
 *
 * DOI: [PENDING]
 */

//! # NAT3D I/O
//!
//! Import/Export for NAT3D 3D modeling, CAD, and simulation suite.
//!
//! This crate provides file format handlers and AES-256-GCM encryption for production security.

#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    missing_docs,
    unused_imports,
    dead_code
)]

pub mod crypto;
pub mod formats;
pub mod image;
pub mod video;

pub use formats::*;

use aes_gcm::aead::rand_core::{CryptoRng, RngCore};
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};

const ENCRYPTED_MAGIC: &[u8; 8] = b"NAT3DENC";
const ENCRYPTED_VERSION: u8 = 1;

/// Encrypts bytes using AES-256-GCM with a new random 96-bit nonce on every call.
/// The wire format is MAGIC (8 bytes), VERSION (1 byte), NONCE (12 bytes), ciphertext + tag.
/// Returns an error rather than saving the input unencrypted.
pub fn secure_save(data: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, String> {
    secure_save_with_rng(data, key, &mut OsRng)
}

fn secure_save_with_rng(
    data: &[u8],
    key: &[u8; 32],
    rng: &mut (impl CryptoRng + RngCore),
) -> Result<Vec<u8>, String> {
    let mut nonce_bytes = [0u8; 12];
    rng.try_fill_bytes(&mut nonce_bytes)
        .map_err(|_| "AES-256-GCM nonce generation failed".to_string())?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let encrypted = cipher
        .encrypt(nonce, data)
        .map_err(|_| "AES-256-GCM encryption failed".to_string())?;
    let mut output = Vec::with_capacity(9 + nonce.len() + encrypted.len());
    output.extend_from_slice(ENCRYPTED_MAGIC);
    output.push(ENCRYPTED_VERSION);
    output.extend_from_slice(nonce);
    output.extend_from_slice(&encrypted);
    Ok(output)
}

/// Decrypts a secure_save payload. Rejects invalid headers, truncated buffers,
/// incorrect keys, and tampered ciphertext; never falls back to plaintext.
pub fn secure_load(payload: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, String> {
    const HEADER_LEN: usize = 8 + 1 + 12;
    if payload.len() < HEADER_LEN + 16
        || &payload[..8] != ENCRYPTED_MAGIC
        || payload[8] != ENCRYPTED_VERSION
    {
        return Err("Invalid or unsupported NAT3D encrypted payload".into());
    }
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(&payload[9..HEADER_LEN]);
    cipher
        .decrypt(nonce, &payload[HEADER_LEN..])
        .map_err(|_| "AES-256-GCM authentication failed".to_string())
}

#[cfg(test)]
mod security_regression_tests {
    use super::*;

    #[test]
    fn nonce_generation_failure_returns_an_error() {
        struct FailingRng;

        impl RngCore for FailingRng {
            fn next_u32(&mut self) -> u32 {
                panic!("must use fallible nonce generation")
            }

            fn next_u64(&mut self) -> u64 {
                panic!("must use fallible nonce generation")
            }

            fn fill_bytes(&mut self, _dest: &mut [u8]) {
                panic!("must use fallible nonce generation")
            }

            fn try_fill_bytes(
                &mut self,
                _dest: &mut [u8],
            ) -> Result<(), aes_gcm::aead::rand_core::Error> {
                Err(aes_gcm::aead::rand_core::Error::new(std::io::Error::other(
                    "entropy source unavailable",
                )))
            }
        }

        impl CryptoRng for FailingRng {}

        let result = secure_save_with_rng(b"confidential scene", &[17; 32], &mut FailingRng);
        assert_eq!(result.unwrap_err(), "AES-256-GCM nonce generation failed");
    }

    #[test]
    fn ciphertext_round_trips_and_is_not_plaintext() {
        let key = [17u8; 32];
        let data = b"confidential scene";
        let ciphertext = secure_save(data, &key).unwrap();
        assert_ne!(ciphertext.as_slice(), data);
        assert_eq!(secure_load(&ciphertext, &key).unwrap(), data);
        assert_ne!(secure_save(data, &key).unwrap(), ciphertext);
    }

    #[test]
    fn wrong_key_tampering_and_plaintext_are_rejected() {
        let key = [17u8; 32];
        let mut ciphertext = secure_save(b"confidential scene", &key).unwrap();
        assert!(secure_load(&ciphertext, &[18u8; 32]).is_err());
        let last = ciphertext.len() - 1;
        ciphertext[last] ^= 1;
        assert!(secure_load(&ciphertext, &key).is_err());
        assert!(secure_load(b"confidential scene", &key).is_err());
    }
}
