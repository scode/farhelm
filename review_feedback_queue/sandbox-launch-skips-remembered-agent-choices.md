# Sandbox launches skip remembered permission and wrapper choices

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

A successful sandbox launch does not persist the user's permission or wrapper choice. The initiating page's local mirror
hides the omission until a reload.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `0ff236126fe94cee84973dc142d17b70fc15bcf3`. Original source location:
`crates/farhelm-helm/src/sessions.rs:1994-2018`. Locations below use that revision unless another is named; the
assessment was checked through the reviewed tip.

Create acceptance skips record_create_history_with_destination entirely for session-owned hosts (helm
sessions.rs:1994-2018). That helper also calls record_launch_history, whose side effects persist helm-wide remembered
permissions and per-agent wrapper prefixes (store.rs:200-290), so successful explicit user choices lose those memories
too.

The initiating page updates its local mirror (list/view.rs:5227-5267,5642-5667), masking the omission until reload; that
mirror sends no independent persistence request. Sandbox recent setups are explicitly deferred, but the
permission/wrapper memory contract is separate and remains supported (SPEC.md remembered-choice rules and its Sandbox
sessions section). Split the memory side effects from the deferred host history, preserving the admitted user selection
as authority.

Suggested bucket other, confirmed source route. No later acceptance of a visibly old default is treated as security
loss. This differs from the older ledger fixes preventing host replies or agent-created sessions from teaching defaults:
the missing memory here comes from an explicit user's successful sandbox launch.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g051/g051-D2`, exclusive base
`1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private findings
history, dispositions and independent consultations preserve original confidence, suggested buckets and every revision.
No fix or triage decision was executed.
