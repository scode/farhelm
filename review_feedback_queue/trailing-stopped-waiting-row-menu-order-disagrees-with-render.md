# A stopped-waiting container row renders an item omitted from keyboard navigation

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: while a stopped container waits for its container host, its row menu visibly includes disabled Stop and suspend, but keyboard arrow navigation skips it. Focus bookkeeping also ignores it if assistive technology focuses it directly. This is a disabled-menu navigation inconsistency, not an unintended lifecycle mutation.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g035/c1`, `g035/l2`.

### g035/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 13 (candidate, open):

Reviewed commit: 350a4b33b6d4476f4a78e43cea124bc2b828f462. Discovery revision: preserve the original possible incorrect/unreachable lifecycle-menu consequence and correct the earlier projection refutation. Actionable Resume/Stop direction remains guarded for waiting_for_place=true, as previously checked, but disabled does not mean absent from the menu-navigation contract. New record l2 establishes the specific projection mismatch: all-Stopped menu_order omits StopSuspend, while Controls projects waiting Stopped to resume=false/changing=true and renders a disabled Stop and suspend item. c1 therefore no longer proposes dropping the entire menu-projection concern. l2 is the retained actionable record for this newly checked accessibility consequence; avoid counting c1 as a second finding. g025/f3 remains separate rendered-event direction authority, not settled here. Suggested bucket: other for l2's established navigation consequence. Could-be-highest: no; no mutation/security/work-loss route. Confidence: source-confirmed narrower defect, no execution. proposed_drop: only the previously considered enabled wrong-direction projection remains source-refuted; disabled-item reachability is not dropped and is reported as l2. possible_cover: l2 within this own log, g025/f3 distinct. No parent drop or lowering assumed.

Returned proposal (claims-021, event 13): **duplicate**, suggested bucket **other**.

The latest record correctly withdraws the broad projection refutation. Its retained disabled-item defect is exactly g035/l2; record that defect once. The original enabled wrong-direction projection is source-refuted. This does not dispose of the separately owned refreshed-callback direction or destructive-guard concerns.

Original stronger consequence: Original incorrect or unreachable lifecycle-menu direction; earlier refutation was narrowed after the disabled-item defect was found. Refreshed-event direction and destructive guard attribution were explicitly excluded and remain separately owned.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Same local trigger, implementation mismatch and navigation consequence as g035/l2; no second queue item. Preserve the c1 correction in provenance.

Existing parent disposition (event 13): duplicate. Parent read both claims-021 objects and accepts the same-owner reconciliation:350a4b33 row.rs:249-268,1377-1381 omits StopSuspend from order for every Stopped row, whereas sandbox_sessions.rs:230-245,338-341 and row.rs:2798-2810 render its disabled command when waiting_for_place=true. g035/l2 carries that exact keyboard/navigation defect. c1's broad enabled wrong-direction suspicion was narrowed by the disabled state and stored Operation, with its earlier wording retained; separate refreshed-event and destructive-consent claims are not absorbed. No highest consequence was alleged.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g035 record](records/review-through-00de0b6c/groups/g035.json). Record ID: `c1`.

### g035/l2

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 14 (finding, open):

Reviewed commit: 350a4b33b6d4476f4a78e43cea124bc2b828f462. TLDR: while a stopped container waits for its container host, its row menu visibly includes disabled Stop and suspend, but keyboard arrow navigation skips it. Focus bookkeeping also ignores it if assistive technology focuses it directly. This is a disabled-menu navigation inconsistency, not an unintended lifecycle mutation. Details: the changed sandbox_menu_state at row.rs:1377-1382 treats every HostPhase::Stopped as true. session_menu_order_with_sandbox at row.rs:249-268 includes StopSuspend only for Some(false), so it excludes that action. Controls has a different boundary: sandbox_sessions.rs:230-245 counts Stopped as suspended/resume only when waiting_for_place=false; waiting_for_place=true sets changing=true and suspended=false. Its ControlButtons at 340-341 therefore carries resume=false and disabled=true. The row adapter at 2797-2821 renders Stop and suspend whenever !controls.resume, leaving a visible aria-disabled menu item absent from menu_order. It always has tabindex=-1; arrow/Home/End selection uses only order in menu_panel.rs:781-819 and 893-913, and record_menu_item_focus returns early when order has no position for the action (737-743). Tab treats current=None as the toggle rather than an item (795-796). The manager publishes the waiting phase from its real place-connectivity gate at manager.rs:3842-3861 and 3889; cached session rows remain inspectable on unavailable hosts. The existing manager test at 13220-13252 confirms the intended state model with an unavailable place and a stopped environment, but was read only, not run. SPEC_impl.md GUI: Dioxus at 1025-1037 says disabled commands remain focusable, arrows step from whichever item actually has focus, and inert menu items stay focusable with aria-disabled. Keep the rendered and navigable action projection identical, including the waiting-for-place distinction, or omit the item from both. Attribution: the full exclusive-base-to-reviewed diff changes the row's predicate from only sandbox_suspended to all Stopped in this commit; Stopped/provider controls are newly introduced in the range. Endpoint diff through 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db retains the all-Stopped predicate and the ControlButtons.resume boundary; later retired-row filtering does not fix ordinary provider-backed container rows. Suggested bucket: other, narrow keyboard/accessibility correctness. Confidence: confirmed source/contract projection mismatch; physical menu navigation not executed. Could-be-highest: no; changing disables the action and no request or loss-of-work route is alleged. proposed_drop: false. possible_cover: c1 shares the state projection but did not settle disabled-item keyboard reachability; g025 direction binding is a different cause/consequence. No forbidden coverage source, runtime test, reproduction or live-install action used. No missing source premise remains for the conditional rendered mismatch; runtime incidence unmeasured. Follow-up admission check: row.rs:2148-2156 deliberately leaves the menu toggle enabled under unavailable/nav-lock conditions. ListView at 5919-5954 renders cached rows and derives row busy only from local pending/session claims, not Stopped. Thus an otherwise idle stopped-waiting row can open the mismatched menu; no active launch or local lifecycle claim is required. The frozen projection dims the row and disables individual lifecycle items, but does not suppress the panel. No execution was used for this premise.

Returned proposal (claims-021, event 14): **retain-verified**, suggested bucket **other**.

A supported stopped/waiting-for-place state draws a disabled Stop and suspend item outside the menu's navigation order. Disabled items are part of the specified keyboard contract. The condition can last as long as its container host is unavailable; rarity filters do not cover it.

Original stronger consequence: Disabled Stop and suspend is skipped by keyboard navigation and ignored by focus bookkeeping. No unintended lifecycle request or user-work-loss consequence was alleged; refreshed enabled callback authority remains a distinct record.

Unresolved premise or limitation: Runtime incidence/navigation has not been measured; no missing source premise remains for the stated conditional mismatch.

Proposed basis: New other-priority queue item; carry g035/c1 only as history/alias.

Existing parent disposition (event 14): verified. Parent explicitly accepted named retain-verified proposal from claims-021 after inspecting it. A supported stopped/waiting-for-place state draws a disabled Stop and suspend item outside the menu's navigation order. Disabled items are part of the specified keyboard contract. The condition can last as long as its container host is unavailable; rarity filters do not cover it. New other-priority queue item; carry g035/c1 only as history/alias. Checked source/results: ["350a4b33b6d4476f4a78e43cea124bc2b828f462:crates/farhelm-ui/src/list/row.rs:1377-1381 treats every Stopped as resume-only; :249-268 then excludes StopSuspend. sandbox_sessions.rs:230-245,338-341 makes stopped/waiting resume=false and disabled=true; row.rs:2798-2810 nevertheless renders that item with tabindex=-1.","Same revision:menu_panel.rs:737-743 ignores focus on absent-order actions; :781-819 and :893-914 navigate only order entries. This establishes the conditional rendered/navigation mismatch, not a mutation.","Same revision:crates/farhelm-helm/src/manager.rs:3845-3861 publishes waiting_for_place from the real place-host connectivity gate. row.rs:2149-2156 leaves its menu toggle available; list/view.rs:5919-5948 renders rows and derives local busy from claims/pending requests, not Stopped. A locally idle waiting row can open this menu.","Tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:list/row.rs:1441-1445 retains the all-Stopped order projection. Retired-only filtering is a different row class. Source only; no physical navigation observed."] Current coverage/results: ["SPEC_impl.md:1025-1046 says disabled commands remain focusable and arrows follow actual focused items. SPEC.md:4806-4891 supports Stopped/Resume and unavailable container-host presentation; it does not waive keyboard access.","Planned and the unlanded-plan index are empty; BUGS has no menu exception. INDEX.md and active ledger searches found no stopped-waiting order item. sandbox-row-menu-owns-transient-lifecycle-state.md was read and concerns scope teardown, not omitted keyboard order.","FILTER.md: no required short-lived timing window or other matching trigger. A disabled command being unreachable by keyboard is also outside the scrollable-explanation filter."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "Disabled Stop and suspend is skipped by keyboard navigation and ignored by focus bookkeeping. No unintended lifecycle request or user-work-loss consequence was alleged; refreshed enabled callback authority remains a distinct record."; "Runtime incidence/navigation has not been measured; no missing source premise remains for the stated conditional mismatch.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g035 record](records/review-through-00de0b6c/groups/g035.json). Record ID: `l2`.

