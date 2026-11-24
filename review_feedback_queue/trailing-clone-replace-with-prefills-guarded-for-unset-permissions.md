# Clone and Replace with prefill Guarded instead of YOLO for cached or unmigrated sessions with unset or unsupported permissions

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

When you open Clone or Replace with on an older session whose permission mode was never recorded (for example, a cached
session or one on a remote host whose supervisor has not been upgraded yet, or a session with an unsupported stored
permission) for an agent that defaults to Guarded, the launcher prefills Guarded instead of YOLO — even though the
session list, session summary, plain Replace, and Restart with all classify that same session as YOLO.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/draft.rs:647`.

Coverage note: `TODO.md` carries an entry under `## Plan decisions to revisit` (from commit `f12ab9c0`, `#4689`) about
revisiting whether cloning old sessions should default to YOLO or Guarded. That bucket is not `## Planned`, so this
finding is recorded with that cross-reference for triage.

In crates/gui/farhelm-gui/src/launcher/draft.rs:512-544 and 607-660, LaunchDraft::clone_of and LaunchDraft::replace_with
call seed_launch(&session.launch) -> restore_selection(selection). Line 647 initializes permissions with
selection.harness.request_permission(selection.permissions), which maps None (and any unsupported explicit permission)
to the harness's omitted_permission() — Guarded for Codex, Claude, Antigravity, Muse, Grok, and OMP. By contrast,
RestartBaseline::of (draft.rs:116-127, line 124), permission::launched (permission.rs:61-67 via SessionLaunch::yolo()),
session_list/row_marks.rs:195, session_list/summary.rs:268, and YoloAsk::for_stored_launch
(launcher/view/yolo_question.rs:78) all classify stored launches via
selection.harness.stored_permission(selection.permissions), which treats None and unsupported permissions as
Some(LaunchPermission::Yolo) per SPEC.md (Creation, lines 1568-1570: "Farhelm treats an agent launch as YOLO when its
saved permission is YOLO, Custom, missing, or unsupported by the saved agent. An upgrade records YOLO on saved agent
launches with no recorded mode and preserves their start and resume commands. Cached or older records with no permission
follow the same rule."). Because supervisor schema 39 rewrites sessions.session_launch on upgraded supervisors, upgraded
sessions carry Some(Yolo), but cached sessions in helm DB or sessions on unupgraded remote supervisors (or launches with
an unsupported stored permission) still arrive with permissions: None/unsupported and diverge between Clone/Replace with
(Guarded) and Restart with/sidebar/plain Replace (YOLO). Notably,
crates/gui/farhelm-gui/src/launcher/draft_tests.rs:699-719 (replace_with_seeds_from_its_source) still asserts that
Replace with on a Codex session with permissions: None resolves to Guarded, whereas draft_tests.rs:882-894 (updated in
7b226839) asserts YOLO for Restart with on the same None permission.

How to verify: Inspect draft.rs:124 vs 647 and draft_tests.rs:699-719 vs 882-894, or open Clone/Replace with on a
SessionInfo whose SessionLaunch::Agent has harness Codex and permissions None.

Fix: Separate stored-session permission resolution (stored_permission) in seed_launch from recent-setup restoration
(request_permission) in restore_recent, and update replace_with_seeds_from_its_source (or if Guarded is intentionally
preferred for Clone/Replace with of older sessions, document that exception in SPEC.md and align migrated vs unmigrated
behavior).

Reviewer metadata (g20_yolo_unset_gui_launcher/l1): suggested bucket: other — UI/spec inconsistency for cached,
unmigrated, or unsupported-permission stored launches; prefilling Guarded is safer than YOLO and is visible in the
editable launcher before launch, so there is no security bypass or data-loss risk.; confidence: confirmed; possible
cover: commit_list.txt entry 67 (f12ab9c0, "docs: add a plan decision to revisit for cloning old sessions as yolo
(#4689)") suggests a TODO entry may already revisit whether cloning old sessions should default to YOLO or Guarded;
check TODO.md / plans/queue/ during recording..

Additional source record at `42996eb829d605bae99e03209a6f7ccc2e3d4478`, `SPEC.md:1568-1570`
(g21b_yolo_guarded_replace_spec/f2):

- SPEC.md:1568-1570 (at 1b3fe49af86979aebc2719965682a17f8c6366ac; added by 42996eb829d605bae99e03209a6f7ccc2e3d4478)
  states: "Farhelm treats an agent launch as YOLO when its saved permission is YOLO, Custom, missing, or unsupported by
  the saved agent. An upgrade records YOLO on saved agent launches with no recorded mode and preserves their start and
  resume commands. Cached or older records with no permission follow the same rule."
- SPEC.md:1596-1598 adds: "The usual Replace answers, agent clone and inherited spawn copy the stored command. For an
  older agent launch without permission flags this preserves the original command and applies the YOLO gate. Clone,
  Replace with and Restart with compile new choices."
- In shipped code at 1b3fe49af86979aebc2719965682a17f8c6366ac:
  1. Supervisor schema 39 (`crates/farhelm-supervisor/src/store.rs:2704-2712`) stamps
     `selection.permissions = Some(LaunchPermission::Yolo)` onto saved agent launches whose permission was `None` (it
     leaves unsupported explicit permissions unchanged).
  2. `LaunchDraft::clone_of` and `LaunchDraft::replace_with` (`crates/gui/farhelm-gui/src/launcher/draft.rs:512-544`)
     call `seed_launch` -> `restore_selection` (`draft.rs:647`), which computes
     `permissions: selection.harness.request_permission(selection.permissions)`. For a migrated session (`Some(Yolo)`),
     `request_permission(Some(Yolo))` yields `Some(Yolo)` (so Clone and Replace with preselect YOLO and compile explicit
     bypass flags, as commit `79279d67`'s changelog caveat notes). For a cached or un-upgraded record still carrying
     `permissions: None` (or an unsupported explicit mode), `request_permission` falls back to `omitted_permission()` =
     `Some(Guarded)` on Claude, Codex, Grok, Muse, Antigravity, and OMP — NOT YOLO.
  3. By contrast, `RestartBaseline::of` (`draft.rs:124`, used by `LaunchDraft::restart_with` at `draft.rs:554-571`)
     calls `stored_permission(selection.permissions)`, so Restart with preselects YOLO for both migrated and
     un-migrated/cached records.
  4. SPEC_impl.md:5206-5208 narrows the claim to "Stored launches use `stored_permission` so a missing or unsupported
     agent mode classifies as YOLO for the host guard and the shield even on older wire rows or cached rows", which
     omits that Restart with also uses `stored_permission` while Clone and Replace with use `request_permission`.
- How to verify: Inspect SPEC.md:1568-1570, 1596-1598, SPEC_impl.md:5204-5208,
  `crates/farhelm-supervisor/src/store.rs:2690-2717`, and
  `crates/gui/farhelm-gui/src/launcher/draft.rs:116-127, 512-575, 622-660` at 1b3fe49af86979aebc2719965682a17f8c6366ac.
- Fix: Align SPEC.md:1568-1570 / 1596-1598, SPEC_impl.md:5206-5208, and `draft.rs` on the intended Clone/Replace-with
  prefill for missing/unsupported saved permissions (and clarify in SPEC.md:1597 that Clone, Replace with, and Restart
  with of a schema-39-upgraded no-flag session now start from YOLO and compile explicit bypass flags, unless the
  behavior itself is changed).

Reviewer metadata (g21b_yolo_guarded_replace_spec/f2): suggested bucket: other (spec/implementation inconsistency; does
not reach `highest` because for cached/un-upgraded records `draft.rs:647` fails safer by prefilling Guarded instead of
YOLO, and for migrated records the launcher visibly displays YOLO selected and hosts that ask before YOLO confirm first;
note that on hosts with `yolo_without_asking` enabled, migrating old no-flag sessions to `Some(Yolo)` causes
Clone/Replace with to emit explicit bypass flags where they previously emitted Guarded, which is a product behavior
choice already reflected in commit `79279d67`'s changelog caveat rather than a docs misstatement causing a user
mistake).; confidence: confirmed (verified from source and spec at 1b3fe49af86979aebc2719965682a17f8c6366ac; no tests
executed).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g20_yolo_unset_gui_launcher/l1`,
`g21b_yolo_guarded_replace_spec/f2`.
