# Development phases

A short record of how OpenStruct reached its current baseline. This is project
history, not a promise of vendor compatibility.

## Phase 1 — Core model and linear solver

Nodes, frame elements, materials, rectangular and standard sections, supports,
joint loads and distributed loads. The direct stiffness solver was verified
against simply supported beams (reactions, midspan deflection, triangular
consistent moments).

## Phase 2 — Building geometry and desktop shell

Stories, grids, reference planes, area objects, 2D plan and 3D viewport, model
explorer, properties panel and command-based undo/redo. Unit-aware display and
parsing were added across the UI.

## Phase 3 — Loads, combinations and tables

Load patterns with a reference catalogue, static load cases (self weight and a
stored P-Delta flag), frame/area/temperature loads, and linear-add, envelope,
ABS, SRSS and CQC combinations. Result tables, envelope tables, CSV export and
HTML reports were added. Materials and sections became editable libraries.

## Phase 4 — Import, interoperability and design checks

`.e2k` text import/export, native JSON models, area-load transfer to perimeter
frames, release/partial-fixity condensation, support springs, rigid end offsets
and steel/concrete interaction checks for several design codes.

## Phase 5 — Hardening

Vertical-member rotation, unique element IDs, inert-joint handling, empty-system
solves and combination caching were fixed with regression tests. Remaining work
is tracked in [ROADMAP.md](ROADMAP.md).

## Current baseline

- 207 core library tests pass on Windows.
- The GUI crate compiles on Windows.
- CI is defined for Windows and Linux; local release binaries are produced by
  the scripts in `scripts/`.
