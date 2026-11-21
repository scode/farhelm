# A failed notice save leaves the desktop silent or showing stale PATH status

Reviewed commit: 1d4fd8a723e5a195f61e625ecf62e8c3a347eb29

## TLDR

A failed notice save can suppress the login PATH warning or leave an obsolete warning visible until a later startup.

## Details

**other**. Confirmed by source and contract inspection.

Confirmed source handling, conditional on a notice transaction error. At notices.rs:466–480, write/commit failures
propagate and a failed transaction does not update the stored row. Startup lib.rs:1422–1429 logs and drops the
observation, then proceeds to readiness (:1516–1537). Subsequent notice reads only load saved rows (notices.rs:483–517).
Git search for both `desktop_login_path_available` and `record_desktop_login_path` found no production retry or
in-memory fallback. A write-specific storage failure can leave reads available. The mandatory checkout baseline at
lib.rs:1519 is a read, and the release producer can enqueue nothing (release_announcements.rs:40–55). SPEC.md:3093–3097
promises the alert and later resolution. SPEC_impl.md:3922–3925 says reconciliation happens before serving. Unlike
trash's specified notice_error at SPEC_impl.md:3912–3914, this path exposes no save failure to the user. Bucket other: a
conditional diagnostic failure, with no established loss of user work. Possible_cover: none in the governing spec. No
runtime observation. The 5f97d10518d6b2e598ecab8e8387e0f78f6e68d8 relaunch fix changes the inherited PATH passed to the
next desktop process and leaves this save handling intact. No later in-range fix through
8bcca67df1afe6eb0f184ef73b9952ed7ebb329f. Synthesis: the PATH decision is already applied before the transaction
(desktop lib.rs:199–224), so a failed save leaves the outcome used for tool lookup intact. It blocks a notice-state
change only when that result calls for one. Failure with no row loses the new diagnostic, and a failed reopen leaves
retained history resolved and clearable during the ongoing fallback. A failed resolution keeps an active warning with
its old read bit even when the app is using a shell PATH. Consecutive identical results normally leave the row as-is.
Subsequent starts can reconcile their own results but there is no replay of the dropped one. This save-only failure is
separate from the startup delay/absence in desktop-path/f2 and the actual lookup/fallback changes in desktop-path/f1 and
/f3. Bucket remains other.

The saved warning can persist across ordinary refreshes after storage recovers. The filter for rare imprecise
diagnostics excludes persistent wrong state.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `path-notices/f4`.
