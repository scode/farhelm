# Finalized release notes promise an Update confirmation

Reviewed commit: 6a780e2ec502a9711eaa55cb16ab66fd5f1e46a4

Also reviewed: `8628361693baafb26ba66023988a874bea6b5b7b`, `44c664d9137de9ba7e425461d762310f10872c4a`,
`59cdaec049e410e4334ef4530a59c3496aacc224`.

## TLDR

The finalized v0.30.0 release notes say a remote host Update shows a numbered confirmation. Update actually starts when
clicked, so the notes promise an inspection opportunity the operation does not provide.

## Details

At the original reviewed commit, `releasing/changelog.d/host-plan-steps.md:5` includes updating in the claim that the
confirmation shows a numbered list of every step. `SPEC.md:105–107` instead makes the Update click the authorization and
explicitly says no plan is shown for confirmation. `SPEC_impl.md:4218–4219` describes automatic consumption of the
update plan, and `crates/farhelm-ui/src/provisioning.rs:274–282` implements that distinction. The user documentation
already says Update runs without asking (`website/src/content/docs/docs/using/manage-hosts.mdx:60–61`). Restrict the
confirmation claim to setup and uninstall; Update can still be described as showing numbered progress.

The original finding concerned an unpublished fragment, subject to release-time curation. The landing report
independently calls out the same wording under Possible follow-ups
(`plans/reports/host-uninstall-dialogs.report.md:185–186`). That report is context, not a recorded triage decision.

Suggested bucket: **other**. Confidence: **confirmed by static reading** for the wording discrepancy; reader confusion
was not reproduced. No security or loss-of-work consequence is established. The wording was introduced in
`93caaa76a28325d9f8e75b66153fe863b2a3b5f3` and remains at the reviewed tip.

Possible cover: none found in the specifications, Planned TODO entries, BUGS, current queue or triage ledger. The
rare-trigger filters do not cover this ordinary action and wording. Proposed drop: none. Provenance: trailing review of
`c5fcfee8681e46b1d47718771b321488037d3f83..6a780e2ec502a9711eaa55cb16ab66fd5f1e46a4`, backend/specification reviewer;
parent checked the fragment, specification and automatic-update decision.

The trailing review through `bf0935f02fc8fbfc26ab54e1b9fb8e7e6bf5c441` found that curation preserved the claim.
`8628361693baafb26ba66023988a874bea6b5b7b:CHANGELOG.md:32` adds it to the release draft;
`44c664d9137de9ba7e425461d762310f10872c4a:CHANGELOG.md:30` retains it while removing the draft warning. The release
screenshot callout at `59cdaec049e410e4334ef4530a59c3496aacc224:e2e/docs-shots/release-notes.spec.ts:1194` reinforces
it: “Update and uninstall list theirs the same way.” These commits delete the original fragment. The record therefore
concerns finalized release-note sources now, rather than only a draft awaiting curation. Publication, screenshot pixels
and actual reader reliance were not independently verified. A correction to published material needs a task targeting
that material under the frozen-material rule.

Confidence remains confirmed for the wording discrepancy, in bucket `other`. At
`c6b1bd4ccabefd1016c0093dbb3e896d5226d64f`, `SPEC.md:105–107`, `SPEC_impl.md:4228–4235`, and
`crates/farhelm-ui/src/provisioning.rs:274–282` still distinguish automatic Update from confirmation. The guide
correctly explains this at `website/src/content/docs/docs/using/manage-hosts.mdx:57–65`. Provenance: separate same-model
docs/plan reviewer; the recording parent checked the finalized note, callout, specification and automatic-update helper.
No new coverage or filter accepts the discrepancy. Proposed drop: none; this extends the original item.
