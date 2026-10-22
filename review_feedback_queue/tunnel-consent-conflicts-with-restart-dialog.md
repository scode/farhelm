# Tunnel consent conflicts with Restart with isolation

Reviewed commit: 5e2c61c9a8ee967e2802ecfe578ba6e88fce5f73

## TLDR

In the Mac app, choosing **Tunnel and open** on an approval card while **Restart with** is open can leave both dialogs
ignoring ordinary input. Escape may recover. The source conflict is established; its native manifestation and recovery
have not been tested.

## Details

Suggested bucket: **other**, for recoverable dialog interaction. Confidence: **likely**, with native event delivery,
exact competing-Escape recovery and frequency unmeasured. No unintended approval, permanent lock or user-work loss is
established. This can predate the reviewed modal extraction; it is not an asserted regression from equivalent
extraction.

Leave tunnel consent off, open Restart with, then use an approval card's Tunnel and open button. Cards are exempt from
modal isolation and remain actionable above that dialog (`crates/farhelm-ui/src/approvals.rs:156–170,365–372,448–451`;
`assets/app.css:3773–3781,7595–7602`). The button reaches `prepare_card`, which publishes consent without handing off
the existing modal (`approvals.rs:221–237`; `tunnels.rs:129–175`). Consent releases only `.app-settings-dialog` before
installation (`tunnels.rs:359–362`), which does not match Restart with.

The existing modal's observer claims the new warning backdrop as inert. Warning isolation then claims the shell holding
Restart with (`modal_isolation.rs:164–180,198–207`; sibling placement at `lib.rs:1801–1803,1826–1827`). Each modal
therefore has an inert ancestor owned by the other. The source-specific Settings handoff does not cover this entrypoint.
Generalizing to every other dialog would be too broad: some share the released Settings selector.

Possible cover: at pinned tip `2ea2826caa0eec867c4e2bddc01badbac7bbe6c7`, `SPEC.md:3323–3326` accepts delivered actions
while the UI is open, not two mutually inert dialogs. The queued host-dialog admission/refusal findings have different
triggers. No subsecond action window is required, and the out-of-view explanation filter does not cover inert input. No
exact Planned, BUGS, queue, ledger or FILTER match was found. Proposed drop: none. Verify the owned modal handoff and
Escape behavior in an isolated desktop fixture before choosing a fix.

Provenance: trailing-review run `30f46c42-a021-475f-96a8-eac06930471b`, `g054/discovery-f1`, found during the
independent modal-exclusion auditor's discovery pass. The original extraction-leakage allegations remain separately
source-refuted.
