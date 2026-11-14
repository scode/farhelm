# Retired-only bulk Delete cancellation has no rendered focus return

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: Retired-only bulk Delete cancellation has no rendered focus return

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g035/l1`.

### g035/l1

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 8 (finding, open):

Reviewed commit: 04d85f0c98a9b635709d00b056719a0baa7fd3fc. Revision reason: caller requested actual rendered-return
chain and terminal/toolbar fallbacks, rather than inferring user focus solely from a missing selector. Original
consequence retained: cancelling bulk Delete with only retired rows marked leaves keyboard focus without the promised
return target. Source chain: MetadataAction::applies rejects retired rows (marking.rs:102-106); order at 234-243 is
empty; the changed conditional at 358 removes the metadata toggle. MarkedBar still renders .marked-delete, .marked-clear
and tabindex=-1 .marked-count (337-397), but CANCEL_FOCUS references none of them: lifecycle.rs:264 selects only
.marked-menu-toggle then .sort-title. ListView renders SortMenu only in the else branch when effective_marks is empty
(view.rs:5391-5416), while cancellation takes only the confirmation token and preserves marks (4375-4379). Thus neither
selected return node exists. Cancel, Close and Escape all enter that same on_cancel (lifecycle.rs:310-335, 374-376).

Fallback checks: DialogHandle::close releases modal isolation and schedules exactly that return body
(modal_dialog.rs:137-149). Its missing-target behavior is explicitly a no-op. modal_isolation.rs:212-223 removes
inert/listener state without restoring a saved opener; its DOM-removal observer only releases. ScrollRegion's recovery
at modal_dialog.rs:577-582 requires both region and dialog connected/usable, and its observer disposes on removal at
644-646; it cannot hand focus to the toolbar after unmount. DialogFocus::remembered stores the last control inside the
modal (794-809), not an opener-restoration policy. A surviving .marked-delete is therefore available but never selected
by this cancellation chain. Explicit terminal.focusSelected is a separate invoked method (assets/terminal.js:3001-3012);
no call occurs from this cancellation. For the particularly clear premise of selecting the retired session itself,
keep_terminal_surface rejects retirement (session_view.rs:3724 onward; test at 4325-4327), and the view draws a
nonfocusable status-only retired card at 2850. There is no terminal to receive a hypothetical home-focus handoff. This
does not require the selected session or row to disappear: cancelling removes the modal only. If independent user
navigation or unrelated asynchronous work supplies focus, the eventual active element can differ; the finding is the
missing deterministic return under otherwise stable state, not a universal claim about every concurrent UI state.

Shipped trigger premise remains explicit: the retired-only marking must include a row whose last-known agent is not
ended or has tabs, with confirmations enabled, so ordinary bulk Delete asks. DeleteTarget::needs_confirmation uses those
visible facts (shared.rs:44-60); retirement identifies stored launch intent without normalizing those facts, and the new
ticker exclusion preserves retired tabs for whole-session cleanup (service/ticker.rs:2098-2100, 2167-2169). No claim
that a terminal exists for that retired row or that all retired rows necessarily ask. SPEC.md Keyboard focus: 'Closing
something the user opened returns focus to where it was before it opened, on every platform'; Session list preserves
marks on cancellation. SPEC_impl.md's existing combined-confirmation cancellation target is the enabled toolbar menu,
whose new absence is the regression. Fix can choose a surviving .marked-delete or .marked-count fallback. Suggested
bucket: other, narrow keyboard-navigation impact. Could-be-highest: no. Confidence: confirmed source/contract mechanism
under the named reachable state; physical post-close activeElement not observed because no execution/reproduction was
requested. proposed_drop: false. possible_cover: none. The earlier unqualified wording is narrowed to this stable
rendered-state premise, without dropping or lowering any loss-of-work allegation (none was made).

Returned proposal (claims-021, event 8): **retain-verified**, suggested bucket **other**.

Under stable retired-only marking, cancellation invokes a return script whose two possible destinations are both absent.
This is a supported keyboard-focus regression, not an inferred destructive consequence.

Original stronger consequence: Keyboard focus lost after cancelling retired-only bulk Delete. Later wording qualifies
the stable rendered-state premise; no security or user-work-loss consequence was alleged.

Unresolved premise or limitation: Physical browser focus after close was not measured. The established defect is the
missing deterministic rendered return target under stable state; unrelated navigation may independently place focus.

Proposed basis: Keep a new other-priority item. Existing focus items share a destination policy but require different
fixes at different close owners; they are related, not exact coverage.

Existing parent disposition (event 8): verified. Parent explicitly accepted named retain-verified proposal from
claims-021 after inspecting it. Under stable retired-only marking, cancellation invokes a return script whose two
possible destinations are both absent. This is a supported keyboard-focus regression, not an inferred destructive
consequence. Keep a new other-priority item. Existing focus items share a destination policy but require different fixes
at different close owners; they are related, not exact coverage. Checked source/results:
["04d85f0c98a9b635709d00b056719a0baa7fd3fc:crates/farhelm-ui/src/list/marking.rs:103-106,234-242 rejects retired rows
from every metadata action, and :358-389 omits the toggle when order is empty. The marked count, Delete and Clear
survive at :337-395.","Same revision:list/view.rs:5391-5417 chooses MarkedBar instead of SortMenu while marks remain.
:4375-4379 consumes only the question and calls lifecycle::CANCEL_FOCUS, so marks and the absent-sort state
persist.","Same revision:list/lifecycle.rs:263 selects only .marked-menu-toggle or .sort-title; :313-327 routes Escape
and Close through on_cancel. modal_dialog.rs:137-148 schedules only that supplied body and explicitly documents
missing-target no-op behavior.","Same revision:list/shared.rs:44-65 confirms live/with-tabs rows can require
confirmation. The claim remains conditional on retained last-known live/tab facts and confirmations enabled, not on all
retired rows. Tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db still omits the empty-order toggle (marking.rs:402) and uses
the same cancellation selector (lifecycle.rs:417). Source only; activeElement was not observed."] Current
coverage/results: ["SPEC.md:2176-2240 preserves marks on cancellation; :3282-3287 requires opener focus return.
SPEC_impl.md:1204-1214 prescribes the enabled toolbar menu for cancellation but does not accept its absence.","Planned
is empty (TODO.md:25-27); plans/queue/INDEX.md has no unlanded entries. The landed podman retirement plan supplies
Delete-only context, not a focus fix.","BUGS.md has no focus exception. INDEX.md searches found
templates-close-targets-disabled-bulk-button.md and bulk-completion-focuses-sort.md; both files were read. Templates
targets a disabled surviving button, and bulk completion removes marks/toolbar. Neither covers cancellation with
preserved marks and no toggle. Active ledger menu-close/menu-blur discard sections concern unrelated delayed focus
ordering.","FILTER.md does not match: no narrow race, crash or unusual-size premise is required; the missing return
repeats for the supported stable state."] Exact cited cover: [] Original stronger consequence and missing premise remain
preserved: "Keyboard focus lost after cancelling retired-only bulk Delete. Later wording qualifies the stable
rendered-state premise; no security or user-work-loss consequence was alleged."; "Physical browser focus after close was
not measured. The established defect is the missing deterministic rendered return target under stable state; unrelated
navigation may independently place focus.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g035 record](records/review-through-00de0b6c/groups/g035.json). Record ID: `l1`.
