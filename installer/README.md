# NAT3D Installer Scripts

Local installer generation. No external services required.

## Windows

### Prerequisites
- Rust toolchain (https://rustup.rs)
- Inno Setup 6 (https://jrsoftware.org/isinfo.php)

### Build

Option A: Automated script
```batch
cd G:\NAT3D
installer\windows\build-installer.bat
```

Option B: Manual
```batch
cargo build --release -p nat3d-app
```
Then open `installer\windows\nat3d.iss` in Inno Setup and compile.

Output: `target\installer\NAT3D-0.1.0-Setup.exe`

### Installer contents
- Application binary (~19 MB)
- Desktop shortcut (optional)
- Start menu entry
- File association for `.nat` files
- Uninstaller

## macOS

```bash
cargo install cargo-bundle
cargo bundle --release -p nat3d-app
```

Output: `target/release/bundle/osx/NAT3D.app`

## Linux

```bash
cargo build --release -p nat3d-app
tar czf NAT3D-linux-x86_64.tar.gz -C target/release nat3d-app
```

## Code signing (optional)

### Windows
Certificate required ($70-200/year from Sectigo, DigiCert, etc.). Without signing, Windows SmartScreen displays a warning that users can bypass.

### macOS
Apple Developer Program required ($99/year) for notarization. Without signing, users must right-click and select "Open" on first launch.
