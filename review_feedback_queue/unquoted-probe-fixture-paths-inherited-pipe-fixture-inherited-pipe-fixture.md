# Unquoted probe-fixture paths — inherited-pipe fixture

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The inherited-pipe fixture could truncate a file outside its temporary directory.

## Details

F87 — **possible** — `crates/farhelm-supervisor/src/tmux.rs:4767` — Unquoted probe-fixture paths — inherited-pipe
fixture

This separate fixture embeds its PID-file pathname into shell code without quoting. With a space-containing temporary
root, the shell redirects to the pre-space prefix rather than the intended fixture file. A writable regular file at that
prefix could be overwritten without attacker-controlled input. No runtime reproduction or shipped-product exploit is
claimed. Correct the shell quoting or pass the path separately at this independently editable interpolation site.

## Evidence and triage context

- tmux.rs:4759–4760 derives the PID file from the ambient temporary root. At :4767–4768 pidfile.display is inserted
  directly after > in shell source, with no quoting or validation. probe_fixture at :4626–4628 writes that source under
  #!/bin/sh, and :4773 executes it through probe_tmux. For /tmp/review scratch as the root, > /tmp/review
  scratch/<generated>/grandchild.pid targets /tmp/review. The existing fixture assertions run after that write and
  cannot undo truncation. No SPEC, Planned item, BUGS entry, queue item or ledger acceptance matching this trigger and
  consequence was found. FILTER.md:38–42 expressly excludes wrong-target writes and work loss. Static trace only; not
  executed.
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
