# The sandbox-launch question lacks its required opening token

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

The sandbox-launch question uses unguarded answers instead of the required opening token. Same-burst Cancel and Submit
is product-accepted; a stronger obsolete-question consequence remains unverified after investigation.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `952183b99721b44fd6fef27b5ca3b38499165b7f`. Original source location:
`crates/farhelm-ui/src/list/create_form.rs:3116-3117,8464-8471`. Locations below use that revision unless another is
named; the assessment was checked through the reviewed tip.

The recipe-launch creation question uses Option<LaunchDraft> state (create_form.rs:3116-3117). Cancel clears the live
question; Confirm restores consent and submits without a rendered opening identity (:8464-8471). Launcher generation
(:5359-5360) and draft equality (:5532) protect different lifetimes and are not question tokens. SPEC_impl.md:1042-1053
requires every question to use opening binding.

Same-dialog Cancel and Submit before redraw can still create and bill the sandbox, but SPEC.md:4856-4859 explicitly
accepts that product consequence. Independent consultation agreed with retaining other for the confirmed mechanism
omission, without treating that charge as an independently prohibited effect. A stronger cross-reopening consequence
remains unverified after investigation: framework current-listener/element dispatch did not establish a shipped
old-answer producer across completed reopening. No runtime event or billing experiment ran.

Use a rendered-token confirmation slot. The existing sandbox-create-cancel-lacks-opening-guard item is the separate
Add-host question in tensorlake/add_host.rs; this new session-launch owner needs its own repair. Do not import the
session lifecycle's process-loss consequence. Original and revised confidence/severity remain in the private history.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g055/g055-001`, exclusive base
`1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private findings
history, dispositions and independent consultations preserve original confidence, suggested buckets and every revision.
No fix or triage decision was executed.
