# Recovery specifications disagree on the missing-output notice's lifetime

Reviewed commit: 6bd019f5359ed2ec4c69b13ab75b4840626f842f

## TLDR

Recovering a terminal during the same supervisor run can clear its missing-output notice. The product specification
promises the notice until the supervisor restarts; the implementation specification promises only the terminal runtime's
lifetime. Both are authoritative, and the new recovery path makes those contracts differ.

## Details

Suggested bucket: **other**, for contradictory diagnostic contracts. Confidence: **confirmed source behavior and textual
conflict**. No new output loss, security consequence or destructive action is claimed. Proposed drop: none.

At the reviewed commit, SPEC.md, "Terminal experience", 2147–2149 says "Some earlier output could not be recovered"
stays above scrollback until the supervisor next restarts. SPEC_impl.md, "Recovery and checkpoints", 1978–1980 instead
says the history-trimmed mark lasts as long as that runtime runs because a snapshot cannot carry it. The latter does not
fully settle the conflicting product promise.

`crates/farhelm-supervisor/src/hosts/terminal.rs:1322–1329` initializes `history_gap=false`; `3694–3709` sets the
runtime-local flag. Encoding at `3401–3437` serializes the emulator without it, and rebuilding at `3228–3231` installs
the restored emulator into a fresh runtime. `crates/terminal-host/src/journal.rs:264–309` accepts a later checkpoint and
clears its trimmed flag; `365–395` then sends no gap when the snapshot and tail agree. A gap, followed by an accepted
checkpoint and a contact failure/re-adoption, can therefore remove the notice before this supervisor ends.

Choose and reconcile the intended lifetime before changing code. Keeping the shorter lifetime would require narrowing
the product promise; keeping the longer one requires retaining the notice across same-supervisor recovery. This record
does not choose between them. The existing resize/clear-scrollback notice findings have different triggers and remedies.

No exact planned repair, known bug, queue item or ledger decision was found. The conflicting implementation paragraph is
possible partial acceptance, retained here for triage. FILTER.md does not settle an authoritative documentation
conflict. Source was inspected without runtime execution; no later repair was found through
`c5ae8548439cc5bfa33c11ef7ce8da6129d8072a`.

Provenance: trailing-review run `ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`, `g048/g048-d10`, event 18. The earlier proposal
to exclude this as implementation-spec acceptance was superseded under the same ID.
