# Sandbox create cancel lacks opening guard

Reviewed commit: 51957c55259d7499b0835115f6f5536df54761ef

## TLDR

Cancel lacks the creation question's opening check. A normal input sequence dismissing a later question remains
unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location: `crates/farhelm-ui/src/tensorlake/add_host.rs:255 at 51957c55259d7499b0835115f6f5536df54761ef`.

Source assessment (read with the later corrections below) (review record g014/g014-l5, event 14):

Correction based on supplied owner record g042:g042-l1@9 (findings.jsonl:9); this is a duplicate of that known issue,
not new discovery. Reviewed commit 51957c55259d7499b0835115f6f5536df54761ef, unchanged through pinned
8a8c9579e04299410652f0e4681975a7198dfc7b. TLDR: creation-question Cancel lacks the required opening check. Whether
normal input can consequently dismiss a later creation offer is unverified after investigation. I retract my earlier
claim that Back/Continue alone establishes the queued-input route. Confirmed local source remains
add_host.rs:213-219,237-265 and provisioning.rs:1486,1511: Cancel/header dismissal forwards no shown token and clears
the current offer; recipe_editor.rs:298-305 is only the embedded-editor close guard. SPEC_impl.md:1036-1052 requires
answers to bind to their displayed opening. Owner investigation in exact registry Dioxus 0.7.10 found web dispatch
synchronous (dioxus-web/src/dom.rs:88-110), desktop XMLHttpRequest synchronous (interpreter native.ts:566-590), and
runtime resolution through current element mappings/current mounted listeners (core runtime.rs:369-379,418-460;
diff/node.rs:64-75,446-459). use_callback replaces current storage on rerender (hooks use_callback.rs:1-28; core
events.rs:519-550). Thus an event does not ordinarily retain A's Rust callback after B renders. Reused ElementIds are a
general possibility, not proof of this application's multi-action input sequence. Missing premise: an actual shipped
producer must retain Cancel from A across Back, form render, Continue, asynchronous probe and B's display, and deliver
its ID to an operative Cancel listener. No such producer or event trace is established. Available source settles
current-listener semantics, not that platform/input ordering and ID mapping. What would settle the consequence: a
version-matched authorized interaction trace or deterministic renderer event test showing origin/ID, ordering and
invoked listener, or a concrete shipped deferred relay. No new dependency reads or experiments repeat g042's completed
investigation. Suggested bucket other; confidence confirmed omission, stale-dismissal consequence unverified after
investigation. No billed POST, sandbox deletion, credential loss or persisted user-work loss claimed.
proposed_drop:false; parent decides disposition. possible_cover: exact duplicate g042:g042-l1, with g071/g072 also noted
by parent. Scope: necessary-source interface observation only; neither the assigned report's no-create-on-Cancel claim
nor its historical runtime passes are disproved. Unbound enclosing close predates PR4; Back is new there. Preserve prior
events as superseded confidence claims.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g014/g014-l5`; supporting
records `g042/g042-l1`, `g071/f2`, `g072/l1`. The retained private log preserves earlier wording and its corrections.
This record does not claim runtime reproduction.
