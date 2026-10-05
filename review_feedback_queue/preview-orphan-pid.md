# Docs preview takeover can kill an unrelated successor process

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Docs preview takeover can kill an unrelated successor process.

## Details

`F41 / COR-PREVIEW-ORPHAN-PID` — **possible** — `website/scripts/preview.sh:149` — Docs preview takeover can kill an
unrelated successor process

The docs preview can reclaim its fixed port from an Astro server whose checkout has been deleted. It first checks that
the listener's working directory is deleted and its command line identifies an Astro development server. It then sends
TERM, polls the numeric process ID for up to five seconds, and sends KILL if that number still exists. This server is
not a child owned by the preview script.

The initial identification does not protect later signals. If that server exits and another process receives its ID, the
polling loop can mistake the replacement for the original server and eventually kill unrelated work under the same
account. No such reuse was measured or reproduced. On Linux, obtain a process handle such as a pidfd that remains tied
to the original process, signal through that handle, and finish when that identity exits. Proposed bucket: highest. No
possible cover was identified.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `auto_lifecycle p1`, `auto_systems p1`, `auto_security p1`, `auto_trust p1`,
`auto_secrets p1`, `auto_edges p1`.

Possible cover recorded during collection: none identified.

Collection caveats: PIDreuse unmeasured; initial identity sound, later recipient unprotected.

## Filed reviewer metadata

- `auto_lifecycle p1`: confidence as filed: possible; confirmed bare-PID reuse across a polling loop, unverified premise
  is PID recycling while this takeover is running. No observed reproduction. Suggested bucket as filed: highest.
- `auto_systems p1`: confidence as filed: **possible / likely**, with the named material premise that the original
  preview exits and its numeric PID is reassigned before the polling loop observes absence. Suggested bucket as filed:
  **highest** (terminating unrelated user processes/work).
- `auto_edges p1`: confidence as filed: possible; actual PID recycling during takeover is unverified. Severity: loss of
  unrelated processes/work. Suggested bucket: highest. Suggested bucket as filed: highest.
- `auto_security p1`: confidence as filed: possible / likely with the unverified premise that the original Astro process
  exits and Linux reuses its PID before the following liveness check or KILL. No reproduction. Suggested bucket as
  filed: highest. Confidence: possible / likely with the unverified premise that the original Astro process exits and
  Linux reuses its PID before the following liveness check or KILL. No reproduction.
- `auto_trust p1`: confidence as filed: possible; source establishes the identity gap and wrong-target signal, but the
  material premise is actual PID reuse before the next 0.1-second poll or final KILL. That timing was not reproduced.
  Suggested bucket as filed: original suggested-bucket wording unavailable
- `auto_secrets p1`: confidence as filed: possible; unverified premise that the orphan exits and its PID is reused
  during the polling/escalation window. Suggested bucket as filed: highest.
