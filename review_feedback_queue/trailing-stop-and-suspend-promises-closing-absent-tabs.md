# Stop and suspend still promises to close terminal tabs a running session does not have

Reviewed commit: 446755516cbe3ac46f8de7147aea0210a93ffb2e

## TLDR

For a sandbox or container session whose agent or shell is still running but which has no terminal tabs open, the row
menu's Stop and suspend item, its hover help, and the confirmation that appears when other activity on the environment
makes Suspend ask all say Farhelm will "close its terminal tabs" (and Cancel says it keeps "its terminal tabs" running).
There are no tabs. The commit set out to make this wording follow the row the user saw, and fixed the ended-program
cases, but the running-without-tabs case still describes tabs that do not exist. The guard sent is correct; only the
text overstates.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t12a-gui-panes-hosts` `f1` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Reviewer's suggested bucket kept. No coverage source covers it.

### Reviewer record

Location: `crates/gui/farhelm-gui/src/session_list/environment.rs:403`

At 446755516c, `stop_target` (crates/gui/farhelm-gui/src/session_list/environment.rs:403-409 at the tip, same at the
commit) maps `(has_ended = false, _)` to `StopTarget::Program(program)` regardless of `row.info.tabs.is_empty()`.
`ProviderUi::suspend_question`, `suspend_action` and `suspend_cancel` (crates/gui/farhelm-gui/src/environment_ui.rs, the
`Some(StopTarget::Program(program))` arms) then say "Stop this session's {program} and close its terminal tabs, then
suspend the whole {noun}? ... The stopped {program} and closed tabs do not continue." and "Cancel: keep the {program},
its terminal tabs and the {noun} running". The row menu's description
(crates/gui/farhelm-gui/src/session_list/menu.rs:1387) says "End this session's {program} and tabs, then suspend the
whole environment". The `StopTarget::Program` doc in environment_ui.rs also states "Stop ends it and closes its terminal
tabs" as if tabs always exist. The guard (`super::stop::guard`, crates/gui/farhelm-gui/src/session_list/stop.rs:66-71)
sends `refuse_if_tabs_open: true` for such a row, so the helm would refuse if tabs had appeared; the wording and the
guard disagree about what the user authorized. The ordinary Stop question already distinguishes this case
(`stop::consequence`, `(0, false) => "Stop the {program}?"`), so the environment variant is inconsistent with plain
Stop. Route: a Tensorlake or Podman session with a running agent and no tabs; open the row menu (hover Stop and suspend
shows the tab wording); click it while another session on the environment is active, so the activity read asks; the
question names tabs. Fix: split `StopTarget::Program` into running-with-tabs and running-without-tabs (or carry the tab
count), and word the no-tabs case as "Stop this session's {program}, then suspend the whole {noun}?" with a matching
Cancel and menu description; add a menu/question test for a running row with no tabs. The wording was pre-existing
before this commit for this case; the commit's own enum design kept it.

Suggested bucket: other, because the text overstates a harmless consequence (closing tabs that do not exist) and the
sent guard is correct; nothing is lost or wrongly authorized.

Confidence: confirmed (source-established: stop_target ignores tabs for a running row; the Program arms always mention
tabs).

proposed_drop: none (pre-existing wording for this case, but the commit under review claimed to tie this wording to the
shown row's tabs and left this branch).

possible_cover: none
