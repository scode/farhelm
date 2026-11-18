# Destructive-action tests can pass with the wrong target

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Several GUI regressions would pass if Farhelm sent a destructive request for a different session, host, credential, or
recipe. One Tensorlake test does not check that its request clears the key.

## Details

Haiku test pass, checked by gpt-6.2-sol and spot-checked at the reviewed commit. Suggested bucket **other**. Confidence
is source-confirmed for the missing assertions. These tests exercise maintainer fixtures. The weakness does not
establish a production wrong-target bug. GUI paths below are relative to `crates/gui/farhelm-gui/src/`.

### Sessions and checkout notices

- H-T3-1, `launcher/pending_tests.rs:193–201`. The pending-launch Delete counts the method. Match
  `DELETE /api/sessions/a` before using it as the test's premise.
- H-T3-2, `launcher/replace_with_tests.rs:33–35,407–410`. The in-flight Replace premise accepts any `/replace` suffix.
  Assert the command names a before checking that the later press sends nothing.
- H-T3-3, `launcher/restart_with_tests.rs:38–42,283–292,585–601,724–737`. The idle-agent and YOLO cases inspect the body
  through helpers that accept any `/restart` suffix. Check `/api/sessions/a/restart` and assert that the retry keeps the
  first request's target. The consent case at :255–267 already checks the full path.
- H-T4-1, `session_list/delete_notice_tests.rs:60–89,105–186,194–223,229–260`. The helper matches by method then checks
  the store's `deleted_here(key)` state. Assert the request path names the passed session before replying with its
  archive outcome.
- H-T5-1 and H-T5-2, `session_list/menu_tests.rs:1369–1375,1478–1480,1519–1521`. The tests discard the Delete path.
  Require a for the refusal and first selection case and b for the visit-history case. Scope the refusal text assertion
  to a's row. The separate drawn-guard and skip-confirmation tests (:1081 and :1198) exercise the deliberate Delete
  contract.
- H-T5-3, H-T5-5 and H-T5-6, `session_list/lifecycle_tests.rs:409–415,731,1009`. The duration test and shared
  `stop_answered` helper discard the Stop path, as does the later Stop of n. Assert the exact path and scope “Stopping…”
  to the target row. Existing per-row assertions in callers help establish the UI side.
- H-T5-4, `session_list/lifecycle_tests.rs:828–836`. `replacing_open` checks that a is selected and discards the path.
  The cases at :884, :971, :1039, :1057, :1085, :1106, :1437, :1487 and :1517 inherit that blind spot. Check the source
  path in the helper. Keep the exact request assertions at :1131 and :1199 as a pattern.
- H-T6-3, `session_pane/tests.rs:996–1017,1113,1153,1176,1211,1245–1251`. The helper keeps request IDs only and the
  other-session test only counts one Delete. Preserve and assert a in the open-session helper and b in the other-session
  case. The scrim check partly guards against an open-session mix-up. The terminal shortcut test at :838–869 already
  matches a's exact route.
- H-T6-5, `session_pane/card_tests.rs:566–568,630–631,747–748`. These Replace cases discard the path and guard. Assert
  Replace for a with the expected guard before replying. Use the exact assertions at :498–506.
- H-T6-6, `session_pane/header_tests.rs:843–847`. The header-inert test counts one lifecycle request as its outstanding
  Stop. Require `/api/sessions/a/stop`.
- H-T8-1, `trash/cue/tests.rs:24–45`. The archive cue's helper hands back an ID for any Delete. Match b's request before
  feeding the archive reply.
- H-T8-3, `terminal/set/tests.rs:1535–1541`. The tab-outage case counts any Restart suffix. Require
  `/api/sessions/a/restart`, as the preceding case checks the whole Stop route.

### Hosts, settings and approval disclosure

- H-T1-5, `approvals/card_tests.rs:362–396`. The card test checks notes and answers. Add frame assertions for the named
  host and SSH destination. The model-only companion `approvals/model_tests.rs:1035–1087` already checks both facts, so
  this is coverage of the drawn disclosure.
- H-T2-1, `hosts/environment_tests.rs:345–362,369–392`. The container case checks the question and the shared keyboard
  case chooses Cancel. Add a confirming case that asserts `DELETE /api/hosts/5`, following the exact sandbox assertion
  in :278–314.
- H-T2-2, `hosts/menu/tests.rs:936–939,953`. The avatar case checks host 3's plan read and cancels the question. Extend
  the avatar scenario independently to confirm and assert host 3 and its plan ID.
  `hosts/uninstall/tests.rs:126–142,217–258` already checks host 2 and the plan ID from the Hosts panel.
- H-T6-1, `settings/credential_tests.rs:713–714`. The delayed-back case checks Delete by method. Match
  `DELETE /api/credentials/free` as the premise for opening a second editor.
- H-T6-2, `settings/recipe_leave_tests.rs:305,330,428–429`. The refusal, carries-on and close-during-Delete cases accept
  one command without verifying the recipe; the last also checks its method. Assert Delete of `rust-dev` before sending
  an outcome or closing the editor.
- H-T6-4, `settings/tensorlake_sandbox_tests.rs:196–197`. The list-clearing test counts one command. Assert
  `PUT /api/tensorlake` with only `{"clear_api_key": true}`, as the key form contract is already pinned in
  `settings/credential_tests.rs:881–895`.

Existing examples also include Remove in `hosts/remove/tests.rs:278–300,627–637`, Retry-delete in
`hosts/environment_tests.rs:421–454`, Suspend in :885–927, and a successful Replace with in
`launcher/replace_with_tests.rs:108–122`. Stand-in keyboard and prompt fixtures exercise input, focus and repeat
behavior without a real request target. Put new cross-target assertions at the normal entry points.

Possible cover: `header-actions-follow-unseen-session.md` and `session-actions-use-unseen-liveness.md` concern
production action authority. Their fixes may add focused tests and overlap individual cases. They do not cover the full
test gap here. SPEC.md “Lifecycle operations” and “Keyboard focus” and SPEC_impl.md “Questions: the prompt” explain why
the request should bind to what was presented.
