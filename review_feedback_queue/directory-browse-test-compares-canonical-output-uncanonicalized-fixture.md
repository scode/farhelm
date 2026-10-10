# The directory-browse test compares canonical output with an uncanonicalized fixture path

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Directory browsing tests fail on symlinked temporary roots.

## Details

F152 — **definite** — `crates/farhelm-supervisor/src/service/core.rs:16561` — The directory-browse test compares
canonical output with an uncanonicalized fixture path

The product returns a canonical parent directory, while the assertion compares it with the fixture's unresolved home
path. A temporary root reached through a symlink gives those paths different spellings even though they identify the
same directory. Canonicalize the expected home before comparing it with the returned parent, keeping the test focused on
browse behavior.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/core.rs:4854–4869: browsing derives parent from a canonicalized directory.
- crates/farhelm-supervisor/src/service/core.rs:16520–16561: the test canonicalizes selected for cwd but compares parent
  with home.path() directly.
- crates/farhelm-supervisor/src/service/core.rs:16520 creates a tempfile home and line 16561 compares returned parent
  with its uncanonicalized spelling.
- crates/farhelm-supervisor/src/service/core.rs:16542 correctly canonicalizes the adjacent cwd expectation.
- crates/farhelm-supervisor/src/service/core.rs:4854 canonicalizes the browsed directory and line 4864 derives its
  parent from that canonical path.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- No matching acceptance or existing item found.

Caveats:

- Requires a temporary root whose spelling differs from its canonical path.
- No runtime reproduction was performed.
- Requires a symlinked temporary-root spelling.
- Requires a temporary-root path containing a symlink component; it is not guaranteed to fail on every macOS
  configuration.
- No runtime reproduction.
- Requires a symlink component in the temporary-root path.
- Do not describe this as failing on every Mac.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_02:p1:F2`,
`gap_supervisor_state_sec_03:p1:F3`.

- `gap_supervisor_state_cor_02:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
- `gap_supervisor_state_sec_03:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.
