# OpenStruct roadmap

This is the public plan for evolving OpenStruct from a linear-static frame tool
into a dependable open structural analysis platform. An item is **done** only
when its verification exists.

Status legend: **done**, **in progress**, **next**, **planned**.

## 0.1 — Public baseline — done

- 3D frame/truss stiffness solver with releases, partial fixity, support
  springs, rigid end offsets and inert-joint handling.
- Stories, grids, reference planes, area objects and editing commands.
- Load patterns, static cases, frame/area/temperature loads, and linear-add,
  envelope, ABS, SRSS and CQC combinations.
- `.e2k` text import/export with an explicit count-and-warning report.
- Steel (AISC 360, EN 1993-1-1, IS 800) and concrete (ACI 318, EN 1992-1-1,
  IS 456) interaction checks; HTML reports and CSV tables.
- Windows and Linux build definitions, CI and a tagged release workflow.

## 0.2 — Solver credibility and performance — next

**Priority:** highest

- [ ] Replace the dense global LU solve with a sparse Cholesky/LDLᵀ solver and
      symbolic reuse across load cases. Target: 50,000 joints and 33
      combinations under 60 s on a 4-core laptop.
- [ ] Add rigid-diaphragm condensation and verify it against a hand calculation.
- [ ] Implement P-Delta as a documented iterative geometric-stiffness solve, or
      remove the stored-only flag and report it as unsupported.
- [ ] Modal analysis with mass participation, benchmarked against analytic
      SDOF/2DOF cases.
- [ ] Response-spectrum analysis with CQC/SRSS modal combination.
- [ ] Equilibrium checks in CI for every sample model: reaction sums and
      free-body residual norms.
- [ ] Publish at least three textbook verification comparisons in
      `docs/VERIFICATION.md`.

**Exit criteria:** sparse and dense solvers agree on every regression model;
the 50k-joint benchmark meets the target; P-Delta and modal results have
analytic verification; reactions balance within 1e-6 relative tolerance.

## 0.3 — Shells and realistic load paths — next

**Priority:** high

- [ ] Add 4-node shell elements (membrane plus bending) and connect them to the
      frame solver.
- [ ] Replace the perimeter-line-load approximation for area pressures with
      shell load transfer; keep the current path as a documented fallback.
- [ ] Slab and wall design results: moments, in-plane forces, reinforcement
      zones.
- [ ] Area stress/contour results and table-registry entries.
- [ ] Optional two-way slab load distribution to supporting beams, clearly
      labelled approximate.

**Exit criteria:** a simply supported slab patch matches plate theory within 5 %
at mesh convergence; a slab-on-beam model reproduces tributary reactions;
shell results appear in tables and the 3D view.

## 0.4 — Modelling and design workflow — planned

- [ ] Rebar definition per member and a real section-design pass, including bar
      selection and spacing checks.
- [ ] Ductile detailing rules and a bar-bending schedule exported to CSV/PDF.
- [ ] Steel connection checks with a connection library.
- [ ] Composite beam and shear-wall workflows completed and verified.
- [ ] Code load-combination generators (ASCE 7, EN, IS 1893/NBC) with editable
      factors and load-type mapping.
- [ ] Auto wind and seismic load patterns, clearly separated from imported
      cases.
- [ ] XLSX export and PDF reports to replace the stubs.
- [ ] Full table registry with named sets, filters and import/export.

## 0.5 — Interoperability and automation — planned

- [ ] Complete `.e2k` round-trip for the records currently written; add property
      modifiers, diaphragms, groups and load patterns to the writer.
- [ ] IFC import/export for geometry and structural members.
- [ ] Python bindings for the C ABI and a documented batch CLI.
- [ ] Plugin boundary for solvers and design codes.
- [ ] Model diff/merge and project templates.

## 1.0 — Release quality — planned

- [ ] Signed Windows MSI and Linux AppImage/deb packages in the release
      workflow.
- [ ] Reproducible builds and published SHA-256 checksums.
- [ ] Accessibility and keyboard-navigation pass on the desktop UI.
- [ ] Crash-free/fuzz testing for every text importer (`.e2k`, CSV, JSON).
- [ ] Performance budgets enforced in CI (model open, solve, 3D frame time).
- [ ] Complete user guide, verification manual and tutorial models.
- [ ] Security review of file parsers and HTML report output.

## Order and dependencies

1. 0.2 sparse solve and equilibrium checks unblock credible large-model work.
2. 0.3 shells depend on the sparse solver and a stable element interface.
3. 0.4 design work depends on 0.2 envelopes and 0.3 slab results.
4. 0.5 interoperability depends on stable model serialization.
5. 1.0 packaging depends on all preceding verification.

## How to contribute

Pick an unchecked item, open an issue describing the model and verification you
will use, and keep the change focused. New numerical behaviour needs a test that
fails before the change and passes after it. Update the relevant document and
this roadmap in the same pull request.
