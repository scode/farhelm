# The upload-memory test leaks its 64 MiB fixture on successful runs

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Successful upload-memory tests leave 64 MiB files behind.

## Details

F150 — **definite** — `crates/farhelm-supervisor/src/files.rs:1570` — The upload-memory test leaks its 64 MiB fixture on
successful runs

The child exits through immediate process termination, bypassing its temporary-directory destructor. The parent never
receives the directory path, and the ordinary repository sweep does not recognize it. Each successful run can therefore
retain another completed 64 MiB upload. Make the parent own the directory, or explicitly close and remove it before the
child exits.

## Evidence and triage context

- crates/farhelm-supervisor/src/files.rs:1550–1574: the measurement child owns the TempDir, publishes the 64 MiB file,
  and calls process::exit.
- crates/farhelm-supervisor/src/files.rs:1577–1598: the parent owns only the child command and exit status.
- crates/farhelm-teststate/src/lib.rs:65–80: repository sweeping covers fh-e2e. and fh-it. prefixes.
- crates/farhelm-supervisor/src/files.rs:1550 creates a child-owned TempDir.
- crates/farhelm-supervisor/src/files.rs:1530 defines the 64 MiB payload; line 1563 publishes it.
- crates/farhelm-supervisor/src/files.rs:1570 exits the process directly before TempDir destruction; the parent at line
  1577 neither owns nor receives that directory.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- SPEC_impl.md:71–76 accepts fixed bounded leftovers, rare explicit-action growth, or leftovers removed on subsequent
  attempts/startup. None describes this repeatedly accumulated successful-test fixture.
- No matching existing disposition found. This is concrete retained disk usage, not a speculative eventual
  disk-exhaustion claim.

Caveats:

- Test infrastructure only.
- External operating-system temporary-file cleanup might eventually remove the files; repository-owned cleanup does not.
- Operating-system temporary-file cleanup may eventually remove the leftovers.
- No runtime reproduction was needed.
- External temporary-directory cleanup can eventually reclaim the file.
- No runtime reproduction was performed.
- External temporary-directory cleanup may reclaim it later.
- No runtime reproduction.
- Keep independently editable from the two RSS-measurement defects.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_01:p1:F2`,
`gap_supervisor_state_sec_01:p1:F2`.

- `gap_supervisor_state_cor_01:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
- `gap_supervisor_state_sec_01:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
