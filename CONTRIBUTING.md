# Contributing

Thanks for improving OpenStruct. This is an engineering tool, so correctness and
honest documentation matter more than feature count.

## Before you start

- Read [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) and the relevant item in
  [`docs/ROADMAP.md`](docs/ROADMAP.md).
- For a numerical change, describe the model, the expected result and the
  verification source in the issue.
- Do not add proprietary code, icons, fonts, manuals, binaries or sample files.
  Use your own model or a synthetic one.

## Workflow

1. Fork and branch from `main` (`feature/…`, `fix/…`, `docs/…`).
2. Keep one logical change per pull request.
3. Add or update a test that fails before the change.
4. Run:

   ```bash
   cargo fmt --all
   cargo test --workspace --locked
   cargo check --workspace --locked
   ```

5. Update the docs the change makes stale — especially
   `docs/E2K-FORMAT.md` if an import record or solver behaviour changes.
6. Open a pull request with what changed, how it was verified, and what remains
   unverified.

## Code standards

- Safe Rust; avoid `unwrap`/`expect`/`panic!` on input-derived values in library
  code.
- Keep analysis logic in `crates/openstruct`; the GUI stays a client.
- SI units in the model; convert only for display/parsing.
- New public behaviour needs a test; new limitations go in the README and the
  format document, not in a comment only.

## Reporting bugs

Include the OpenStruct version, OS, the smallest model or `.e2k` that reproduces
the problem, the command or dialog used, and the exact output. Remove
confidential project data before attaching a model.
