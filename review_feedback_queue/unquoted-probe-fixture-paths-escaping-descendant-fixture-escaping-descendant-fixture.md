# Unquoted probe-fixture paths — escaping-descendant fixture

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A space in the temporary root could redirect this fixture's write outside its directory.

## Details

F86 — **possible** — `crates/farhelm-supervisor/src/tmux.rs:4707` — Unquoted probe-fixture paths — escaping-descendant
fixture

The escaping-descendant fixture interpolates its PID-file path into an inner shell script without quoting it. A
legitimate temporary root containing a space makes the redirection use only the path prefix before that space. If a
writable file exists at that prefix, setup truncates it outside the fixture. This test-only sequence was not reproduced.
Quote the generated shell argument correctly or pass the path as a separate argument instead of embedding it.

## Evidence and triage context

- tmux.rs:4694–4696 derives both PID files from tempfile::tempdir. pidfile_arg at :4602–4608 rejects only apostrophes
  and returns spaces and other shell syntax unchanged. At :4707–4709 those strings enter the inner bash command without
  path quoting. probe_fixture at :4626–4628 writes the body into an executable shell script; :4715 calls probe_tmux,
  which executes it at :723–736. The outer single quotes protect argument transport to bash, not parsing by that inner
  bash. Cargo.lock:5544–5545 pins tempfile 3.27.0; its src/lib.rs:595–596 uses the default temporary root, and
  src/env.rs:41–50 delegates to std::env::temp_dir absent an override. No matching acceptance found. FILTER.md:38–42
  excludes wrong-target writes and work loss. Static trace only; not executed.
- tmux.rs:4602–4608 rejects only a single quote and otherwise returns the unchanged path.
- tmux.rs:4707 or :4767 inserts that path after an unquoted shell redirect operator; tempfile paths derive from the
  environment temporary root.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- Requires nondefault space-containing temporary root and a writable prefix-named file; no runtime reproduction.
  Test-only; no shipped-product exploit claimed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_runtime_sec_02:p1:C2`, `sr_systems:p3:C2`.

- `gap_supervisor_runtime_sec_02:p1:C2`: confidence as filed: not separately tagged in candidate list; suggested bucket
  as filed: not separately tagged in candidate list.
- `sr_systems:p3:C2`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.
