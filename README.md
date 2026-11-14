# OpenStruct

OpenStruct is a free, open-source structural analysis application for 3D frames,
trusses and building models. It pairs a Rust analysis engine with a desktop
modelling interface, imports and exports `.e2k` text models, evaluates load cases
and combinations, and performs steel and concrete member checks.

The project is intended for students, enthusiasts and developers who want a
readable, hackable structural analysis tool rather than a closed black box.

**Status:** early public snapshot (`0.1.0`). The linear-static frame workflow is
usable. The gaps listed below are real, documented and tracked in the roadmap.

## What works

- **3D frame and truss analysis** with the direct stiffness method, member
  releases, partial fixity, support springs and rigid end offsets.
- **Stories, grids and area objects.** Slabs and walls are stored and displayed;
  area pressures are transferred to perimeter frame members (see limitations).
- **Materials and sections:** steel, concrete and aluminium materials;
  rectangular, circular, I/wide-flange, channel, tee, box, pipe and angle
  sections; a section designer with composite polygons and holes.
- **Loads:** load patterns, static load cases (self weight and a stored P-Delta
  flag), frame and area loads, temperature/strain loads, and linear-add,
  envelope, ABS, SRSS and CQC combinations.
- **`.e2k` text import and export** for the records listed in
  [`docs/E2K-FORMAT.md`](docs/E2K-FORMAT.md), plus native JSON models and CSV
  tables.
- **Design checks:** AISC 360 / EN 1993-1-1 / IS 800 steel interaction and
  ACI 318 / EN 1992-1-1 / IS 456 concrete interaction, driven by result
  envelopes.
- **Desktop UI:** 2D plan and 3D views, model explorer, tables, HTML reports and
  a headless CLI.

## Known limitations

These are deliberate, documented gaps — not hidden behaviour:

- No shell/slab finite elements yet: area pressures are approximated as uniform
  line loads on perimeter frames, and area stress/contour results are not
  available.
- Linear static only. Modal, response-spectrum, staged-construction, moving-load,
  nonlinear and P-Delta solutions are not implemented (the P-Delta flag is stored
  but not solved).
- Rigid-diaphragm constraints are model data only; they are not condensed into
  the stiffness matrix.
- Auto wind and seismic loads are not generated. Imported cases with no exported
  loads are reported as warnings and solve to zero.
- Steel and concrete checks are simplified. Concrete assumes 1 % reinforcement
  and 0.9 h effective depth; there is no rebar layout or detailing design.
- XLSX export and PDF reports are stubs; CSV and HTML reports work.
- Some import records are reported as skipped rather than approximated.

## Download and run

Prebuilt binaries live in [`builds/`](builds/):

- **Windows:** [`builds/windows/OpenStruct.exe`](builds/windows/OpenStruct.exe)
  (desktop) and `openstruct-cli.exe` (headless).
- **Linux:** [`builds/linux/OpenStruct`](builds/linux/OpenStruct) (desktop) and
  `openstruct-cli` (headless), built by the `v0.1.0` release workflow on
  `ubuntu-latest`.

The Linux desktop build needs a working X11/Wayland session and OpenGL drivers.
Rebuild either package with the scripts in [`scripts/`](scripts) or the release
workflow; see [`docs/BUILDING.md`](docs/BUILDING.md).

## Build from source

```bash
cargo build --release --locked -p openstruct -p openstruct-gui
```

Windows output: `target/release/openstruct-gui.exe`, `target/release/openstruct.exe`,
`target/release/openstruct.dll`.
Linux output: `target/release/openstruct-gui`, `target/release/openstruct`,
`target/release/libopenstruct.so`.

Linux also needs the native window/OpenGL development packages listed in
[`docs/BUILDING.md`](docs/BUILDING.md).

## Command line

```bash
# Import an E2K text model, solve the first case and every combination
cargo run --release -p openstruct -- model.e2k --combinations

# Open a native JSON model
cargo run --release -p openstruct -- model.json
```

## Repository layout

```text
crates/openstruct/       analysis engine, E2K/JSON I/O, design checks, CLI
apps/openstruct-gui/     desktop application (egui/eframe)
builds/                  packaged binaries and SHA-256 checksums
docs/                    architecture, format notes, build guide, roadmap
scripts/                 Windows and Linux build helpers
.github/workflows/       CI and tagged release builds
```

## Documentation

- [Architecture](docs/ARCHITECTURE.md)
- [Building and packaging](docs/BUILDING.md)
- [E2K text format support](docs/E2K-FORMAT.md)
- [Roadmap](docs/ROADMAP.md)
- [Development phases](docs/PHASE.md)
- [Contributing](CONTRIBUTING.md)
- [Changelog](CHANGELOG.md)

## Tests

```bash
cargo test --workspace --locked
```

The suite covers frame stiffness, vertical members, area-load transfer, load
combinations, E2K round-trips, section properties, editing and design checks.

## Name

The application is **OpenStruct**, as chosen by the project owner. The name is
also used by a Ruby standard-library class; if a distinctive public brand is
preferred later, **OpenStrut** and **StructForge** are available alternatives.
The crates and binaries use `openstruct`, so a rename is mechanical.

## License

MIT — free for personal, academic and commercial use, modification and
redistribution. See [LICENSE](LICENSE).
