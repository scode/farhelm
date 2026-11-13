# Expanded Delete error handling retains a false no-change explanation

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: Expanded Delete error handling retains a false no-change explanation

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g038/l4`.

### g038/l4

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 21 (finding, open):

Reviewed commit: 2ef10815f12ed719ab6c51c6e3ac6fcd3f8bc1fa

## TLDR
The expanded Delete error handler tells future maintainers that an unsuccessful result changed nothing on the host and cannot produce a fleet event. Newly handled unanswered supervisor requests can already have removed the session or sealed the environment. The current refresh is appropriate; its explanatory invariant is wrong.

## Details
At view.rs:3060-3064 the moved comment says "A refusal changed nothing on the host, so no fleet event will correct a stale row" and justifies refresh as correcting a row the supervisor found alive. It is after all UnaskedFollowUp alternatives inside Err((e, unasked)), not in a definite-refusal-only branch. The assigned patch adds Unasked ingress at :2995. API api/sandbox_sessions.rs:162-173,224-234 recognizes a received Conflict with the helm marker as Unasked. sessions/sandbox.rs:220-239 sets that outcome after client.delete_session_or_seal was sent but timed out or lost a supervisor transport answer; :120-122 expressly states such a request may already have been acted on. Thus this is not merely the ambiguity of the nothing-decided label or confusion between losing a supervisor answer and losing the helm-to-page reply.

Direct event/source premise settled: supervisor service/handlers.rs:1443-1445 starts the mutation separately; :1515-1518 records the count/seal. The Sealed branch stores its in-memory seal and calls hint_sessions_changed at :1547-1553 before awaiting finish_teardown (:1554-1562). The nonfinal path awaits ordinary finish_teardown at :1529-1541; service/teardown.rs:1227-1230 removes the session and hints the change on the mutation task, with an explicit comment that its reply waiter may be abandoned before that point. The same-revision test description at teardown.rs:2579-2585 independently states that the supervisor-owned teardown can finish and hint after a connection-owned reply waiter is aborted; it was read, not executed. An Unasked UI result therefore does not establish no host change, and a related invalidation is allowed. Even the new contract distinguishes a pre-decision busy refusal, which really changes nothing, from a sent request whose result is unknown (SPEC_impl.md Environment Delete and the seal).

Full-range attribution: the exact sentence is already present at exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a view.rs:2750-2754 and at the immediate parent. The base error path handles untyped String errors from ordinary delete_session or whole-resource delete_sandbox_session; base api/sandbox_sessions.rs:132-148 has only delete_sandbox=true. This record does not claim the words were first authored here or that their older generic transport-error applications were universally correct. The assigned 2ef10815 diff introduces Session-scope environment Delete and typed marked Unasked handling, and moves the old rationale below all newly expanded alternatives. This new documented application to explicitly sent-without-answer operations is inaccurate within the assigned change. At inclusive tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db view.rs:3023-3028 retains the comment after the same Unasked branches (:2985-3022). The API and helm environment-delete files have an empty reviewed-to-tip diff. No later repair changes the documented domain.

Contract/fix: root AGENTS.md requires nearby explanations to track changed async and cross-phase invariants. SPEC_impl.md Environment Delete and the seal requires the supervisor to hint seals and the helm to finish those learned through listings, including when the request answer is unknown. Explain the refresh for both cases: a definite refusal may not generate any event, while an unknown result may already have changed host state, so explicitly reread without assuming rollback or future event delivery. No production behavior change is required for this independent documentation defect.

Suggested bucket: other, internal maintenance guidance only. Confidence: confirmed source/comment contradiction for the expanded error domain, not runtime observed. proposed_drop: false. possible_cover: named focused claims-023/parent late input, not an accepted finding or coverage search. Could-be-highest: no established security, process-loss or data-loss route from the prose; current code still refreshes and separately confirms wider destruction. Original consequence preserved: future readers receive a false no-state-change/no-event invariant. l3 retains its exact scoped terminology proposed-drop bases and refutation history; they do not accept this stronger claim. Caveats: this is newly expanded misuse of an inherited explanation, not first-ever creation of all possible historical no-change misstatements. No broader runtime defect or actual reader-caused harm alleged. No tests, builds, reproductions, installs, live actions or checkout writes. No remaining readily checkable premise or open lead for this finding.

Existing parent disposition (event 21): verified. Parent read complete new events18-21, exact reviewed view.rs:3038-3072 and its2ef10815 diff, and current full Environment Delete/seal contracts. The inherited categorical no-host-change/no-fleet-event comment now sits below newly typed Unasked errors, including a sent request whose answer is unknown. Supervisor-owned mutation/hint source at handlers.rs:1443-1562 and teardown.rs:1227-1230 establishes why that is false, independently of current refresh correctness. The words and some generic-error applications predate base; the new explicit sent-without-answer domain is introduced in this interval, and tip retains it. Record that attribution narrowly, without claiming all historic false uses originated here. Coverage searches across current specifications/Planned/BUGS/unlanded-plan index/queue index/files/activeoutcomes found only the governing uncertainty/seal rules and no exact accepted explanation or existing item. FILTER has no persistent-source-comment exception. This is other maintenance guidance, not observed bad deletion or user-work loss; original l3 shorthand refutation remains separate.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g038 record](records/review-through-00de0b6c/groups/g038.json). Record ID: `l4`.

