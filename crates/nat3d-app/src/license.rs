// SPDX-License-Identifier: AGPL-3.0-or-later
//! License serial validation (Ed25519 signature check against an embedded public key), machine identifier derivation, and the license status and edu-flow event types used by the application.
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
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

// PUBLIC KEY ONLY — safe to embed, cannot generate signatures.
//
// Key rotation 2026-10-08: the previous public key (3b6a27bc…) belonged to a development
// keypair whose private key was committed to this public repository. It was REVOKED here:
// any serial signed with it is now rejected (see tests::revoked_development_key_is_rejected).
// The private key of the current pair lives OUTSIDE every repository, with the licence-issuing
// tool in a private repository. Rotating again requires only replacing this constant.
const LICENSE_PUBLIC_KEY: &[u8; 32] = &[
    0xee, 0xc0, 0x02, 0xac, 0xa8, 0x60, 0x1e, 0x29, 0x10, 0x6b, 0x63, 0xce, 0xf0, 0x63, 0x4e, 0x4c,
    0x28, 0x13, 0x25, 0xb2, 0xea, 0xf5, 0x74, 0xb6, 0x69, 0xcd, 0x1c, 0xb7, 0x3c, 0x81, 0x21, 0x41,
];

fn get_verifying_key() -> VerifyingKey {
    VerifyingKey::from_bytes(LICENSE_PUBLIC_KEY).expect("Invalid public key")
}

/// Check `serial` against `machine_id` using the embedded public key. Returns `LicenseStatus::Licensed` with `Tier::Pro` or `Tier::Edu` if the serial is a valid signature for that machine and tier, otherwise `LicenseStatus::Invalid`.
pub fn validate_license(serial: &str, machine_id: &str) -> LicenseStatus {
    validate_license_with_key(serial, machine_id, &get_verifying_key())
}

/// Same check with an explicit verifying key (lets tests exercise the full path without the
/// production private key, and lets a revoked key be proven rejected).
fn validate_license_with_key(
    serial: &str,
    machine_id: &str,
    verifying_key: &VerifyingKey,
) -> LicenseStatus {
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

/// Derive a short machine identifier: the first 6 bytes of the SHA-256 of `"{hostname}:{username}"` (from the `COMPUTERNAME`/`HOSTNAME` and `USERNAME`/`USER` environment variables), encoded as unpadded RFC 4648 base32.
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
/// Result of license validation.
pub enum LicenseStatus {
    /// No license has been applied; the application runs in trial mode.
    Trial,
    /// A valid serial was verified for the given tier.
    Licensed {
        /// Tier encoded in the verified serial.
        tier: Tier,
    },
    /// The serial failed validation.
    Invalid,
}

#[derive(Debug, Clone, PartialEq)]
/// License tier a serial can be issued for.
pub enum Tier {
    /// Pro tier.
    Pro,
    /// Edu tier.
    Edu,
}

impl LicenseStatus {
    /// Returns `true` for `Licensed` and `Trial`, `false` for `Invalid`.
    pub fn allows_export(&self) -> bool {
        matches!(self, Self::Licensed { .. } | Self::Trial)
    }

    /// Returns `true` only for `Trial`.
    pub fn watermark_renders(&self) -> bool {
        matches!(self, Self::Trial)
    }

    /// Short text for the status, e.g. "NAT3D Pro", "NAT3D Edu", "Trial (30 days)" or "Unlicensed".
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
/// Event sent over a channel by `start_edu_oauth_flow` and consumed by the application's edu-verification UI.
pub enum EduFlowEvent {
    /// A device code was obtained and the user must visit the verification address.
    DeviceCodeReady {
        /// Code the user has to enter.
        user_code: String,
        /// Address where the code is entered; the UI opens it when the `file-dialog` feature is enabled.
        verification_uri: String,
        /// Lifetime value reported with the device code (not read by the UI in this crate).
        expires_in: u64,
    },
    /// The account was confirmed as an edu account; the event carries a serial.
    EduConfirmed {
        /// Serial for the confirmed account; the UI validates it with `validate_license`.
        serial: String,
        /// Handle of the confirmed account.
        github_handle: String,
    },
    /// The account was checked and is not an edu account.
    NotEduAccount {
        /// Handle of the account that was rejected.
        github_handle: String,
    },
    /// Edu verification is not configured; the UI reports that verification via GitHub was retired.
    NotConfigured,
    /// The flow failed with the contained message.
    Error(String),
}

/// Start the edu verification flow. In this version it only sends `EduFlowEvent::NotConfigured` on `tx`; `_machine_id` is unused.
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

    // ── Signing helpers: tests sign with THROWAWAY keys only, never with a production key ──

    use ed25519_dalek::{Signer, SigningKey};

    fn serial_for(sk: &SigningKey, machine_id: &str, tier: &str) -> String {
        let msg = format!("NAT3D|{machine_id}|{tier}");
        let sig = sk.sign(msg.as_bytes());
        base32::encode(Alphabet::Rfc4648 { padding: false }, &sig.to_bytes())
    }

    #[test]
    fn valid_serials_verify_for_both_tiers_with_a_test_key() {
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        let vk = sk.verifying_key();
        assert_eq!(
            validate_license_with_key(&serial_for(&sk, "MACH1", "pro"), "MACH1", &vk),
            LicenseStatus::Licensed { tier: Tier::Pro }
        );
        assert_eq!(
            validate_license_with_key(&serial_for(&sk, "MACH1", "edu"), "MACH1", &vk),
            LicenseStatus::Licensed { tier: Tier::Edu }
        );
    }

    #[test]
    fn serial_is_bound_to_machine_and_to_the_key() {
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        let other = SigningKey::from_bytes(&[8u8; 32]);
        let vk = sk.verifying_key();
        let s = serial_for(&sk, "MACH1", "pro");
        assert_eq!(
            validate_license_with_key(&s, "MACH2", &vk),
            LicenseStatus::Invalid
        );
        assert_eq!(
            validate_license_with_key(&s, "MACH1", &other.verifying_key()),
            LicenseStatus::Invalid
        );
    }

    #[test]
    fn revoked_development_key_is_rejected() {
        // The all-zero seed was the development key whose private half was published in this
        // repository. Serials it signs MUST NOT validate against the embedded production key.
        let revoked = SigningKey::from_bytes(&[0u8; 32]);
        assert_eq!(
            revoked.verifying_key().to_bytes()[..4],
            [0x3b, 0x6a, 0x27, 0xbc],
            "sanity: this is the revoked key"
        );
        for tier in ["pro", "edu"] {
            let forged = serial_for(&revoked, "ANYMACHINE", tier);
            assert_eq!(
                validate_license(&forged, "ANYMACHINE"),
                LicenseStatus::Invalid
            );
        }
        assert_ne!(*LICENSE_PUBLIC_KEY, revoked.verifying_key().to_bytes());
    }
}
