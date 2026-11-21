# The rail can acknowledge an off-screen PATH warning and clear unseen history

Reviewed commit: 1d4fd8a723e5a195f61e625ecf62e8c3a347eb29

## TLDR

Opening and closing Notifications can mark an off-screen PATH warning as read. Clear resolved can also remove unseen
history.

## Details

**other**. Confirmed by source and contract inspection.

Confirmed by source: rail_notices.rs:425–465 sorts the entire collection newest-first and stores the unread/resolved ID
sets before layout; :473–492 and :525 render those rows in a vertical scroll region. Notices_bell.rs:686–712 caps the
panel to the window, and focus.rs:1066–1074 and :1093–1097 explicitly distinguish drawn children from controls scrolled
outside a region. The release sends the full unread set (:351–357), and Clear resolved sends the full resolved set
(:364–373, :494–503). Helm notices.rs:532–546 persists these exact IDs. A PATH notice sorts below newer host or app
notices, and an unread warning still active across failed launches retains its first raise time (helm
notices.rs:246–266), so a clipped row can participate without being visible. SPEC.md:3067–3071 says close “marks only
the items it showed” and SPEC_impl.md:8303–8307 says marks are for “the unread notices the last frame drew” and “Clear
resolved removes only the resolved notices the panel showed.” Existing rail_notices/tests.rs:483–520 checks a store
update without a frame, not rows outside the viewport. The selection behavior predates the reviewed commit, which adds
the PATH condition to that same panel. Bucket other: diagnostic visibility/history, with no established loss of user
work. Possible_cover: none from the governing spec. No runtime observation. The relevant rail, its tests and the notice
store have identical source at the reviewed commit and pinned tip 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f. No later
in-range fix. Synthesis: the two operations have different persisted effects. Read marks written for an off-screen
active row quiet later failed launches too (reconciliation leaves active rows and their read state alone), but leave
history. Clear resolved permanently removes the off-screen saved row only while it is resolved. The store checks
`id = ?1 AND resolved = 1`. If failure recurs after a clear, it creates a different unread ID. A stale clear does not
delete a reopened active row or replacement, although an unseen clear can consume the retained quiet-history behavior
and make a later recurrence re-alert. The backend's state and identity guards work as specified. Viewport membership is
the defect. Neither action influences the PATH selected by the app or the session supervisor. Bucket remains other.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `path-notices/f5`.
