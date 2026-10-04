# Planned shutdown exits before every output client becomes safe

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Planned shutdown exits before every output client becomes safe.

## Details

`F14 / COR-SHUTDOWN-EXPIRY` — **possible** — `crates/farhelm-supervisor/src/service/teardown.rs:1111` — Planned shutdown
exits before every output client becomes safe

A planned supervisor stop can still exit while terminal-output clients have not reached their safe shutdown boundary.
Those clients connect the supervisor to the private tmux server that keeps all sessions running on the host. Safe
shutdown asks tmux to stop sending pane output and waits for acknowledgment before closing the connection; closing while
output is queued is the trigger associated with historical tmux server crashes.

The entire shutdown shares one ten-second timeout, including waiting for the lock used by terminal attachments. That
wait can consume the budget before shutdown even marks the supervisor as stopping or asks its clients to stop streaming.
Expiry cancels the shutdown wait. The supervisor's entry point then logs that some clients did not finish and returns
anyway (`service/core.rs:14354`). Cleanup tasks may still be running, but process exit cannot rely on their completion.
If the pinned tmux has the historical abrupt-close defect, a routine stop, restart, or upgrade could end every session
and its running work.

Reproduce budget expiry narrowly against the pinned tmux 3.7c, with streaming clients and a controlled delay at the
attachment lock or cleanup boundary. Define an expiry fallback that still establishes the acknowledged no-output
boundary for every output client before exit.

Suggested bucket: highest

Possible cover: none

Caveats: A tmux 3.7c abort on this path is unverified. BUGS.md documents observed crashes on distro tmux 3.6 and leaves
applicability to newer pinned versions unsettled. TRIAGE_OUTCOMES.md:1490–1508 explicitly accepts a bounded shutdown
budget, but does not explicitly accept losing all sessions when it expires. BUGS.md:34–44 excludes planned stops from
its accepted abrupt-death residual. The fresh D14 audit found the acceptance record ambiguous and retained the finding.

Restater note: Timeout followed by return is confirmed in the source. The all-session-loss consequence remains
conditional on the pinned tmux defect, and the explicit budget decision makes the intended expiry behavior a triage
question.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `runtime_general p1`, `runtime_data p1`, `runtime_lifecycle p1`, `runtime_systems p1`,
`runtime_security p1`.

Possible cover recorded during collection: none identified.

Collection caveats: Pinned tmux3.7c abort unverified, historical3.6 crash. Ledger1490–1508 accepts bounded budget but
not explicitly all-session loss; BUGS34–44 excludes planned stop. Fresh D14 ambiguous-record.

## Filed reviewer metadata

- `runtime_lifecycle p1`: confidence as filed: possible; timeout and exit confirmed, but whether pinned tmux 3.7c
  retains the server-abort behavior observed on distro tmux 3.6 is the open material premise. Suggested bucket as filed:
  highest. Confidence: possible; timeout and exit confirmed, but whether pinned tmux 3.7c retains the server-abort
  behavior observed on distro tmux 3.6 is the open material premise.
- `runtime_systems p1`: confidence as filed: possible, with confirmed timeout behavior but an unverified consequence on
  pinned tmux 3.7c. Suggested bucket as filed: highest.
- `runtime_security p1`: confidence as filed: **possible**. The timeout and subsequent exit are established; a
  tmux-server abort on the pinned release, and an ordinary trigger that leaves dangerous output outstanding, remain
  unverified. Suggested bucket as filed: `highest`.
- `runtime_secrets p1`: confidence as filed: possible / likely only if the shutdown budget expires with an
  output-bearing client and the affected tmux abort path is reached; that tmux consequence was not reproduced in this
  review. Suggested bucket as filed: highest, proposed drop. Confidence: possible / likely only if the shutdown budget
  expires with an output-bearing client and the affected tmux abort path is reached; that tmux consequence was not
  reproduced in this review.
- `runtime_general p1`: confidence as filed: original confidence wording unavailable Suggested bucket as filed: original
  suggested-bucket wording unavailable
- `runtime_data p1`: confidence as filed: original confidence wording unavailable Suggested bucket as filed: original
  suggested-bucket wording unavailable
