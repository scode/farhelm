# Cancelled orphan delete accepts queued confirmation

Reviewed commit: 8bd92e1336565dfc263fe5f3fa2fef413a82a87a

## TLDR

A queued Delete answer can still destroy orphaned cloud resources after Cancel was accepted in the same rendered
question.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: highest.

Source location: `crates/farhelm-ui/src/tensorlake/orphans.rs:167-172 at 8bd92e1336565dfc263fe5f3fa2fef413a82a87a`.

Source assessment (read with the later corrections below) (review record g041/f4, event 24):

Discovery interface reconciliation, not a new discovery. Reviewed 8bd92e1336565dfc263fe5f3fa2fef413a82a87a; same
behavior originated at c340166d754d3edb3de7fd538c6408948d3536c6:118-135 and remains at pinned
8a8c9579e04299410652f0e4681975a7198dfc7b:163-180. TLDR: Cancel does not revoke deletion authority before a queued Delete
in the same render, so a sandbox and its files, sessions and snapshots can be destroyed after cancellation. Newly read
decisive source: pinned ops.rs:314-342 in full, especially :317-328, explicitly states EVERY confirmation can receive
button clicks in one event burst before removal rendering, first answer wins, and a previous Replace actually shipped
Cancel-then-confirm replacing the session. This supplies the application event-model/source premise I previously
required; it is not merely frequency speculation or arbitrary Rust callback invocation. Orphan Cancel :167 clears Option
and leaves busy false; Delete :168-172 only checks busy, then dispatches rendered sandbox ID without token; backend
tensorlake/orphans.rs:218-243 terminates it and deletes snapshots. Existing version-matched Dioxus 0.7.10 inspection
corroborates notification schedules render (signals signal.rs:255-267,527-536; core reactive_context.rs:103-114),
synchronous web event dispatch (web dom.rs:88-110), current mounted-listener lookup (core runtime.rs:359-379,418-460),
later render/removal (web lib.rs:259-264; core diff/node.rs:269-297). ca28b604
e2e/tests/tensorlake-hosts.spec.ts:174-208 is creation-only same-burst regression evidence, not an orphan runtime
reproduction. Read named g054:g054-l2 and g061:g061-c3 records; they are duplicates and correctly identify the newly
read application contract. Suggested bucket highest, confidence source-confirmed supported same-render route and
destructive consequence. Prior other recommendation rested on missing application producer evidence; that premise is now
supplied, so the prior lowering is superseded, not retained as a competing severity judgment. Preserve qualifications:
no dynamic orphan reproduction, human timing frequency/desktop IPC realization not measured, and no claim that old
callbacks survive COMPLETED unmount; current-listener lookup refutes that arbitrary post-retirement theory. Target
capture remains correct and distinct from opening authority. Add-host Back/Continue/async-probe/reopen crosses renders
and is not established by this same-render precedent; no extension to that route. SPEC_impl.md Answers bind to what was
drawn and ops first-answer contract require ConfirmSlot token-aware synchronous take/cancel before dispatch.
proposed_drop false; possible_cover confirmed duplicates g054:g054-l2,g061:g061-c3, parent consolidates. No highest
lowering or drop proposed. No runtime/network/process/browser experiment.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g041/f4`; supporting records
`g054/g054-l2`, `g061/g061-c3`, `g073/f2`, `g074/f1`. The retained private log preserves earlier wording and its
corrections. This record does not claim runtime reproduction.
