# Changelog

## 0.1.0 — 2026-11-13

First public snapshot.

- Standalone workspace with the `openstruct` engine and `openstruct-gui`
  desktop app.
- Linear-static 3D frame/truss solver with releases, partial fixity, support
  springs, rigid end offsets and inert-joint reporting.
- Stories, grids, reference planes and area objects.
- Load patterns, static cases, frame/area/temperature loads, and linear-add,
  envelope, ABS, SRSS and CQC combinations with cached case results.
- `.e2k` text import with an explicit counts/warnings report; a readable E2K
  subset export; JSON and CSV I/O.
- Steel (AISC 360, EN 1993-1-1, IS 800) and concrete (ACI 318, EN 1992-1-1,
  IS 456) interaction checks.
- Fixed during verification: transposed member rotation for vertical members,
  frame loads missing from assembly, duplicate element IDs, area loads not
  applied, repeated dense combination solves, and the zero-DOF solve panic.
- Windows/Linux build scripts, CI and tagged release workflow.
