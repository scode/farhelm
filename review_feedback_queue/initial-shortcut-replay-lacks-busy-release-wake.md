# Initial shortcut replay may miss a busy-release wake

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

An initial replay that observes busy state has no explicit wake when that state clears; an ordinary overlapping input
producer remains missing.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

Matching Dioxus hooks and subscription behavior were inspected. Backdrop/opener guards obstruct the naive overlap, and
no complete ordinary input/IPC busy producer was established for the remaining case. The implementation is pre-existing.
Preserve the checked results and exact producer gap rather than claiming a reproduced stuck sequence.

Source locations:
`crates/farhelm-ui/src/list/create_form.rs:4499-4546; crates/farhelm-ui/src/list/create_form/draft.rs:1415-1422`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Discovery investigation completed at d6645f771bee8ed03a66486ef8755834937d759d. This is a contextual pre-existing lead,
not a demonstrated typed-quick-launch regression: immutable exclusive base da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c
already has the same initial replay effect (create_form.rs:4413-4465) and untracked busy admission
(create_form/draft.rs:1370-1386). TLDR if the missing premise holds: a quick launch can stay hidden/loading after a
competing page operation ends, until cancelled and retried. Positive source facts: current replay pops a captured
template only after accept_search accepts (4534-4546); its live draft gate returns None on ops.busy_now
(draft.rs:1420-1422); busy_now is explicitly untracked (ops.rs:116-131). The effect depends explicitly on hosts_loaded
plus its tracked readiness/queue reads, not busy. Inspected local locked-version dioxus-hooks 0.7.10
use_effect.rs:11-49, use_reactive.rs:103-115, use_callback.rs:3-24; Cargo.lock:1447-1461 selects that version, but
registry file/archive provenance was not independently authenticated. Source resource setup has no ordinary polling wake
for launch catalog (create_form.rs:2678-2694). Negative producer checks: open_new refuses an already held lock
(list/view.rs:3614-3618); after mounting, the transparent backdrop blocks page clicks (app.css:6090-6093), and the
hidden form is a modal (create_form.rs:6068-6128). An ordinary concurrent operation therefore cannot simply be assumed.
Header actions do have a shared-lock producer (session_view.rs:2382-2396; ops.rs:177-181), but the only remaining
suspected admission route is an old-page event already delivered or queued before the hidden backdrop reaches the native
view. No complete shipped event/IPC ordering proving that overlap was established. No operation on a wrong target,
automatic destructive action, security consequence or existing-user-work loss is claimed. Actual checks also found live
pending-replay submission refusal (create_form.rs:4933-4939) and cancellable hidden-modal Escape (6149-6153), so this is
availability rather than unsafe submission. Required coverage search across SPEC/SPEC_impl, empty TODO Planned, BUGS,
queue INDEX and matching queue text, TRIAGE_OUTCOMES and queue plans found no exact initial-replay/busy-release item.
Existing host-snapshot batch item is a different trigger and consequence. Exact missing premise: a compatible shipped
input/IPC schedule admitting a lock claim after the opening guard and before replay, with the relevant readiness/queue
dependencies unchanged after release. Source inspection narrows but does not prove physical native event timing; an
owned focused trace/regression witnessing that admission and failed post-release wakeup would resolve it. Suggested
bucket other; confidence unverified after investigation. proposed_drop none. possible_cover FILTER Rare, self-correcting
glitches and Races a person would have to win inside a sub-second window; applicability is not established without the
producer and recovery trace. SPEC.md:3850-3853 accepts delivery of an early action, but does not explicitly accept a
later missing wakeup. No highest exclusion is requested.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g154/f3`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
