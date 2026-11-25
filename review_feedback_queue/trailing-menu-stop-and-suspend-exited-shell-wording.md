# Row menu's Stop and suspend still offers to stop an exited shell

Reviewed commit: 380a9079d33f90a170371e26ef9c8ec84aff0eb4

## TLDR

In a disposable environment, when a session's shell has already exited but its terminal tabs are still open, choosing
Stop and suspend from the session's row menu still asks "Stop this session's shell and close its terminal tabs, then
suspend the whole container? ... The stopped shell and closed tabs do not continue.", and its Cancel says "keep the
shell, its terminal tabs and the container running". There is no shell left to stop or keep. The commit fixed exactly
this wording in the Stop confirmation's Stop and suspend answer, but the menu's own Stop and suspend goes through a
different question that was not changed, so the two entry points describe the same action differently.

## Details

Line numbers are at the tip, unchanged since 380a9079d3 for these lines. The commit makes `stop::question` compute
`running_program = (!row.info.status.has_ended()).then_some(program)` and pass it to `environment::stop_note` and
`environment::stop_and_suspend_answer` (`stop.rs:128-176`). The row menu's environment items (`menu.rs:1365-1387`) offer
`session-menu-stop-and-suspend` for any non-resumable environment session regardless of status, with tooltip
`suspend_action(Some(program))` and description "End this session's {program} and tabs, then suspend the whole
environment". Clicking it calls `environment::begin_shown(target, stop=true, ...)` (`menu.rs:1318`), which freezes
`Asked.stopped_program = stop.then(|| super::stop::program_word(&target.row))` (`environment.rs:457`) without checking
`status.has_ended()`. Because `stop::asks` returns true whenever the row lists tabs (`stop.rs:75-77`), an exited shell
with an open tab always gets the question from `environment::question` (`environment.rs:540-564`), whose texts come from
`ProviderUi::suspend_question` / `suspend_action` / `suspend_cancel` with `Some("shell")` (`environment_ui.rs:797-847`).
SPEC.md's sandbox section says "The row menu always offers Stop and suspend, so it remains reachable when ordinary
confirmations are skipped", so this is a primary entry point.

Fix: freeze `stopped_program` only when the program is still running, or keep a separate "stop requested" flag so the
question is still titled Stop and suspend while the sentence, answer help and Cancel describe closing tabs only
(`suspend_question`/`suspend_action`/`suspend_cancel` need a tab-only variant, since `None` means plain Suspend); apply
the same to the menu item's tooltip and description. A golden for the menu path on the exited-shell fixture the commit
added (`goldens/session_list.rs` `environment_tab_only_stop_confirmation_setup`) would catch it.

Suggested bucket: `other` (misleading confirmation wording about a resumable action; nothing extra is lost because the
shell has already exited and the rest is frozen, not killed). Confidence: confirmed from source, not observed in a
running app. Possible cover: the commit's resolved queue item (`exited-shell-suspend-confirmation-wording`) may have
been scoped to the Stop question only, in which case this is its unfixed sibling.

Provenance: trailing code review through `953603f1`, run `fee35ad2-3831-4a91-b555-fe8cb1f8f1ae`, reviewer
`g16-session-removal-env-deletion` record `f1`.
