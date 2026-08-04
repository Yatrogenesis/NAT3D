# NAT3D

[![DOI](https://zenodo.org/badge/DOI/10.5281/zenodo.21796068.svg)](https://doi.org/10.5281/zenodo.21796068)
[![CI](https://github.com/Yatrogenesis/NAT3D/actions/workflows/ci.yml/badge.svg)](https://github.com/Yatrogenesis/NAT3D/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-AGPL--3.0--or--later%20OR%20Commercial-blue.svg)](LICENSE-AGPL)

Professional 3D modeling, CAD, physics simulation, and rendering suite. Written in Rust.

## Overview

NAT3D provides integrated tools for 3D content creation, engineering simulation, and production rendering. The architecture prioritizes correctness, performance, and scientific accuracy.

### Capabilities

- **Modeling**: Polygon mesh editing, NURBS surfaces, boolean CSG, subdivision
- **Physics**: Navier-Stokes fluids, cloth, rigid/soft body, N-body particle systems
- **Rendering**: Real-time GPU viewport (wgpu), offline path tracing, PBR materials
- **Animation**: Keyframe, skeletal rigging, inverse kinematics, shape keys
- **I/O**: glTF, FBX, OBJ, DXF, native binary format
- **Scripting**: Python API via PyO3
- **Distributed**: LAN render farm, iPad/stylus input over TCP

## Requirements

- Rust 1.85+ (edition 2024 dependencies)
- GPU with Vulkan, Metal, or DirectX 12 support
- Windows 10+, macOS 13+, or Linux with Wayland/X11

## Building

```bash
git clone https://github.com/Yatrogenesis/NAT3D.git
cd NAT3D

# Release build
cargo build --release -p nat3d-app

# Run
./target/release/nat3d-app    # Linux/macOS
target\release\nat3d-app.exe  # Windows

# Tests
cargo test --workspace
```

## Architecture

```
crates/
├── nat3d-core        Core geometry, scene graph, selection
├── nat3d-math        Linear algebra, splines, noise (YatroSci integration)
├── nat3d-physics     Fluid dynamics, cloth, particles, FEM
├── nat3d-render      wgpu GPU backend, ray tracing, post-processing
├── nat3d-modeling    Mesh operations, modifiers, sculpt, CAD
├── nat3d-animation   Keyframes, rigging, IK, motion paths
├── nat3d-io          File format import/export
├── nat3d-scripting   Python bindings, Rust command API
├── nat3d-ui          egui interface, viewport, panels
├── nat3d-sync        Device sync, render farm, stylus input
├── nat3d-cloud       Remote rendering integration
├── nat3d-mobile      iOS/Android native wrappers
├── nat3d-cli         Command-line tools
├── nat3d-app         Main desktop application
└── nat3d-keygen      License key generation
```

## Dependencies

External libraries from crates.io:

- wgpu 23.0 (GPU abstraction)
- egui/eframe 0.30 (UI framework)
- nalgebra 0.33 (linear algebra)
- yatrosci-* 0.1.0 (scientific computing stack)

## Installer

Windows installer scripts are provided in `installer/windows/`. See `installer/README.md` for instructions.

## License

NAT3D is **dual-licensed** — choose one:

- **AGPL-3.0-or-later** (`LICENSE-AGPL`) — open-source, academic, and non-commercial
  use. Network use requires disclosing your source under the same license.
- **Commercial** (`LICENSE-COMMERCIAL`) — proprietary/closed-source deployment
  without AGPL source-disclosure obligations, under a signed agreement.

SPDX: `AGPL-3.0-or-later OR LicenseRef-Commercial`

Commercial licensing contact: fmolina@avermex.com

## Author

Francisco Molina-Burgos, Avermex Research Division
