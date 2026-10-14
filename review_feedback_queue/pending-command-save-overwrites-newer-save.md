# A dismissed pending save can overwrite a newer command

Reviewed commit: f6da791d88336193fba346079fe03de5c84a5ae9

## TLDR

After reopening Settings, a successful replacement or removal can later be undone by an older pending save, restoring a
command the user no longer wants executed.

## Details

Suggested bucket: `highest`. Confidence: likely; the reachable state and unconditional overwrite are established by
inspection, but an actual transport schedule allowing the newer GET/PUT to overtake the old PUT was not reproduced.
Source: dedicated `gpt-6-astra` high adversarial review. Proposed drop: none.

crates/farhelm-ui/src/checkout_post_clone.rs:23–29 and 78–101 keeps saving state within one mounted field while
launching an app-owned PUT. Closing Settings unmounts that field (app_bar.rs:526–529, 700–702); reopening creates an
unlocked field after its GET, even while the earlier PUT remains pending. The new field can submit and successfully save
command B, including null to disable the hook. If delayed PUT A subsequently reaches the helm,
checkout_config.rs:118–135 and 326–368 applies it unconditionally. There is no cross-mount serialization or request
revision check. The retired component then suppresses its completion at line 97, leaving the newer field showing B and
its success message while future checkouts execute A. This is a durable wrong command, not merely stale display. Keep
save ownership/order outside the dialog lifetime, or enforce an equivalent stale-write guard. The added pending-save
test releases the old PUT before reopening (e2e/tests/settings.spec.ts:200–204), so it does not cover this interleaving.

Possible cover: SPEC_impl.md:482–497 requires saves to survive dismissal, but does not authorize an older save to undo a
subsequent successful save. FILTER.md's timing filters exclude durable wrong state and security/data-loss consequences.
No matching original queue entry or ledger decision found.

Status at frozen endpoint: Persists at pinned main 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703. The component is unchanged,
current api.rs:553–562 still sends each PUT directly, and the production setter is unchanged. No fixing commit.

Review coverage: full commit diff read: true. No commit-diff hunks remain unread. The 13,089-line triage ledger was
searched rather than read end-to-end; unrelated queue finding bodies and unrelated specification sections were not read.
Supporting dependency code and checkout retry machinery were traced selectively, not exhaustively. Aggregate tool-output
truncation affected an initial broad TODO read and a sessions excerpt; the complete Planned bucket and relevant sessions
excerpt were reopened before use. New working-copy queue additions and other reviewers' reports were not read.

Runtime evidence: No runtime tests or reproductions run, as requested. Applied the supplied test-authoring checklist to
all added tests. Findings are static-review conclusions with the stated caveats. No checkout or scratch files created,
modified or deleted; no fetch, rebase or range extension.
