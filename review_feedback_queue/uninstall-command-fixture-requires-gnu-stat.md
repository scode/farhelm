# The command fixture now requires GNU stat on macOS

Reviewed commit: 2ef29e756c983e9a0600865845ef25b4bea13dd1

## TLDR

The modified uninstall command test can fail on macOS before reaching its cleanup assertions.

## Details

Suggested bucket: `other`. Confidence: likely; native macOS execution was not run.. Source: dedicated `gpt-6.1-sol` high
adversarial review. Proposed drop: none.

provisioning.rs:5740–5741 leaves the test platform-unrestricted. Its launcher executes remote shell scripts locally with
fixture-bin:/usr/bin:/bin (252–280), without a stat shim. The newly invoked backend.rs:2548 uses GNU stat's '-c' syntax;
native macOS stat does not provide that interface. The first stop assertion unwraps the resulting failure at
provisioning.rs:5920–5930. Gate this Linux-host shell fixture appropriately or supply the required fixture tool.

Possible cover: none found. The separate XDG_STATE_HOME fixture defect was fixed, but does not address stat portability.

Status at frozen endpoint: Persists: pinned-main provisioning.rs:5791–5792 remains ungated, and backend.rs:2548 retains
the GNU stat invocation.

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
