# Local desktop build recipe fails with a relative target directory

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The desktop build recipe could split artifacts across relative target directories.

## Details

F195 — **possible** — `crates/farhelm-desktop/README.md:45` — Local desktop build recipe fails with a relative target
directory

The two commands run from different working directories but do not export the normalized absolute target directory
described by the recipe. With an exported relative target setting and cwd-relative Dioxus resolution, they can use
different trees and embedding fails. That CLI-resolution premise was not independently traced and no build ran. Export
the normalized absolute directory before either build; default and absolute settings are outside this finding.

## Evidence and triage context

- crates/farhelm-desktop/README.md:44 normalizes TARGET but never exports that absolute value as CARGO_TARGET_DIR.
- crates/farhelm-desktop/README.md:47 runs the web build from the UI crate and the desktop build from the repository
  root.
- crates/farhelm-helm/build.rs:113 refuses an embedding directory without index.html.
- crates/farhelm-desktop/README.md:56 explicitly explains why these two builds must receive the same absolute target
  directory.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- The plain-Cargo compile-check warning concerns a different recipe. No matching queue or ledger coverage found.

Caveats:

- Requires an exported relative CARGO_TARGET_DIR.
- No build was run and Dioxus CLI internals were not independently traced.
- The default and absolute target-directory cases are not established as defective.
- No build or installed Dioxus source verification.
- Default and absolute-directory cases are outside this finding.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_01_cor:p1:F3`.

- `ui_desktop_01_cor:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.
