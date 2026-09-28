# Changelog

Notable user-facing changes in each stable release of Farhelm. Release candidates and dev builds are not listed; their changes appear under the stable release that follows them. Entries are written for someone running Farhelm, not for someone reading its source, so internal mechanics are left out unless they change what you have to do. cargo-dist copies each release's section into its GitHub release; `releasing/AGENTS.md` describes the format and how a section is written.

## v0.18.0 - 2026-09-27

### 💥 Breaking

- The web UI now only works at `http://127.0.0.1:<port>`. Opening `http://localhost:<port>` or `http://[::1]:<port>` redirects you there, and anything else that talks to the helm at those two addresses is refused, so update bookmarks and scripts that use them. On a machine shared with other users, this makes it slightly less likely that another user's program on the same port could end up serving the page your browser opens. (#1035)

### 🔧 Fixed

- Stopping or restarting Farhelm on a host (restarting its service, updating it, pressing Ctrl-C on a foreground `farhelm supervisor run`, or quitting the desktop app) no longer risks ending every session on that host. The risk comes from a tmux bug that can crash its whole server when a terminal connection closes mid-output; Farhelm already worked around it elsewhere and now does so on these stops too. Farhelm being killed outright (out of memory, `kill -9`) still carries that risk. (#1043)
- If your shell startup files run `systemctl --user import-environment` (or `dbus-update-activation-environment --systemd --all`), opening a terminal tab could hand that tab's details to systemd, and Farhelm's service on the host picked them up on its next restart. Deleting that session or closing that tab then stopped the service itself, which could end every session on the host. `farhelm helm setup` and host updates now write a service definition that prevents this. A remote host gets it the next time it is updated, and Farhelm started by hand is not covered. (#1032)
- A tmux that stops responding can no longer freeze every terminal on a host. Resizing a terminal, which also happens each time one opens, now gives up after a short wait and leaves that one terminal at its old size. (#1064)
- Sessions and terminal tabs in a folder whose name contains `#`, such as `C#Samples`, now start in that folder. tmux reads `#` in a folder name as the start of one of its own formatting codes, so before, the agent quietly started in your home folder while Farhelm showed the folder you picked, and a folder name containing `#(…)` ran the text inside as a command every time the session was created, restarted, or got a new tab. Farhelm now escapes the `#` for tmux. (#1040, #1041, #1042)
- When you delete the last session using a fresh GitHub checkout Farhelm created, Farhelm moves the checkout folder into `farhelm-archived-working-copies`, after checking that the folder is still the one it created. That check could be fooled: if you had removed the checkout yourself and cloned or created a new folder at the same path, the filesystem often gave the new folder the same internal id number, so deleting the old session moved your new folder into the archive. Farhelm now also compares the folder's creation time. Checkouts created before this version have no recorded creation time and are still checked the old way. (#1046)
- A fresh GitHub checkout could end up named `farhelm-archived-working-copies`, the same name as the folder Farhelm moves deleted checkouts into, for example from a title like "archived working copies" on a repository called `farhelm`. Deleting other sessions then moved their checkouts inside it, and its own session could never be deleted or restarted. That name is now refused. If you already have a checkout with that name, other deletes no longer move anything into it, and deleting its own session tells you to move or rename the folder by hand. (#1044, #1045)
- Updating a remote host could put Farhelm's own copy of tmux in the same folder as `farhelm`, such as `~/.local/bin` where the install script puts it. If you had your own `tmux` there, it was overwritten; if not, typing `tmux` on that host could start Farhelm's copy instead of the one you installed. Updates now keep Farhelm's tmux in `~/.local/lib/farhelm`, off your PATH. A copy an earlier update left behind is not removed: if there is a `tmux` in that folder you did not put there, delete it. (#1072)
- A remote host can no longer sign your browser out of the web UI. When a host turned down an action on one of its sessions with a "not authorized" error, the web UI took that as its own sign-in expiring and showed the token prompt, for every host. Such refusals now show as an ordinary error on that one action. A host also can no longer make the session launcher throw away a launch you were setting up by wording its error a particular way. After updating, reload any browser tab that was open during the update. (#1036)
- The session launcher's remembered choices (permission mode, workspace trust, and recent setups) now come only from launches you made yourself in it. Before, a remote host's reply to a new session could make "yolo" with workspace trust the preselected default for every host, sessions started by agents showed up among your recent setups, and pressing Replace on a session recorded that session's settings or profile as if you had chosen them. (#1053, #1055, #1060, #1063)
- Starting a session whose command begins with the `{conversation}` placeholder is now refused. Such a session stopped Farhelm on its host from starting after the next restart, upgrade, or reboot, until the session was removed from Farhelm's database by hand. (#1065)
- Running the install script twice in quick succession can no longer delete, or quietly undo, the `farhelm` that was just installed. (#1075)
- Stopping or deleting a session no longer sometimes leaves one of its processes frozen instead of ended, still holding on to things like a dev server's port. This could happen to a process that had just started a program such as `sudo`. (#1068)
- Deleting a session on a Linux host no longer sometimes leaves a background process running, from an earlier run of that session or from one of its closed terminal tabs. (#1066)
- On Linux hosts, interrupting a restart at the wrong moment, for example by closing the browser, can no longer leave Farhelm on that host unable to stop, delete, restart, or open terminals for any session until Farhelm itself is restarted. (#1067)
- Adding a remote host where you already installed Farhelm with the install script now finds that installation. Before, the helm did not look in `~/.local/bin`, concluded Farhelm was missing, and offered to install a second copy. An installation in a custom `FARHELM_INSTALL_DIR` is still not found this way. (#1073)
- If `XDG_STATE_HOME` is set to a relative path, Farhelm now ignores it and uses `~/.local/state/farhelm`. Before, the helm service and commands you ran by hand, such as `farhelm helm token show`, could end up reading different folders, so `token show` could print a token the running helm did not accept. If you set a relative `XDG_STATE_HOME` on purpose, use `--state-dir` instead. (#1113)
- `farhelm helm token rotate` now refuses when it finds no helm in the folder it is pointed at, and says which folder it checked. Before, a mistyped `--state-dir`, or a shell with a different `XDG_STATE_HOME` than your helm, created a new, empty helm there and printed a new token, while your real helm kept the old token and every browser stayed signed in. If you rotated a token that way to lock someone out, rotate it again against the right folder. (#1034)
- `farhelm helm token rotate` now waits up to 15 seconds for the running helm to answer, instead of 2. Before, a rotation that was merely slow could be reported as a timeout even though it went through and signed every browser out, without showing you the new token. If it still times out, the error tells you to run `farhelm helm token show`. (#1098)
- The helm, including the one built into the desktop app, no longer fails to start with "another process owns token control" when `farhelm helm token show` or `farhelm helm token rotate` runs at the same moment, as it easily can right after `farhelm helm setup`. (#1070)
- Starting a second helm on the same state folder as one that is already running, for example the desktop app next to the helm service, is now refused straight away. Before, the second helm first upgraded the shared database and connected to every host before giving up, and if it was a newer version, the older running helm could then fail and be unable to start again. (#1069)
- A terminal you close and reopen right away is no longer sometimes closed again immediately with "session terminal ended", "output stream failed", or a stalled message. (#1056)
- Opening the same session from two browser tabs or windows at the same moment can no longer leave one of them waiting forever when tmux stops responding; the slower one now shows an error and you can try again. (#1054)
- Typing in one of a session's extra terminal tabs no longer affects which agent conversation Farhelm connects to the session. Before, using a tab before your first message to the agent could make restart stop offering to resume the conversation, or pick up a conversation you ran in the tab instead. This affected agents whose conversation Farhelm finds by looking on disk, such as Claude. (#1057)
- With many sessions, the web and desktop UI no longer jumps away from the session you just opened to the one you had open on your last visit. (#1078)
- The Restart and Replace confirmations at the top of a session now say what will happen to the agent. Replace warns that a running agent is killed and its conversation thrown away, and neither claims the agent is "still running" when Farhelm does not know. (#1079, #1080)
- After you replace a session on a remote host, the session launcher again defaults to that host and the session's folder, instead of the local machine with an empty folder. (#1124)
- Renaming the open session from the session list now updates the name at the top of the session straight away, and in the Clone and Replace with forms it fills in. (#1121)
- A session you delete no longer briefly reappears in the list, with a working Delete button, if the list was refreshing at that moment. (#1123)
- Error messages in the session view, "failed to load sessions", and the folder and host tooltips in the session list now show hidden and text-direction characters as visible codes, like the rest of the interface, so such characters in a folder name, host name, or error cannot hide or reorder part of the text. (#1126)
- Clicking a copy button at the top of a session twice in quick succession no longer cuts the second "copied" confirmation short. (#1122)
- Marking a session as read from its row menu now always shows an error on the row if it fails, and clears an earlier error when it works. (#1125)
- In the browser, entering the access token again while a session action was still running no longer leaves the Restart, Replace, and "+ terminal" buttons refusing to work until you reload the page. (#1120)
- In the desktop app, the first thing you do after rotating the helm's token no longer fails with "authentication is required". (#1077)
- The desktop app no longer sits on "Starting Farhelm…" forever when its built-in helm starts but never answers; after 5 seconds it shows an error. (#1118)
- When the desktop app fails to start, it now shows the actual reason instead of only "embedded helm stopped unexpectedly". On macOS this also avoids a second, misleading alert. (#1119)
- `farhelm helm setup` and the desktop app no longer hang at startup when your `tmux` is, for example, a wrapper script that leaves a program running in the background; they now report that it did not answer like tmux. (#1091)
- If an earlier `farhelm helm setup` failed before restarting Farhelm's services, the next run restarts them. When it could not read its note about that pending restart, it used to skip the restart and report success while the services kept running the old setup; it now stops with an error. (#1114)
- `farhelm agent` commands no longer sometimes wait 30 seconds and time out, or report that the outcome is unknown, after the helm has reconnected to a host. (#1062)
- A `farhelm agent` command that changes something (stop, restart, rename, create, or clone) no longer waits up to ten minutes when an earlier such command from the same session has not been answered yet. It now gives up after about half a minute and says it is safe to try again later. Before, the waiting command could still take effect minutes after the agent had given up on it. (#1061)
- `farhelm agent restart` no longer kills the agent of a session whose start was never confirmed (shown as unknown, so its agent may still be running) unless you pass `--stop-if-running`. Restarting a session that was interrupted by a reboot or has exited still needs nothing extra. (#1030)
- The `agent` column of `farhelm agent sessions` no longer shows text copied from a session's command line. A session started as `API_KEY=… claude` showed the key itself there, to every agent on every host. The column now shows the profile's name, the agent Farhelm recognized, or `custom`. Until a remote host is updated, its sessions that were not started from a profile show `custom`. (#1038)
- When an agent's request to start another session is turned down, the agent's own session is no longer left stuck, unable to be stopped or deleted and blocking new sessions on that host, if the agent stopped listening for the answer. (#1058)
- A session started with `farhelm spawn --inherit-agent` that re-runs its parent's spawn command with the same `--idempotency-key` no longer gets itself back as the "new" session, which made stopping or restarting "its child" hit itself. That spawn is now refused and nothing is created. (#1059)
- When `farhelm spawn --agent` or `--profile-id` fails because no helm is connected to the session, the error now tells you to use `--inherit-agent`, instead of suggesting to leave out `--agent`, which the command rejects. (#1082)
- `farhelm agent clone --cwd ""` is now refused straight away with a hint to leave `--cwd` out, instead of failing later with a confusing error on the destination host. Clone's errors about the source session now name its real flag, `--source-session`. (#1083, #1084)
- If a host answers a new session request with a session id that another host already uses, the helm now reports an error naming that host, instead of success. Before, opening, typing into, or stopping that "new" session could act on the other machine's session. Only a faulty or compromised host answers this way. (#1052)
- The helm now rejects a host's reply to a rename or restart that is about a different session than the one you acted on, instead of showing that other session's details. Retrying a session in a fresh GitHub checkout now also rejects a malformed session id from the host, which used to give you a session the interface could not open. (#1128, #1129)
- When adding a host fails, the helm now also drops any connection it had already opened to it. Before, a host the helm said it had not added could still show up in the host list, possibly connected, and could not be removed. (#1103)
- A remote host removed while it was connected could, in some cases, stay in the host list as connected. It now disconnects and leaves the list at the next refresh. (#1104)
- When the helm's connection to a host crashes and is restarted (on Retry or after you change the host), the old connection is now always closed. Before, it could keep answering `farhelm agent` commands for that host for a short while. (#1105)
- Renaming a host, or editing or retrying one while its sessions are refreshing, now shows up in your other open windows straight away instead of after a later refresh. (#1106, #1107)
- On a host that does not report an identity to the helm, a session you create right after the host connects now works straight away instead of answering "no such session" for a while, and a session id that another host also has is listed once rather than twice. (#1100, #1102)
- Adding a remote host with an empty or invalid remote state folder is now refused. Such a host used to be added and then never connect, and the only way out was to remove it and add it again. (#1071)
- Updating a remote host where the `farhelm` program or its service file is a symlink no longer changes the permissions of the file it points to on every update, or fails when that file belongs to another user. (#1074)
- If the connection to a host drops at the start of installing or updating Farhelm there, the large file already uploaded is now removed instead of left behind on the host. (#1111)
- Updating a host that was added with a relative path to `farhelm` now fails with a message saying to use a full path, instead of an error that looked like a connection problem. (#1109)
- Creating a fresh GitHub checkout without a title no longer keeps suggesting, and then refusing, the name of a checkout folder that was removed from disk but still belongs to a session in your list. (#1130)
- If tmux fails partway through starting a new session, the session now opens normally. Before, its agent could be running while the session had no terminal to open until Farhelm on that host was restarted. (#1094)
- If starting a session fails and Farhelm also cannot clean up afterwards, the session now shows in the list with an unknown status so you can delete it. Before, it was invisible, and stopping or deleting it said "not found" until Farhelm on that host was restarted. (#1092)
- In the rare case where a restart fails and Farhelm cannot restore the session's earlier status, the session now shows as unknown straight away, instead of showing its old status and changing to unknown later on its own. (#1093)
- If Farhelm cannot work out a new session's folder at the moment it creates it, for example because part of the path is being changed, the create now fails and you can try again. Before, the session was created, but restarting it could be refused for as long as it existed. (#1095)
- Two Codex, Goose, Pi, OMP, or Grok sessions started in the same folder within about a minute of each other no longer log a warning that their conversations cannot be captured, and are no longer marked that way. That rule is for agents whose conversation Farhelm finds by looking on disk, such as Claude; these agents tell Farhelm their conversation directly. (#1089)
- When Goose does not tell Farhelm its session id, the hook log now says so instead of staying empty, which helps figure out why a Goose session cannot be resumed. (#1115)
- If Farhelm crashed on a host while starting or restarting a session, the temporary files it left behind, which can contain the session's command line and access token, are now removed the next time Farhelm starts there instead of staying until the session is deleted. (#1127)
- The helm now only accepts browser requests from its own `http://` page. When it ran on port 80, it also accepted requests from a page served by a different local web server at `https://127.0.0.1`, although those requests still had to pass the helm's sign-in check. (#1099)
- The install script no longer refuses forever with "another farhelm install/update is already running" when an earlier, killed run left its lock behind and the new run happens to get the same process id, which is common in containers. (#1116)
- The install script now explains why it stopped when it cannot read the downloaded archive's contents, instead of stopping without a message. (#1117)
- Downloading a release now follows up to five redirects, as documented, instead of failing with "too many redirects" at the fifth. (#1112)
- On a Mac with two Farhelm installations in different folders, `farhelm uninstall` run for the one that does not own `~/Applications/Farhelm.app` now tells you which installation owns the app and to uninstall that one first, or move the app aside. It used to suggest rerunning the installer, which would have taken the app away from the other installation. (#1076)

## v0.17.0 - 2026-09-27

### 💥 Breaking

- This release *requires* you to update your remote hosts. (#970, #977)

### 🚀 Added

- `restart with` in a session's header restarts the session with a different model, effort, permissions, or workspace trust, and continues the same conversation. The session's harness, host, and folder cannot be changed this way. (#977, #978)
- The session launcher's search now takes `yolo`, `perms:yolo`, and `perms:default` to set the launch permissions. (#964)
- The Farhelm wordmark now appears at the top left of the window, above the session list. (#963)

### 🔄 Changed

- Host updates now show their current step and elapsed time in the host row, so you can follow an update without opening the row. A failed or uncertain update still opens the row with its details. (#975)
- The session `⋯` menu now opens beside the session list, with the session's name and details at the top and a short explanation for each action that creates, replaces, stops, or deletes a session. (#974)
- The profiles button now sits beside the new session button at the top of the session list, in the same compact shape. (#962)
- "+ terminal" and the session launcher's "reset choices" now look like buttons, and the session's action buttons (restart, replace, clone, replace with) are lowercase like the rest of Farhelm's controls. (#957, #958, #959)

### 🔧 Fixed

- Cloning a session that was started from a profile could pick the profile you last launched with instead of the session's own, if you chose "other / command" before the launch dialog had finished loading your profiles. The dialog now picks nothing in that case and asks you to choose. (#1023)
- While renaming a session or using the session launcher, a terminal tab closing in the background could move keyboard focus to the agent's terminal, so the rest of what you typed went to the agent. Focus now stays in the dialog. (#1014)
- The folder and command shown at the top of an open session now use the available width before being cut off. Previously they were cut off after a handful of characters even in a wide window. (#961)
- A profile's resume command is now split into arguments the same way as its invocation. Previously a backslash inside double quotes was dropped only in the resume command, so `--append-system-prompt "match \d+"` restarted the session with `match d+`. Saved profiles are unaffected until you edit their resume command. (#995)
- Sessions now start when the path to Farhelm or its state folder contains characters such as `{}` or `!`. Previously the shell that starts the session could mangle the path. (#971)
- A session whose tmux session you renamed, or whose pane you moved, by hand was recorded as exited even though its agent was still running. It now shows as unknown. A permission problem on the tmux socket could, in rare cases, also make a live session look like it had lost its terminal; that is fixed too. (#968, #996)
- The session launcher now refuses `{codex:trusted-cwd}` and `{codex:untrusted-cwd}` as a custom model id, as it already did for `{cwd}` and `{conversation}`. Previously they were accepted, and the launch then failed. (#997)
- The session launcher now spells OMP as "OMP" everywhere, instead of "Omp" in some places. (#1000)
- If the host's supervisor went away while `farhelm spawn` was starting a session, spawn now says the outcome is unknown and to check before retrying, since the session may already be running. (#985)
- The instructions Farhelm gives agents now explain how the restart options listed by `farhelm agent sessions --json` map to `farhelm agent restart --mode`. Agents following the old instructions got a usage error instead of a restart. (#998)
- `farhelm agent` tables now show invisible characters in titles, paths, and errors as visible escapes such as `\u{200b}`, so two titles that look identical can be told apart. (#966)
- Several error messages are clearer: an unexpected reply from a supervisor no longer dumps the whole reply into the error, a host identity mismatch no longer names the two identities the wrong way round, and refusing an unknown remembered permission lists all four accepted ones. (#972, #1005, #1011)

## v0.16.0 - 2026-09-25

### 🔧 Fixed

- Restarting a session whose agent had died could kill all of the session's extra terminal tabs and the shells running in them, while the session went on listing those tabs as open. The tabs and their shells now survive the restart. (#919)
- Changing a host's destination while Farhelm was connecting to it could briefly keep Farhelm talking to the old machine, so something you did right after the change could land there instead. Changing the destination of a host marked as a duplicate now connects to the new destination right away, instead of staying stuck until you changed it a second time. (#920, #924, #954)
- Deleting a session behaves better when the delete does not go through. A delete that hit an internal error no longer blocks every later attempt to delete that session until the supervisor restarts. If a delete fails at its very last step, the session's attached files are put back, instead of becoming unreachable and then being erased at the next supervisor restart while the session itself stays. And a delete that is turned down because the session is not yet safe to delete no longer destroys an attachment that is still uploading. (#911, #914, #916)
- Stopping, restarting, or deleting a session could in rare cases stop an unrelated program: if the session's process had already exited and the system had since given its process ID to something else. Farhelm now checks that it is still the same process before stopping it. (#915)
- Farhelm no longer mistakes a session's agent window for one of its extra terminal tabs. A program running in the session could relabel the agent's tmux window so that it showed up as a tab; closing that tab, or Farhelm removing it on its own, then stopped the agent along with it. Farhelm now recognizes the agent window from its own record of the session instead of from that label. (#921)
- If starting a session was interrupted partway (for example by a supervisor crash) and the retry then failed a safety check, such as the session's folder having changed in the meantime, Farhelm now deletes the files left by the interrupted attempt right away. They can contain credentials and secrets passed on the command line, and previously stayed on disk until the supervisor restarted. (#918, #953)
- If Farhelm could not tell whether a restart had actually started the agent, the session was shown as exited, with the exit code of the run before the restart, and could flip between exited and unknown every time the supervisor restarted. It is now shown as unknown. (#917)
- Using Browse on a saved folder in the session launcher could erase the name you had most recently used for that folder, so searching for it by that name stopped finding it. In some cases it also left duplicate entries in the saved folders and logged a warning on every visit. Both are fixed. (#889, #890)
- For generic sessions, a restart command containing a conversation placeholder was accepted even though it could never work, so every restart quietly started over with a fresh conversation. Farhelm now refuses to create the session with such a command, and suggests removing the placeholder or picking a supported harness. (#895)
- If a host's user database answered Farhelm's login-shell lookup with malformed output, starting a session there failed with a misleading error. Farhelm now ignores the malformed answer and finds the shell another way. This has not been seen to happen in practice. (#896)
- An interrupted host install or update left full-size copies of the Farhelm binary hidden on the host, where nothing ever removed them, so repeated failures over a flaky connection could slowly fill its disk. The next install now removes those leftovers, and nothing else. The same goes for half-unpacked files the helm leaves in its own cache if it crashes while provisioning with `--payload-dir`. (#905, #909)
- Closing a terminal tab while the connection to its host was stuck could tie up the helm for about a minute. Cleanup now stops waiting after five seconds. (#900)
- An attachment upload that stalled in one particular way could, in principle, keep a CPU core on the helm busy indefinitely. It is now aborted like any other stalled upload. It is not known whether this can actually happen. (#898)
- When a host kept failing to connect or refresh with the same error, the helm's log output stopped mentioning it after the first few times. Those errors are now logged every time they repeat. (#887)

## v0.15.0 - 2026-09-24

### 🔄 Changed

- The session header keeps Restart, Replace, Clone, and Replace with visible together in one row. The session's directory and command line are shown in the header too; long values are shortened, hovering shows the full text, and clicking copies it. Confirmations for these actions are clearer. (#928, #932)
- When a host restart interrupts a session, the session now shows a centered card explaining what happened, with Restart and Replace buttons, instead of the previous notice. (#927)
- Buttons now look the same wherever they appear according to what they do: blue for the main action, gray for secondary actions, and red for confirming something that cannot be undone. (#926)

### 🔧 Fixed

- Maybe: On macOS, typing in a terminal could insert words you never typed, such as `SPECIALLY` after typing `SPE` in Codex, apparently because macOS inline predictive text completed words in the terminal's input. Farhelm now turns inline predictions off for terminal input. The corruption has always been inconsistent in how easily it can be reproduced, so this is a *somewhat likely* fix but not confirmed. Will see after more use. (#912)

## v0.14.0 - 2026-09-23

### 🚀 Added

- Grok is now a supported harness. Launch it from the session launcher, and Farhelm tracks its conversation so a restarted session can resume it. There is no model or effort picker for Grok. (#845, #861)
- The session launcher can set "workspace trust", avoiding the "do you want to trust this project?" prompts, for Codex, Muse, and Pi sessions with `trust:true` or `trust:false`. The choice applies to that launch only, is remembered as the default for the next new session, and never changes the harness's own trust settings. Claude, Goose, OMP, and Cursor have no such switch. (#863, #869)
- The session launcher's search now takes `name:foo` to set the session name and `host:foo` to pick a host; `host:local` picks this machine. (#864)
- The Hosts header has an `update all` action for remote hosts whose individual Update action is available. Each host shows its own progress and result, and a failure on one does not stop the others. Hosts already busy with setup or another update are skipped, and the local host is never included. (#894)
- The Hosts panel shows the binary upload as its own progress step during setup and updates, so you can tell when Farhelm is still transferring over the network. (#891)

### 🔄 Changed

- After a host reboot interrupts a session, Farhelm now explains what happened and waits for your choice: restart the session with its existing conversation, or replace it with a fresh conversation using the same settings. (#897)
- The desktop app remembers its window size and position, and reopens maximized if it was maximized when closed. If the display layout changed, it picks a safe placement instead of reopening off-screen. (#899)
- OMP session tracking is more reliable: Farhelm only accepts a conversation reported by the session's own interactive OMP, not by subagents or nested OMP processes. OMP sessions started before this version offer only a fresh restart until they are relaunched. (#814)

### 🔧 Fixed

- Selecting a GitHub repository in the session launcher now shows the folder the checkout will be created in, instead of a stale path from earlier. `use existing folder` switches back to editing the path yourself. (#865)
- OpenCode, Goose, Pi, and OMP sessions can be launched without picking a model; the harness then uses its own configured default. (#866)
- After a supervisor restart, sessions keep their last known status instead of briefly showing as running. A session with no known status asks for confirmation before Restart. (#867)
- The destination picker and Browse button in the session launcher line up again. Hovering a session's status dot shows its status, and the unlock icon on YOLO sessions explains the permission mode. (#862)
- Remote host updates no longer abort an upload after 60 seconds while it is still making progress. A stalled upload still times out and leaves the installed binary intact. (#888)
- Host rows show compatible supervisors running an older Farhelm build as `old version` instead of `needs update`, and they stay connected and usable. Hosts whose supervisor is actually incompatible still show `needs update`. (#893)

## v0.13.0 - 2026-09-22

### ✨ Highlights

#### Session archiving is removed

The concept of session archival is removed. No use for it. Might add back in the future if useful.

On upgrade, any archived sessions return to the session list. (#834)

#### More reliable session id tracking for codex

This version should more reliably be able to resume codex sessions. The bug was that sub agents and shelled out sub harnesses were able to report a session id back to farhelm as the currently active session. This should now be prevented.

Other harnesses have similar problems which is planned to be addressed in future releases. (#811)

### 💥 Breaking

- As stated above, session archival is removed. (#834)

### 🚀 Added

- The cursor CLI agent is now a partially supported harness. There is no session tracking yet, so restarts won't resume sessions properly. (#844)
- Better icons for the various harness types. Official ones where allowed by terms of use; otherwise something reasonable. (#847)
- Going forward, releases will have curated release notes, such as the ones you are reading now. (#858)

### 🔄 Changed

- More reliable codex session tracking as mentioned above. (#811)
- Replace gpt-5.6 luna+sol models with their gpt-6-* counterparts. (#859)

### 🔧 Fixed

- The examples in `$farhelm help` are now clearer and no longer include unrelated restart caveats. (#828)
- Rare edge case could cause a helm to inappropriately claim a browser is still attached when attempting to re-attach. (#843)
- The icons indicating harness type in the session list, and the permission lock icon for yolo sessions, are now properly displayed in two fixed columns. Previously they would not be aligned when some sessions were in yolo and others were not. (#850)

## v0.12.0 - 2026-09-21

### ✨ Highlights

#### The session launcher looks like the rest of Farhelm

Previously the session launcher had a considerably different look and feel from the rest of Farhelm. It now uses the same colors, corners, and selection style as the main window, and its "recent-setup" rows, which show recent launches, only show what those launches explicitly override relative to default values. (#819, #820)

### 🔄 Changed

- The session launcher uses the main window's colors, corners, and selection style, and its "recent-setup" rows, that show recent launches, only show what those launches explicitly override relative to default values. (#819, #820)

### 🔧 Fixed

- Buttons, inputs, and selects in the session launcher render in Farhelm's typeface instead of the platform's sans-serif. (#818)
