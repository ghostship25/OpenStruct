# Architecture

OpenStruct is a Cargo workspace with a pure-Rust analysis crate and an egui
desktop shell.

```text
crates/openstruct/src/
  model.rs            nodes, elements, areas, stories, groups, diaphrams
  elements.rs         frame element and releases
  sections.rs         section properties and validation
  materials.rs        material families and unit-aware densities
  loads/              patterns, cases, combinations
  fem.rs              local stiffness, transformation, consistent loads
  analysis.rs         assembly, static solve, combinations, envelopes
  solver.rs           linear algebra helpers
  e2k.rs              E2K text import and report
  e2k_text.rs         native E2K text round-trip
  io.rs               JSON model I/O
  design/             steel, concrete, composite, connections, rebar
  reporting.rs        HTML reports
  main.rs             headless CLI
apps/openstruct-gui/src/
  app.rs              application state and commands
  menu.rs, toolbar.rs, explorer.rs, properties.rs, tables*.rs
  canvas.rs, wgpu3d.rs  2D plan and 3D viewport
  dialogs/            model, load, design and report dialogs
```

## Solver

The solver assembles a global stiffness matrix from frame elements, applies
support restraints and springs, and solves the reduced dense system. Member
releases and partial fixity are condensed locally before assembly. Joints that
belong only to area objects, or to frame islands with no support, are excluded
from the reduced solve and reported in the status text; this prevents a model
with unmodelled shell geometry from failing as singular.

Area pressures are transferred to perimeter frame members as uniform line loads.
This is an explicit approximation until shell elements are added.

## Units

The core stores SI: metres, newtons, pascals, kilograms. Display units are a
conversion layer in `units.rs`; parsing accepts quantities such as `5 ft`,
`3000 mm`, `12 kN/m` and `2 kPa`.

## State and persistence

A model owns its nodes, elements, areas, materials, sections, stories, grids,
load patterns, cases and combinations. Native `.ostrut` files are JSON and are
versioned by the crate version. Undo/redo is command-based in the desktop app.
