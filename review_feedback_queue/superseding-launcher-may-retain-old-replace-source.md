# Superseding launcher may retain an earlier Replace source

Reviewed commit: f1d71445d629f8ee244a592b2173507e32b8c343

## TLDR

A stale Guarded answer could replace the earlier source if another source opening superseded the mounted launcher before
that answer ran. A normal shipped route through the modal barriers has not been established.

## Details

Suggested bucket: provisionally **highest**, for conditional deletion of user-owned work in the earlier source.
Confidence: **conditional captured-source mechanism established; harmful event reachability unresolved**. No
wrong-source replacement or native event sequence was reproduced.

Parent source-opening handlers can change the prefill without unmounting the composer
(`crates/farhelm-ui/src/list/view.rs:2385–2407,2450–2478`). The submit callback captures its earlier source
(`list/create_form.rs:5223–5226,5295–5298`) and uses that source through replacement dispatch
(`5466–5510,5796–5821,5957–5971`). Under the proposed ordering, a stale Guarded answer could therefore submit the old
source. The previous native-submit path already captured the source; pre-existence alone does not establish acceptable
behavior or justify excluding a credible loss-of-work concern.

The missing source premise is an actual shipped route that supersedes an already-open guarded launcher and delivers its
old answer before rerender. The full-screen backdrop and focus trap obstruct the naive second-row interaction
(`assets/app.css:5728–5737`; `create_form.rs:1847–1890`). Sidebar flyouts remain below that backdrop; the native menu
has no Clone/Replace-with source action. The Clone tests explicitly close the modal before choosing another source
(`e2e/tests/clone.spec.ts:820–831,878–898`). These checks narrow the hypothesis without proving every possible native or
accessibility ordering impossible.

Independent auditor `g151` returned **ambiguous** and stopped this investigation incomplete. Possible cover: none
established. Proposed drop: none for this conditional lead. It is separate from the established delayed deletion-guard
defect. Whole-launcher Cancel followed by a queued first submit is separately refuted by ancestor-first rendering and
scope-task teardown; that conclusion does not settle a composer that stays mounted.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, `g046/f5`, independently checked by `g151`.
