# nat3d

Umbrella crate for the NAT3D 3D modeling, simulation and rendering stack.
One dependency brings in every NAT3D library crate:

```toml
[dependencies]
nat3d = "0.2"
```

```rust
use nat3d::math;
use nat3d::core;
```

Optional features: `python`, `ipad`, `file-dialog`, `mobile`, `full`.
Nothing is enabled by default, so it builds with Rust alone.

This is a library. To install the programs: `cargo install nat3d-cli` (binary
`nat3d`) or `cargo install nat3d-app`.

Source and issues: <https://github.com/Yatrogenesis/NAT3D>

Licence: `AGPL-3.0-or-later OR LicenseRef-Commercial`.
