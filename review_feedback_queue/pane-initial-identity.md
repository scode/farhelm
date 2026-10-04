# Delete can bind a recycled pane PID to an unrelated process

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Delete can bind a recycled pane PID to an unrelated process.

## Details

`F31 / COR-PANE-INITIAL-IDENTITY` — **possible** — `crates/farhelm-supervisor/src/service/teardown.rs:182` — Delete can
bind a recycled pane PID to an unrelated process

Delete could adopt an unrelated process as one of the session's terminal roots if PID reuse occurs between tmux's answer
and the supervisor's first kernel observation. Delete now enumerates every live pane belonging to the session, so it can
terminate tab processes even when their environment no longer carries Farhelm's session marker. For each pane, tmux
supplies a bare PID. The supervisor then reads the kernel's process start time and saves the pair as the identity to
reap.

The later cleanup checks correctly reject a PID whose start time changes after that capture. They cannot repair a
mistaken initial capture: if the pane process exits, is reaped, and its PID is reused before the first kernel read, the
replacement process's start time becomes the accepted value. The process sweep can then follow and terminate that
unrelated process and its descendants even though they have no Farhelm marker. Reading the PID immediately after the
tmux reply makes the window small but does not tie the two observations to the same process.

Reserve or corroborate the pane's identity before accepting the kernel sample as a cleanup root. Add a focused
regression that replaces the process during this initial observation, rather than only changing its identity after
capture.

Suggested bucket: highest

Possible cover: TRIAGE_OUTCOMES.md:940–959 fixes the later asynchronous gap by carrying a captured PID-and-start-time
pair; it does not explicitly resolve reuse between tmux's bare-PID observation and that initial capture.

Caveats: This requires an extremely narrow, unreproduced PID-reuse window. The fresh independent D23 audit found the
acceptance record ambiguous and retained the finding.

Restater note: The tmux query and initial kernel read are separate observations, and the subsequent sweep trusts the
captured replacement identity if it remains stable. Whether a real tmux pane process can be reaped and its PID reused
inside this exact window was not established.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `runtime_security p1`.

Possible cover recorded during collection: Ledger940–959 fixes later async gap, not this initial capture..

Collection caveats: Extremely narrow unreproduced PID-reuse premise; D23 independent ambiguous-record.

## Filed reviewer metadata

- `runtime_security p1`: confidence as filed: **possible**. The separate tmux PID read and kernel identity read are
  established; actual PID reuse during that short interval is unverified. Suggested bucket as filed: `highest`.
