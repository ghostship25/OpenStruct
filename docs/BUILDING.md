# Building and packaging

## Prerequisites

- Rust stable 1.85 or newer (`rustup toolchain install stable`).
- Windows: Visual Studio Build Tools with the **Desktop development with C++**
  workload (MSVC linker).
- Linux: a C toolchain plus the native window/OpenGL development packages
  listed below.
- No proprietary structural package or binary file is needed.

## Windows

```powershell
cargo build --release --locked -p openstruct -p openstruct-gui
```

Outputs:

```text
target\release\openstruct-gui.exe   desktop application
target\release\openstruct.exe       headless CLI
target\release\openstruct.dll       C ABI library
```

Run the application:

```powershell
.\target\release\openstruct-gui.exe
```

`scripts/build-windows.ps1` builds both crates and copies the binaries plus a
`SHA256SUMS.txt` into `builds/windows/`.

## Linux

```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libxkbcommon-dev libwayland-dev \
  libx11-xcb-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libgl1-mesa-dev libfontconfig1-dev
cargo build --release --locked -p openstruct -p openstruct-gui
```

Outputs are `target/release/openstruct-gui` and `target/release/openstruct`;
the C ABI is `target/release/libopenstruct.so`.

`scripts/build-linux.sh` performs the build and copies the binaries plus a
SHA-256 file into `builds/linux/`.

## Tests

```bash
cargo test --workspace --locked
```

GUI tests drive `egui::Context` directly, so most do not need a display. If a
GUI test needs one on Linux, run it under `xvfb-run`.

## Release artifacts

`.github/workflows/release.yml` builds on `windows-latest` and `ubuntu-latest`,
runs the test suite, packages the binaries and attaches them to a tagged GitHub
release. Local helpers:

```powershell
.\scripts\build-windows.ps1
```

```bash
./scripts/build-linux.sh
```

## Reproducibility

`Cargo.lock` is committed. Build with the lockfile in place; CI uses `--locked`
so dependency updates are explicit pull requests.
