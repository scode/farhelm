# Account save visible draft mismatch unresolved

Reviewed commit: da24fb6822d289a144e18677cdd248cc35093246

## TLDR

Account Save reads the live API-key field. A native input sequence saving a key different from the visible field remains
unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location: `crates/farhelm-ui/src/tensorlake_settings.rs:196 at da24fb6822d289a144e18677cdd248cc35093246`.

Source assessment (read with the later corrections below) (review record g060/f2, event 13):

Later-turn synthesis completed; this revision supersedes the earlier categorical wording of a behavioral violation while
preserving the structural flag requested by the brief. Reviewed commit: da24fb6822d289a144e18677cdd248cc35093246;
unchanged source structure through pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b. TLDR: Save reads the current
credential draft when handled. No supported sequence has been established in which this sends a different credential
from the field the user acted on. Structural fact: original onclick :196-198 (pinned tip :226-228) rereads key() and
constructs key_save(key()), with no immutable render-owned credential. SPEC_impl.md GUI: Dioxus, Answers bind to what
was drawn prohibits user-confirmed actions selecting their target from live state, so retain this fact for interface
review; a stale-answer mismatch cannot be inferred from the read alone. Ordinary typing then Save is not a
counterexample: typing changes the native visible field and oninput copies that value to the local draft. Searches of
the complete pinned component identify only oninput and successful key Save/Clear as draft writers (:196 and :153);
settings snapshots, switch replies and SSH-key replies never install another nonempty draft. The success path resets to
empty, and Save's live empty check refuses that value. Key strings are passed to the request synchronously before spawn;
no await splits the read/argument construction. Closing Settings retires this local component rather than reusing its
draft. Event-transport investigation: original and pinned Cargo.lock both pin dioxus-core/desktop/web/interpreter-js
0.7.10 with identical registry checksums. Inspected the corresponding existing registry source, not an invented harness.
dioxus-interpreter-js src/ts/native.ts:42-44 initializes liveview=false; :397-408 uses handleVirtualdomEventSync;
:566-590 performs synchronous xhr.open('POST', endpoint, false), xhr.send(), then returns the handler result.
dioxus-desktop src/webview.rs:78-84,190-194 dispatches Runtime::handle_event before replying. Therefore later ordinary
typing cannot overtake an already-dispatched desktop Save through the proposed asynchronous event queue. Input
serialization reads HTMLInputElement.value (serialize.ts:181-191). dioxus-web src/dom.rs:88-118 directly calls
runtime.handle_event inside the browser event callback, without an async hop. Desktop webview.rs:562-588 also waits for
edits to flush before polling/rendering more VDOM work; this does not itself prove every visual/event boundary but
supplies no alternate credential-substitution route. These checks refute the concrete ordinary-typing/overtaking
examples rather than prove all platform schedules safe. Remaining premise: a supported rendered-control/field lifecycle,
beyond normal typing, that actually substitutes a different nonempty draft before the earlier Save is handled.
Accessible sources checked here do not establish it; permitted source-only review cannot use runtime webview/browser
event-order experiments. Settling a stronger claim needs a faithful integration reproduction showing displayed field,
event order and sent request, not merely manually invoking an old closure after changing a signal. Harmful overwrite
remains unverified after investigation. No disclosure, credential-loss or highest consequence follows merely because the
field contains a secret. Suggested bucket: other for the structural concern; confidence confirmed live read, unverified
behavioral mismatch. proposed_drop: false. possible_cover: unknown; parent will route account UI interface owners. No
lead remains awaiting an available focused source check. Keep f1 separate: its independently returned aggregate
snapshots can demonstrably disagree with later acknowledged display, while this item's stale-credential consequence
lacks its event premise.

Later qualification (review record g060/f2, event 16):

This event supplements, rather than replaces, the complete latest f2 synthesis event and its checks/missing premise.
Parent supplied g056:g056-l3, g056/findings.jsonl:11, reviewed
68790603037bb51ccd8655e4a948c7ffb747760a:crates/farhelm-ui/src/tensorlake/recipe_editor.rs:687 and persistent at pinned
tip. Its recipe leave-Save reads live draft after a rendered ConfirmSlot token, but no supported stale-authority
consequence was established. It independently confirms ordinary visible editing then Save is not a mismatch; web/core
and desktop dispatch are synchronous and no yield splits validation/capture/request construction. It identifies a
possible native input/DOM/paint gap without proving a shipped stale-visible sequence. Preserve its owner, qualifications
and source scope: this is recipe-editor confirmation, whereas f2 is the account's plain API-key Save; analogous
structural pattern does not establish one shared proven harmful defect or justify copying recipe isolation assumptions
onto the account form. No new event evidence settles f2, so do not repeat the unchanged source investigation or upgrade
it. f2 remains confirmed live key() read with stale credential mismatch unverified after investigation, suggested other,
no disclosure/highest inference merely from secret-valued input. proposed_drop false; possible_cover analogous
g056:g056-l3 for framework boundary evidence, not exact same source-location duplicate. g048:l1's separate provider-auth
disclosure is confirmed on its own supported producer and is not weakened by this uncertainty. No NEW discovery in this
supplement.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g060/f2`. The retained private
log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
