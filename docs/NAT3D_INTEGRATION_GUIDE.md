# NAT3D Integration Guide (implemented API)

This guide describes the APIs present in this workspace. Scientific integrations
such as Koine, NSTQL and HumanBody are project-specific, not an automatically
working connection just because both repositories exist.

## 1. Integrating a simulation

- Depend on `nat3d-core` for scene and geometry structures; use the actual
  `nat3d-sync` protocol modules for application-specific transport.
- Convert your simulation mesh or other geometry into NAT3D's documented
  scene/geometry representation. Preserve units, timestamps, provenance and
  coordinate systems in your integration adapter.
- Use an explicit compatibility/integration test for each simulator.

**Not yet provided**: this workspace does not contain a `nat3d-interface`
crate, a `CompartmentVertex` interface contract, or a `NeuralVisualizer`
trait. Do not add these names to consuming projects until an implementation
is merged. This document does not promise that the previously proposed
`SyncMessage::PencilUpdate` is a universal scientific interchange format.

## 2. Secure project payloads

`nat3d_io::secure_save(data, key)` now returns `Result<Vec<u8>, String>`
and encrypts using AES-256-GCM, with a random nonce per call.
`nat3d_io::secure_load(payload, key)` authenticates and decrypts this format.
Both functions reject failure instead of silently returning unencrypted bytes.
Failure to obtain a random nonce from the operating system also returns `Err`.

These are **byte payload utilities**, not automatic filesystem encryption.
Callers must securely manage their keys, select appropriate destinations,
handle errors, and avoid storing plaintext in ancillary files.
The on-disk format starts with `NAT3DENC`, version byte 1 and a 12-byte
nonce, followed by ciphertext and authentication tag. Legacy plaintext is
not accepted by `secure_load`.

`nat3d_io::crypto::stream_cipher::{encrypt_asset_stream, decrypt_asset_stream}`
wrap these same authenticated payload operations. Both accept
`data: &mut Vec<u8>` and `key: &[u8; 32]`, and return `Result<(), String>`.
They replace the buffer only on success, leaving it unchanged on error.
Encryption adds 37 bytes: the 9-byte header, 12-byte nonce and 16-byte tag.
Despite their names, these functions process the entire buffer rather than
encrypting or decrypting incremental chunks.

**Migration:** the former XOR API accepted mutable slices and keys of arbitrary
length and returned no result. Callers must now supply a resizable `Vec<u8>`,
a 32-byte key and handle the returned `Result`. Legacy XOR payloads are not
compatible with the authenticated format and must be migrated separately;
the decrypt functions do not fall back to XOR or plaintext.

## 3. Validation before production

- Build and test the relevant Rust workspace targets.
- Run encrypted save/load roundtrip, wrong-key and tamper tests.
- Validate the actual scientific adapter against known simulation fixtures.
- Review storage, key management, protocol versioning and scene provenance.
- Generate an SBOM with the dependency tooling available in your own build
  environment; this guide does not claim an audited SBOM script exists.

The system's CAD, rendering, and scientific simulation components have
separate validation requirements. Integrating them does not automatically
certify physical correctness.
