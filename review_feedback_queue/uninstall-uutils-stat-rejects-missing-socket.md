# Absent sockets fail cleanup with uutils stat

Reviewed commit: 2ef29e756c983e9a0600865845ef25b4bea13dd1

## TLDR

Remote uninstall stops Farhelm but repeatedly refuses to finish on hosts using uutils coreutils.

## Details

Suggested bucket: `high`. Confidence: confirmed; source matches retained repository reproduction evidence.. Source:
dedicated `gpt-6.1-sol` high adversarial review. Proposed drop: none; already tracked outside the mandatory covered
categories..

backend.rs:2550 accepts only diagnostics ending exactly ': No such file or directory'. uutils coreutils 0.8.0 appends
'(os error 2)', so the absent socket reaches the failure branch at 2552 after the supervisor stop at 2547. This blocks
lib removal and host deletion; retries repeat the same failure. Current FLAKES.md:1974–1997 records an actual Ubuntu
26.04 failure with this diagnostic and an unchanged GNU-coreutils retry passing. Accept the precise uutils missing-path
form while retaining permission/I/O refusals.

Possible cover: TODO.md:251–255, 'Remote uninstall and uutils stat', records this exact bug in Tricky bugs, not Planned.
It is neither an accepted behavior nor a BUGS.md entry.

Status at frozen endpoint: Persists at pinned main; backend.rs:2550 is unchanged. No fixing commit found.

Review coverage: full commit diff read: true. No commit diff content remains unread. Initial aggregated output was
truncated; the full diff and queue index were subsequently reopened in bounded chunks. The 13,089-line triage ledger was
searched throughout, with relevant sections read, rather than read exhaustively; TODO outside the specified Planned
bucket and cited relevant entries was likewise searched. Unchanged implementation outside the listed ranges, full
browser fixture setup, and remote-container server-disappearance behavior were not exhaustively traced. Covered highest
findings await the parent’s independent drop review; no nested agents were used.

Runtime evidence: None run. No checkout or scratch files created, modified, or deleted. uutils confirmation relies on
retained repository evidence; stale-socket interruption, native-macOS fixture failure, ambient-service killing, and
deliberate socket redirection were not reproduced. Applied the supplied test-authoring checklist to changed tests and
fixtures.

## Additional affected scope

Reviewed commit: 2ef29e756c983e9a0600865845ef25b4bea13dd1

## TLDR

On an otherwise supported Linux host using BusyBox utilities, uninstall can stop the supervisor and then fail
permanently because its tmux socket is absent.

## Details

Suggested bucket: `other`. Confidence: likely; diagnostic mismatch is confirmed, but a provisioned systemd host
resolving stat to BusyBox was not reproduced. Source: dedicated `gpt-6-astra` high adversarial review. Proposed drop:
none.

crates/farhelm-helm/src/provisioning/backend.rs:2548–2552 recognizes only diagnostics beginning 'stat: cannot stat'.
BusyBox emits "stat: can't stat '<path>': No such file or directory", so genuine absence fails the step before tmux is
attempted. Subsequent retries encounter the same absence and fail again.
[BusyBox stat source](https://github.com/mirror/busybox/blob/master/coreutils/stat.c) confirms the alternate spelling.
SPEC.md:119–124 supports Linux by systemd and architecture capabilities rather than distribution; backend.rs:2938–3050
implements that policy without requiring GNU stat. Recognize the supported absence diagnostic without weakening
permission/I/O-error refusals, and exercise both utility implementations.

Possible cover: No exact cover found. Supported host setup does not explicitly exclude a panel-provisioned Linux host
merely because its stat implementation is BusyBox.

Status at frozen endpoint: Persists at pinned main: backend.rs:2548–2552 is unchanged. No fixing commit found.

Review coverage: full commit diff read: true. No commit-diff content remains unread. Unrelated portions of the
13089-line triage ledger, other queue finding bodies, and unchanged surrounding modules were not exhaustively read. An
initial aggregate policy read was truncated; policy files and relevant ledger sections were subsequently reopened, but
the ledger was not read end-to-end. A later aggregate SPEC_impl output truncation was reopened at 3377–3478. No Sol
reports or new working-copy queue additions were read. Independent highest-candidate drop adjudication remains for the
parent; no nested agents were launched.

Runtime evidence: None. Findings use historical code, pinned-main comparison and upstream source inspection. No checkout
files were created, modified or deleted; no fetch or rebase was performed.
