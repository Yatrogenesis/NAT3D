// SPDX-License-Identifier: AGPL-3.0-or-later
// NAT3D license validation — Ed25519 asymmetric signature verification.
//
// Security model:
// - This binary contains ONLY the PUBLIC key (safe to distribute)
// - The keygen tool (never distributed) holds the PRIVATE key
// - Even if someone extracts this public key, they CANNOT generate valid serials
//
// Serial format: BASE32( Ed25519_Sign( private_key, "NAT3D|{machine_id}|{tier}" ) )
// The signature is stored IN FULL (64 bytes) — no truncation. Truncating an
// Ed25519 signature and zero-padding the rest does NOT verify; the verification
// equation depends on all 64 bytes (R || s). This was fixed after empirical
// testing confirmed the truncated scheme rejects 100% of legitimately-signed
// serials, including valid ones.
// Tiers: "pro" (commercial) · "edu" (academic, free)

/// LemonSqueezy product page — replace before launch.
pub const STORE_URL: &str = "https://nat3d.lemonsqueezy.com";

use base32::Alphabet;
use ed25519_dalek::{Verifier, VerifyingKey, Signature};

// PUBLIC KEY ONLY — safe to embed, cannot generate signatures.
// Generate a production keypair with: nat3d-keygen gen-keypair
// Then update this constant with your public key.
//
// This is the public key corresponding to the development private key (all zeros).
// REPLACE THIS with your production public key before distributing.
const LICENSE_PUBLIC_KEY: &[u8; 32] = &[
    0x3b, 0x6a, 0x27, 0xbc, 0xce, 0xb6, 0xa4, 0x2d,
    0x62, 0xa3, 0xa8, 0xd0, 0x2a, 0x6f, 0x0d, 0x73,
    0x65, 0x32, 0x15, 0x77, 0x1d, 0xe2, 0x43, 0xa6,
    0x3a, 0xc0, 0x48, 0xa1, 0x8b, 0x59, 0xda, 0x29,
];

fn get_verifying_key() -> VerifyingKey {
    VerifyingKey::from_bytes(LICENSE_PUBLIC_KEY).expect("Invalid public key")
}

pub fn validate_license(serial: &str, machine_id: &str) -> LicenseStatus {
    let verifying_key = get_verifying_key();
    let serial_clean = serial.trim().to_uppercase().replace('-', "");

    // Decode the FULL 64-byte signature from base32. A real Ed25519 signature
    // is exactly 64 bytes; anything else cannot possibly be valid, so we
    // reject early instead of attempting a doomed verification.
    let sig_bytes = match base32::decode(Alphabet::Rfc4648 { padding: false }, &serial_clean) {
        Some(bytes) if bytes.len() == 64 => bytes,
        _ => return LicenseStatus::Invalid,
    };

    let mut sig_array = [0u8; 64];
    sig_array.copy_from_slice(&sig_bytes);
    let signature = Signature::from_bytes(&sig_array);

    // Try each tier — the serial doesn't encode which tier it's for, so we
    // check both possible messages against the signature.
    for tier in &["pro", "edu"] {
        let message = format!("NAT3D|{machine_id}|{tier}");
        if verifying_key.verify(message.as_bytes(), &signature).is_ok() {
            return match *tier {
                "pro" => LicenseStatus::Licensed { tier: Tier::Pro },
                "edu" => LicenseStatus::Licensed { tier: Tier::Edu },
                _ => unreachable!(),
            };
        }
    }

    LicenseStatus::Invalid
}

pub fn get_machine_id() -> String {
    use sha2::{Digest, Sha256};
    let hostname = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "localhost".to_string());
    let username = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "unknown".to_string());
    let raw = format!("{hostname}:{username}");
    let hash = Sha256::digest(raw.as_bytes());
    base32::encode(Alphabet::Rfc4648 { padding: false }, &hash[..6])
}

// ── License status types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum LicenseStatus {
    Trial,
    Licensed { tier: Tier },
    Invalid,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tier {
    Pro,
    Edu,
}

impl LicenseStatus {
    pub fn allows_export(&self) -> bool {
        matches!(self, Self::Licensed { .. } | Self::Trial)
    }

    pub fn watermark_renders(&self) -> bool {
        matches!(self, Self::Trial)
    }

    pub fn display_label(&self) -> &str {
        match self {
            Self::Trial => "Trial (30 days)",
            Self::Licensed { tier: Tier::Pro } => "NAT3D Pro",
            Self::Licensed { tier: Tier::Edu } => "NAT3D Edu",
            Self::Invalid => "Unlicensed",
        }
    }
}

// ── Edu flow (GitHub OAuth RETIRED 2026-07-30) ────────────────────────────────

#[derive(Debug)]
pub enum EduFlowEvent {
    DeviceCodeReady {
        user_code: String,
        verification_uri: String,
        expires_in: u64,
    },
    EduConfirmed {
        serial: String,
        github_handle: String,
    },
    NotEduAccount {
        github_handle: String,
    },
    NotConfigured,
    Error(String),
}

pub fn start_edu_oauth_flow(tx: std::sync::mpsc::Sender<EduFlowEvent>, _machine_id: String) {
    let _ = tx.send(EduFlowEvent::NotConfigured);
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_id_is_stable() {
        let id1 = get_machine_id();
        let id2 = get_machine_id();
        assert_eq!(id1, id2);
        assert!(!id1.is_empty());
    }

    #[test]
    fn garbage_serial_rejected() {
        assert_eq!(
            validate_license("XXXX-YYYY-ZZZZ-AAAA", "TESTMACHINE"),
            LicenseStatus::Invalid
        );
    }

    #[test]
    fn empty_serial_rejected() {
        assert_eq!(validate_license("", "TESTMACHINE"), LicenseStatus::Invalid);
    }

    #[test]
    fn short_serial_rejected() {
        // Not 64 bytes decoded -> must be rejected without attempting verify.
        assert_eq!(
            validate_license("AAAAAAAAAAAAAAAAAAAAAAAA", "TESTMACHINE"),
            LicenseStatus::Invalid
        );
    }

    #[test]
    fn license_status_display() {
        assert_eq!(LicenseStatus::Trial.display_label(), "Trial (30 days)");
        assert_eq!(
            LicenseStatus::Licensed { tier: Tier::Pro }.display_label(),
            "NAT3D Pro"
        );
    }

    // NOTE: a true positive test (valid serial -> Licensed) requires signing
    // with the matching production private key, which does not live in this
    // repo. Add that test in the keygen crate's integration tests instead,
    // where the private key is available via NAT3D_LICENSE_PRIVKEY.
}
