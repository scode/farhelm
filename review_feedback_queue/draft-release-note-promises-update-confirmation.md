# Draft release note promises an Update confirmation

Reviewed commit: 6a780e2ec502a9711eaa55cb16ab66fd5f1e46a4

## TLDR

The draft release note says a remote host Update shows a numbered confirmation. Update actually starts when clicked. If
published unchanged, the note would tell users to expect another chance to inspect the action before it starts.

## Details

`releasing/changelog.d/host-plan-steps.md:5` includes updating in the claim that the confirmation shows a numbered list
of every step. `SPEC.md:105–107` instead makes the Update click the authorization and explicitly says no plan is shown
for confirmation. `SPEC_impl.md:4218–4219` describes automatic consumption of the update plan, and
`crates/farhelm-ui/src/provisioning.rs:274–282` implements that distinction. The user documentation already says Update
runs without asking (`website/src/content/docs/docs/using/manage-hosts.mdx:60–61`). Restrict the confirmation claim to
setup and uninstall; Update can still be described as showing numbered progress.

This is an unpublished fragment, subject to release-time curation, not evidence that a published release note misled
anyone. The landing report independently calls out the same wording under Possible follow-ups
(`plans/reports/host-uninstall-dialogs.report.md:185–186`). That report is context, not a recorded triage decision.

Suggested bucket: **other**. Confidence: **confirmed by static reading** for the wording discrepancy; reader confusion
was not reproduced. No security or loss-of-work consequence is established. The wording was introduced in
`93caaa76a28325d9f8e75b66153fe863b2a3b5f3` and remains at the reviewed tip.

Possible cover: none found in the specifications, Planned TODO entries, BUGS, current queue or triage ledger. The
rare-trigger filters do not cover this ordinary action and wording. Proposed drop: none. Provenance: trailing review of
`c5fcfee8681e46b1d47718771b321488037d3f83..6a780e2ec502a9711eaa55cb16ab66fd5f1e46a4`, backend/specification reviewer;
parent checked the fragment, specification and automatic-update decision.
