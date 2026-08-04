# NAT3D Integration Guide

NAT3D is engineered as a standalone SOTA visualization engine. This guide outlines how to integrate scientific projects (e.g., HumanBody, NSTQL) with NAT3D.

## 1. The "Clone-and-Integrate" Model
1.  **Clone NAT3D:** Add NAT3D as a git submodule in your project's `external/` directory.
2.  **Interface Dependency:** Add `nat3d-interface` to your `Cargo.toml`. This crate is lightweight and contains only the shared data structures (e.g., `CompartmentVertex`).
3.  **Data Flow:**
    *   Implement your simulation loop.
    *   Map your domain data (e.g., neuronal voltages, chemical properties) into the `CompartmentVertex` struct.
    *   Use the `nat3d-sync` protocol to send a `SyncMessage::PencilUpdate` or raw buffer data to NAT3D.

## 2. Security & Compliance
*   **Encrypted Storage:** Use `nat3d-io` for saving your project files. It utilizes AES-256-GCM by default.
*   **SBOM:** Execute `scripts/generate_sbom.sh` in the NAT3D root to generate a verified cyclonedx-cargo manifest for your integrated project.

## 3. UI Integration (Visual Viewport API)
To control your simulation parameters from the NAT3D viewport:
*   Define a `VisConstants` struct in your project.
*   Implement the `NeuralVisualizer` trait from `nat3d-interface` to feed these constants to NAT3D's shaders.

*For further support, see the repository documentation or open an issue.*
