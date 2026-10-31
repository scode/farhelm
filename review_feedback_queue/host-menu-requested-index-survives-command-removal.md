# Host menu retains an index after commands change

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Keyboard navigation can stay on the wrong command position after Adopt or Update disappears from an open host menu.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The requested navigation index is not reconciled when the command list changes. Repeated End followed by Adopt removal
supplies a source-complete producer; the second review preserves a pending-arrow/Update-withdrawal variant. The menu
still dispatches its actual rendered command. Related session-menu acceptance does not clearly cover this host-menu
state, and no unintended activation or user-work loss was shown.

Source locations: `crates/farhelm-ui/src/hosts.rs:4130-4143,4248-4286`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed commit d6645f771bee8ed03a66486ef8755834937d759d. TLDR: after another client resolves a host identity mismatch,
the next Up key in an already-open host menu can stay on its last command rather than move to the previous command.
Discovery-pass finding, pre-existing at exclusive base da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c (hosts.rs 4072-4081);
not attributed to the highlight implementation. Concrete shipped sequence: in client A, with Hosts concealed, open a
mismatched remote host's rail menu onto its last item Remove and press End again while Remove already has focus; let the
focus request complete. handle_menu_key maps End to Last (menu_panel.rs 815-816,1306-1314); focus_menu_item sets
requested to the old last index (907-915). Same-element focus produces no focusin, and queue completion never clears
requested (407-450). Client B adopts the host using the normal action. The endpoint and manager replace IdentityMismatch
with Connecting and notify clients (farhelm-helm/src/hosts.rs 1272-1283; manager.rs 2542-2563). Client A withdraws Adopt
(hosts.rs 884-888,3267-3286) and keeps the same keyed HostRow (2911-2919). The accepted-host publication only
invalidates moved anchors, whose own-row changes are excluded (hosts.rs 1300-1325; list/view.rs 1846-1913); concealed
Hosts excludes aggregate trace/shape invalidations (1224-1265). Choose a host with no cached sessions and no
provisioning run to avoid unrelated changes. Remove remains focused and is reconciled to the shorter list, but requested
remains its old index: HostRow explicitly retains that old numeric position (4130-4143;4248-4286). menu_step_origin
gives it precedence (menu_panel.rs 706-711), and next_menu_focus discards the now-out-of-range index and chooses the new
last item for Up (1374-1387), yielding a no-op instead of stepping backward. The first bad arrow corrects the numeric
request, so this is a keyboard convenience/correctness issue. No command activation, unauthorized action or work loss is
established; do not infer highest from a person's acting on a visibly focused Remove. Suggested bucket other. Confidence
confirmed by static control/data flow, not runtime reproduction; no arbitrary native-delay premise is needed.
Proposed_drop none. Possible_cover: TRIAGE_OUTCOMES.md 7313-7331 fixes the same cause in session menus, but
assessment/surface and its destructive consequence differ; applicability of its broad completion wording to host menus
is ambiguous. SessionRow now uses carry_menu_request (list/row.rs 1495-1510), HostRow does not. SPEC_impl.md 879-880 and
924-936 require the shared menu keyboard behavior. Targeted searches found no exact Planned/BUGS/queue/unlanded-plan
coverage; TODO Planned is empty. The accepted mixed-input race rule concerns near-simultaneous control navigation,
whereas End and the other client's adoption can be arbitrarily separated here; no rare-window filter applies to the
established sequence. Suggested repair: remap or clear requested by action in HostRow reconciliation as SessionRow does,
correct the stale-index comment, and add a host action-withdrawal regression covering a settled same-target End request.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g102/f2`, `g133/f2`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
