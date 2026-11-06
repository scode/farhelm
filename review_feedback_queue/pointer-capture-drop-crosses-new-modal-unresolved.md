# Pointer capture drop crosses new modal unresolved

Reviewed commit: 720e18bf8e78339e59bb876af75a621bfd1a69e9

## TLDR

A captured drag may finish after a new modal appears and still reorder rows beneath it. The native event sequence
remains unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location:
`crates/farhelm-ui/assets/pin-order.js:107-118,152-158 and crates/farhelm-ui/src/list/view.rs:3458-3479 at 720e18bf8e78339e59bb876af75a621bfd1a69e9`.

Source assessment (read with the later corrections below) (review record g085/l2, event 30):

Reviewed commit: 720e18bf8e78339e59bb876af75a621bfd1a69e9. TLDR: Opening New with the keyboard while holding a pin drag
may allow the subsequent release over the modal or scrim to reorder pins behind it. This is distinct from l1: there need
be no geometry change or stale insertion line. Unverified after investigation. Source route: start an eligible drag and
move to a real target; native Mac Cmd+N invokes the live New button (lib.rs:1735-1753), and open_new sets show_create
without an operation claim (view.rs:3675-3706). create_form.rs:5008 only claims the shared lock at submit. Its
ModalDialog (6227 onward) inerts surrounding content through modal_isolation.rs:175-194 but never cancels the pin
gesture. PinDragList is hidden only for Hosts, and its JS sample sees an otherwise connected/rendered root, an enabled
source and coordinates inside the geometric bounds; it does not inspect inert ancestors or covers. Rust receipt checks
hosts_panel_open/operation/source locks, but not show_create or another modal. Thus a delivered captured pointerup
samples and sends the old-list move even though the list is no longer an interactive drop surface. Existing
SPEC_impl.md:778-805 describes modal page isolation; SPEC.md Session list says releases outside the pinned group cancel.
A new modal's covered background should not remain a drop destination. The One GUI at a time short pre-redraw acceptance
does not establish this as accepted: the sequence can keep the modal fully drawn for more than a second before release,
and it is a list gesture rather than an answer to a covered dialog. Checks/results: reread complete pin asset, relevant
Rust guards, modal isolation installer, actual Cmd+N producer and New opener/claim boundaries. Official HTML Inert
subtrees https://html.spec.whatwg.org/multipage/interaction.html#inert-subtrees describes hit-test exclusion and focus
restrictions, not capture release. Immutable official WebKit f2f0dead988af103f8106b25078617041fa67a03 source was
selectively inspected: Source/WebCore/page/PointerCaptureController.cpp:67-73 returns the capture override without an
inert check; dispatchEvent:582-595 dispatches it; Source/WebCore/dom/Element.cpp:497-618 dispatches the pointer before
compatibility handling without an inert test; Source/WebCore/dom/EventDispatcher.cpp:189-285 dispatches the event path,
and its shouldSuppressEventDispatchInDOM:129-145 covers editing-related key/text events only. These paths support
capture bypassing inert hit tests. Exact remaining premise: whether the supported deployed WKWebView/platform path
preserves this capture across inerting and modal focus transfer, or emits an intervening cancellation/retarget not
established by the inspected entry paths. The inspected engine snapshot is not mapped to a deployed build, and no
source-only check can substitute for the final event trace at that UI/platform boundary. A focused regression that
performs the real Cmd+N opener during actual pointer capture, waits for modal isolation, releases inside former bounds
and asserts zero pin-order writes would settle the observable. No runtime test was run. Suggested fix if reproduced:
cancel a gesture when the list becomes inert/covered, and reject such drops in the geometry/bridge admission path.
Suggested bucket other: recoverable unintended metadata order only; no security, process or user-work loss route.
Confidence likely, with the deployed capture-survival premise above missing. proposed_drop false; possible_cover
unknown, excluded sources not searched. Temporal scope: pin JS/bridge are unchanged through
8a8c9579e04299410652f0e4681975a7198dfc7b; tip lib.rs:1744-1759 retains the same Cmd+N producer and prior tip receipt
checks still lack a modal guard. No later repair found in inspected relevant paths. Novel local discovery concern, not
g086:f1 or a supplied refutation.

Later qualification (review record g085/l2, event 34):

This supplements the full l2 finding/event immediately preceding the l3 discovery record; preserve that event's
mechanism, checks, suggested regression and exact unresolved deployed capture-survival premise. Reviewed full hash
720e18bf8e78339e59bb876af75a621bfd1a69e9. TLDR: New's modal may leave an already captured pin gesture able to reorder
covered rows. Suggested bucket other, confidence likely, unverified after investigation at the deployed WKWebView/input
boundary; proposed_drop false; possible_cover unknown. Final bounded checks: pinned tip view.rs:3616-3637 still checks
only Hosts, shared/pending/source operation state and pin membership, not modal coverage; tip view.rs:4029-4059 still
opens New without a lock claim; tip lib.rs:1744-1759 still has the Cmd+N producer. The only later change among
create_form.rs, modal_isolation.rs, pin_drag.rs and pin-order.js is 1a57aa8e's Tensorlake resume display/preparation
change in create_form.rs; its diff does not alter modal isolation, mount/open claim ownership or pin capture.
modal_isolation.rs and both pin bridge files have no later changes. No repair found through
8a8c9579e04299410652f0e4681975a7198dfc7b. This remains a novel investigated uncertainty from discovery pass 1; no
runtime experiment or new browser-version claim was added.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g085/l2`; supporting records
`g083/c10`. The retained private log preserves earlier wording and its corrections. This record does not claim runtime
reproduction.
