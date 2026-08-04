// SPDX-License-Identifier: AGPL-3.0-or-later
// NAT3D license key generator — INTERNAL ONLY, never ship this binary.
//
// Uses Ed25519 asymmetric signatures:
// - This tool signs with the PRIVATE key (never distributed)
// - The app verifies with the PUBLIC key only (safe to embed in binary)
//
// The serial encodes the FULL 64-byte Ed25519 signature (base32, no padding
// -> ~104 characters). Earlier revisions truncated this to 20 bytes and
// zero-padded the rest before verifying; that is cryptographically invalid
// and was confirmed (empirically) to reject every legitimately-signed
// serial. Do not reintroduce truncation.
//
// Usage:
//   nat3d-keygen generate --machine-id ABCDEF --tier pro
//   nat3d-keygen verify  --machine-id ABCDEF --serial XXXXXXXXXXXX
//   nat3d-keygen machine-id   (prints local machine ID)
//   nat3d-keygen gen-keypair  (one-time: generate new keypair)
//
// The private key is read from NAT3D_LICENSE_PRIVKEY env var (hex-encoded).
// If not set, falls back to the embedded development key (INSECURE for production).

use base32::Alphabet;
use clap::{Parser, Subcommand};
use ed25519_dalek::{Signer, SigningKey, Verifier};
use sha2::{Digest, Sha256};

#[derive(Parser)]
#[command(
    name = "nat3d-keygen",
    about = "NAT3D license key generator (INTERNAL)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate a serial key for a machine+tier pair
    Generate {
        #[arg(long)]
        machine_id: String,
        #[arg(long, default_value = "pro", value_parser = ["pro", "edu"])]
        tier: String,
    },
    /// Verify a serial key against a machine ID (real Ed25519 check, not string compare)
    Verify {
        #[arg(long)]
        machine_id: String,
        #[arg(long)]
        serial: String,
    },
    /// Print this machine's ID (matches what NAT3D shows in the License dialog)
    MachineId,
    /// Generate a new Ed25519 keypair (one-time setup)
    GenKeypair,
}

// Development-only private key — REPLACE via NAT3D_LICENSE_PRIVKEY env var for production.
// Generate your production key with: nat3d-keygen gen-keypair
const DEV_PRIVATE_KEY_HEX: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

fn get_signing_key() -> SigningKey {
    let hex_key = std::env::var("NAT3D_LICENSE_PRIVKEY").unwrap_or_else(|_| {
        eprintln!("WARNING: Using development key. Set NAT3D_LICENSE_PRIVKEY for production.");
        DEV_PRIVATE_KEY_HEX.to_string()
    });
    let bytes = hex::decode(&hex_key).expect("NAT3D_LICENSE_PRIVKEY must be valid hex (64 chars)");
    let bytes: [u8; 32] = bytes.try_into().expect("Private key must be 32 bytes");
    SigningKey::from_bytes(&bytes)
}

/// Serial = base32(full 64-byte Ed25519 signature). No truncation.
fn generate_serial(machine_id: &str, tier: &str) -> String {
    let signing_key = get_signing_key();
    let message = format!("NAT3D|{machine_id}|{tier}");
    let signature = signing_key.sign(message.as_bytes());
    base32::encode(Alphabet::Rfc4648 { padding: false }, &signature.to_bytes())
}

fn get_machine_id() -> String {
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

fn format_serial(serial: &str) -> String {
    serial
        .chars()
        .enumerate()
        .fold(String::new(), |mut s, (i, c)| {
            if i > 0 && i % 4 == 0 {
                s.push('-');
            }
            s.push(c);
            s
        })
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Generate { machine_id, tier } => {
            let serial = generate_serial(&machine_id, &tier);
            println!("Machine : {machine_id}");
            println!("Tier    : {tier}");
            println!("Serial  : {}", format_serial(&serial));

            let signing_key = get_signing_key();
            let public_key = signing_key.verifying_key();
            println!(
                "\nPublic key (embed in app): {}",
                hex::encode(public_key.as_bytes())
            );
        }
        Command::Verify { machine_id, serial } => {
            let serial_clean = serial.trim().to_uppercase().replace('-', "");
            let signing_key = get_signing_key();
            let verifying_key = signing_key.verifying_key();

            let sig_bytes =
                match base32::decode(Alphabet::Rfc4648 { padding: false }, &serial_clean) {
                    Some(bytes) if bytes.len() == 64 => bytes,
                    _ => {
                        println!("❌ INVALID — serial is not a well-formed 64-byte signature");
                        std::process::exit(1);
                    }
                };
            let mut sig_array = [0u8; 64];
            sig_array.copy_from_slice(&sig_bytes);
            let signature = ed25519_dalek::Signature::from_bytes(&sig_array);

            for tier in &["pro", "edu"] {
                let message = format!("NAT3D|{machine_id}|{tier}");
                if verifying_key.verify(message.as_bytes(), &signature).is_ok() {
                    println!("✅ VALID — machine={machine_id} tier={tier}");
                    return;
                }
            }
            println!("❌ INVALID — serial does not match machine {machine_id}");
            std::process::exit(1);
        }
        Command::MachineId => {
            println!("{}", get_machine_id());
        }
        Command::GenKeypair => {
            use rand_core::OsRng;
            let signing_key = SigningKey::generate(&mut OsRng);
            let verifying_key = signing_key.verifying_key();

            println!("=== NAT3D License Keypair (SAVE SECURELY) ===\n");
            println!("PRIVATE KEY (set as NAT3D_LICENSE_PRIVKEY env var, NEVER commit):");
            println!("{}\n", hex::encode(signing_key.to_bytes()));
            println!("PUBLIC KEY (embed in crates/nat3d-app/src/license.rs):");
            println!("{}\n", hex::encode(verifying_key.as_bytes()));
            println!("=== Instructions ===");
            println!("1. Save the private key securely (password manager, HSM, etc.)");
            println!("2. Set NAT3D_LICENSE_PRIVKEY=<private_key> when running keygen");
            println!("3. Update LICENSE_PUBLIC_KEY in license.rs with the public key");
        }
    }
}
