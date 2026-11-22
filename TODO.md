# TODO

A running list of things the maintainer wants fixed or built. This is intent, not history: an entry is REMOVED in the
same PR that addresses it, so the file only ever describes what is still wanted. It is not a roadmap and carries no
priorities unless an entry says so itself.

Twelve buckets, assigned by the maintainer: "definite simplification" is complexity the maintainer has decided to remove
— the decision is made, only the work remains; "planned" holds accepted work to implement later and suppresses duplicate
review triage within each item's stated scope; "plan decisions to revisit" holds the calls made while a plan ran or
landed, often on the maintainer's behalf when a plan blocked, each recording the alternative; "near term" is what should
be picked up next; "mid term" is wanted after the near-term work, sooner than "maybe later"; "doc todo" holds
documentation work; "tricky bugs" retains unresolved bug reports and their investigation findings; "deflake" gathers
test and harness reliability work, including CI execution and restoring gates; "broken tests" records tests that fail
deterministically, with the failure and the evidence that it predates any in-flight work; "code cleanup" holds
structural code smells from a whole-codebase assessment, grouped by kind with a suggested order; "maybe later" is wanted
but not soon, and may never happen; "unbucketized" is everything not yet sorted, which carries no implication either
way. Within a bucket, no order unless the bucket explicitly says so.

Near term's "Plan follow-up" subsection holds the entries that came out of plans (follow-ups asked for in review,
leftovers from reports and landing notes), kept apart from the rest of the near-term work.

Known product fixes stay in their product bucket. "Difficult deflake" retains unresolved failures and their
investigation evidence within "Deflake"; that placement does not establish that the cause is test-only. Move a diagnosed
product fix out of "Deflake" rather than changing user-visible behavior as a test correction.

## Definite simplification

## Planned

## Plan decisions to revisit

- **Revisit deferring the Codex credential migration.** On 2026-11-22 the landing monitor answered
  `recipe-credentials-in-launch-env`'s second block on the maintainer's behalf: the plan moves Claude's recipe
  credentials into the launch environment now and leaves Codex's delivery as it is, settings writes included, with the
  Codex migration recorded as a Plan follow-up. The executor found that Codex 0.171.0 launch overrides cannot keep a
  leftover settings bearer away from a tokenless endpoint: leaving the token out keeps the stale bearer, and an empty
  one sends an empty Authorization header. The alternative was to widen the plan to find and verify a way to isolate
  Codex's launch configuration that keeps saved conversations, regular settings and tokenless endpoints working.

- **Revisit skipping Claude trust in the home directory and root.** On 2026-11-22 the landing monitor, landing
  `agent-trust-every-folder` (fix #4583), narrowed the maintainer's "every folder" decision for Claude only: with
  Claude's trust switch on, a launch in the home directory or the filesystem root no longer marks it trusted in Claude's
  settings file, prints a note, and leaves Claude to ask. Claude treats a trusted directory as covering every directory
  inside it, so trusting home would trust everything under it, outside Farhelm and after the switch is off, with no
  removal record. The alternative was to apply the edit in every folder as decided, with a caveat in Settings and the
  release note.

## Near term

- **One slow host holds up every other action for up to 20 seconds.** The desktop app sends its commands to the helm one
  at a time, each starting when the previous one has answered. Saving a replacement credential (delivered to every host,
  at most 20 seconds per host) and deleting from Trash (at most 20 seconds per host) can therefore keep every later
  click that needs the helm (a launch, a Stop, another save) waiting for up to 20 seconds when one host is slow or
  unreachable. The window itself stays responsive, but actions look stuck. Check whether the per-host waits run in
  parallel (if they run one after another, the worst case is 20 seconds times the number of hosts), and improve this,
  for example by moving these routes to the accepted form, as provisioning already is. Plan:
  `plans/queue/slow-host-async-actions.md`.

- **Idle CPU of the GPUI desktop app.** Accepted for now on 2026-11-16: an idle window with one running session uses
  about 3.05-3.09% CPU on the test Mac (Apple silicon), against 1.61-1.62% for the last browser-UI build in the same
  session. Frames are back to the old app's rate (3.0 changed frames a second, after the cursor blink was aligned with
  the running pulse); the whole gap is cost per frame, about 4.4 ms against 1.8 ms, spread over the session list (+0.86
  ms), the session header and tabs (+0.76 ms), whole-tree layout (+0.74 ms) and the shared buttons. Most of it is GPUI's
  own per-element work. Levers looked at and not taken: lighter Button and IconButton (about 1% of a frame), repainting
  only the pulsing dot (a vendored GPUI patch; helps only pulse frames), and caching the session list and header behind
  the cache wrapper, the one large lever, which drops a cached view's screen-reader information unless GPUI is patched
  to replay it. Look into caching with that patch, or another way to cut per-element cost.

- **Decide whether the GPUI screenshot goldens belong in the release process, and how.** The goal is that releases run
  the GUI's visual checks. `scripts/check-gui-goldens.py` compares every GUI component and dialog, pixel for pixel, with
  a reference picture. Today it runs only during development and landings: its reference pictures live outside this
  repository, in the private storage repository `scripts/gui-goldens.py` fetches from (moved there on 2026-11-16 to keep
  main's history small), and the release runners cannot read it. Decide first whether exact pixel equality belongs in a
  release at all. The pictures come from a software rasterizer, so a GPUI or GPUI Kit bump that touches rendering shifts
  all of them at once, and a runner with a different Mesa or font version can too. So far they have mostly confirmed
  intended changes; their one clear catch was two changes that broke the Trash dialog together (#3662). The options
  include exact comparison, a tolerance-based comparison, or keeping them a development-only check. Only then pick where
  the pictures live so a release can reach them (a read token for the release runners, a public store, another home, or
  back in the repository), and add them to the release workflow next to the end-to-end GUI smoke.

- **Tunnels from remote sessions on Linux.** The desktop app opens Cmd/Ctrl-click tunnels (and approval cards' Tunnel
  and open) only in the Mac app; on Linux the pill, the consent switch and the card's Tunnel and open are not offered.
  Port them to the Linux desktop app: the tunnel consent switch in Settings → Terminal, Ctrl-click to tunnel a loopback
  link, the tunnel pill, and the approval card's Tunnel and open, with the same consent and refusal rules as the Mac.

- **Recognize Muse Code questions and approvals.** Detect questions and approval dialogs during a conversation from real
  Muse screens. These can currently look idle while Muse waits for an answer. Capturing them needs an authenticated test
  provider or a documented provider fixture protocol that can ask a real question. Plan:
  `plans/queue/muse-session-tracking.md`.

- **Custom agents, starting with conversation tracking.** Let users run an agent harness Farhelm has no built-in support
  for (an internal or unreleased tool, say) and still get conversation tracking, so Restart resumes the conversation.
  Farhelm cannot know such a tool's details, so the user brings the integration and Farhelm defines the contract. A
  design sketched on 2026-10-30:
  - The contract is a public, stable farhelm command the user's hook calls (for example
    `farhelm report-conversation --id <ID>`, or reading one JSON document on stdin and picking the id out with a JSON
    pointer), not the report files the built-in hooks write today. Those carry process ancestry, a format version and a
    temp-file-then-rename protocol; making them public would freeze internals and make every user script reimplement the
    ancestry walk. The existing user-installed hook integrations that call `farhelm internal hook` already show the
    shape.
  - Farhelm promises: a valid report becomes the session's resume target, the latest report wins, and repeating an id is
    harmless; a report made while the supervisor is down is recorded as reports are today; the reporter always exits 0,
    prints nothing and bounds its stdin read, sending problems to the session's hook log so a broken hook never shows in
    the agent's interface; outside a Farhelm session it is a logged no-op, so the same hook can stay configured when the
    tool runs by hand.
  - The user promises: the reporter runs as a descendant of the session's agent process (so it inherits the session's
    environment), whether through per-launch arguments or the tool's own configuration; it reports the conversation the
    session's top-level agent is in now, not a subagent's; the id is an opaque string of bounded length with no control
    characters, substituted into the resume command as one argument.
  - The supervisor checks only what it can without knowing the tool: the report came from a process in the session's
    pane, from the current launch, and the id is well formed. It cannot stop a nested agent from overwriting the report
    or check that the id actually resumes; a bad id shows up as a failed resume command in the terminal, and the docs
    should say so plainly.
  - Wiring: command launches and templates already store a command and a resume command containing `{conversation}`. The
    minimal addition is a "custom" agent type a command launch can declare, with generic screen-based status,
    `{farhelm_args}` filled with nothing, conversation captured only through the reporter, Resume allowed with the
    user's resume command, and no "agent never reported" warning, since Farhelm cannot know when a custom agent should
    report. A named custom-agent catalog in the launcher can come later on the same contract.
  - Maintainer decisions first: this contradicts SPEC.md's statements that integrations are not user-authored and that
    Farhelm identifies an agent's conversation only from the harness's own explicit report. Also decide whether Restart
    keeps the old id until a new report arrives or clears it, whether the reporter gets a mode to withdraw the target
    ("no resumable conversation yet"), and whether to offer optional nesting protection (only admit a report when a
    process with a named executable sits between the pane and the reporter).

- **Run the desktop app's own supervisor as the same launchd job remote Macs get.** Asked for by the maintainer on
  2026-11-08 while planning remote macOS hosts: once that plan has landed and remote Macs are validated as stable, move
  the local Mac's supervisor from being started detached by the desktop app to the same per-user launchd job, so every
  Mac uses one consistent mechanism. Related: "Let the Mac's supervisor outlive the desktop app" under Maybe later.

- **Decide how terminal snapshots are supposed to work.** Every checkpoint of a terminal in a terminal host encodes the
  whole terminal (screen, up to 12,000 lines of scrollback, modes) into one self-contained blob, even though most of the
  scrollback has not changed since the previous snapshot. That repeated work is what makes typing pause during a
  checkpoint and what makes the burst at an orderly supervisor stop expensive. The plans `faster-snapshot-encoder` and
  `stop-checkpoint-burst` only make today's whole-terminal encode cheaper and rarer. The maintainer takes a position
  first, in particular on whether snapshots should be maintained incrementally, and implementation follows. Two shapes
  came up while planning those plans: incremental on the host (a base snapshot plus a chain of changes, which needs a
  new snapshot format and a protocol change for hosts that outlive every release), or incremental in the supervisor
  (cache the encoded bytes of history rows and re-encode only the screen and new rows, producing the same v1 blob as
  today). The repository records no reason for the original choice of whole snapshots.

- **Credentials for ordinary SSH hosts.** The Credentials section of Settings serves environments through their recipes.
  Extend it so ordinary SSH hosts can use the same named credentials.

- **Cut the new desktop app's idle CPU with a large running fleet.** With the session list now drawing only the rows on
  screen, idle CPU no longer depends on how many sessions are listed, and typing stays at about 30 ms at any fleet size.
  But it still grows with how many sessions are running: on the test Mac, 1.6% with 5 sessions, 3.3-3.5% with 179 of 200
  running and 5.3-5.4% with 449 of 500 running, against the old app's 1.6% idle. Most of it remains when the window is
  in the background, so it comes from applying the helm's updates for running sessions, not from drawing. Asked for on
  2026-11-14; first measure the old app with 200 running sessions for a fair comparison, then profile what each update
  costs in the app.

- **Linux installation and updates for the desktop app.** The GPUI desktop app runs on Linux but has no install story:
  no package (tarball, .deb, AppImage or Flatpak not chosen), no desktop entry or icon install beyond what a build
  provides, and no auto-update (the updater runs only in the installed Mac app, so Linux shows no update UI). Decide the
  package form and architectures, how updates arrive (a self-updater, the package manager, or manual), and write the
  install docs and SPEC "Installation and updates"' Linux sentences to match. Known Linux issues to settle with it:
  - No desktop entry or icon is installed, so there is no menu launcher. The app id the entry must match is
    `org.scode.farhelm.desktop`, the Mac bundle id, reused on Linux until decided otherwise. A second launch brings the
    open window forward by forwarding its `XDG_ACTIVATION_TOKEN`, so the launcher must pass one for the compositor to
    allow focus.
  - A launch without a Wayland display refuses with a line on stderr only (Linux has no native alert on that path), so a
    launch from a desktop menu in an X11-only session shows nothing. X11 is not supported at all.
  - No updater and no update UI on Linux: the updater runs only in the installed Mac bundle.
  - The local supervisor is started detached and outlives the app, as on the Mac; whether an installed Linux app should
    run it some other way (a systemd user unit, say) is open. SPEC "Topology"'s Linux NOTE says how the Linux app runs
    and keeps its local supervisor is not specified yet; it needs replacing once this is decided.
  - Under KDE (KWin draws the title bar) the app is untested: no drawn bar, no GPUI Kit frame margin, the 722 by 596
    minimum, and drag, resize and close by KWin's own bar.
  - In a real GNOME session the drawn title bar's drag, double-click maximize and Minimize are untested (headless Mutter
    has no pointer drag and no visible minimize), and a close the compositor asks for (Alt+F4) reaches the app by
    another path than the drawn Close button, also untested.

- **Harden the repository cache's Git lock repair.** Since the repo-cache-stale-git-locks plan (#3296), checkout
  preparation and background cache cleanup delete Git lock files left in a host's repository cache by an interrupted
  job, while holding Farhelm's own exclusive cache lock (`clear_git_locks` in
  `crates/farhelm-supervisor/src/repo_cache.rs`, called from `launch.rs`; SPEC_impl.md says cache Git processes "end
  with their preparation job"). The landing review left three gaps, bundled here for one decision on which to fix:
  - **A Git that outlives Farhelm can race a later launch.** Git's own lock file used to be the backstop: a Git process
    still running after Farhelm's lock was released made the next fetch fail cleanly. Now Farhelm deletes that file. If
    Farhelm's processes are killed by hand with SIGKILL during background cache cleanup, its `git` keeps running with
    nothing to stop it, and a later launch can then race it on the cache's branch list. Nothing in Farhelm does this
    itself. Recommended fix: on Linux, have the kernel end cache Git processes when their parent dies (a parent-death
    signal on the spawned `git`), which also makes SPEC_impl.md's "end with their preparation job" true. Even an
    ordinary termination signal lets Git run its own cleanup just after Farhelm's lock is released; the worst case found
    is that launch's checkout failing once and a retry succeeding. Without the fix, soften that SPEC_impl.md wording.
  - **Reftable storage and per-branch history logs are not covered.** The repair removes the lock files of Git's default
    branch storage (`packed-refs.lock`, `HEAD.lock`, `config.lock`, `shallow.lock` and per-reference locks under
    `refs/`). A cache using Git's newer reftable storage, or with per-branch history logs (`logs/`) turned on, can leave
    locks it does not remove, and checkouts then fail as before the plan. Farhelm turns on neither, but cache Git runs
    with the user's own Git settings, so a global `init.defaultRefFormat` or `core.logAllRefUpdates` setting can. Either
    remove those locks too or run cache Git with the settings pinned.
  - **A cache path that is a plain file gives a misleading error.** When the cache path is a file instead of a
    directory, a checkout fails with "could not clear cache Git locks: Not a directory" rather than Git's own error. No
    working cache is affected, but the message points at the lock repair, not the broken cache. The test that pins the
    new message would change with it.

  The plan's test covers one planted `packed-refs.lock` on a launch's checkout; the walk through `refs/`, the refusal to
  follow a symlinked `refs/`, and the repair on the background cleanup path are untested and worth covering with
  whichever fix lands. Recorded by the maintainer on 2026-11-18 from the plan's review.

  Sent back unplanned on 2026-11-21: the maintainer does not want these gaps patched one by one. Their words: "We need
  to fundamentally rethink this. This cannot be this complicated. We cannot rely on these implementation details and
  whatever in Git; we need to fundamentally redesign this." Planning research that day also found that the parent-death
  signal recommended above would not cover background cleanup, where `git gc` hands the branch-list rewrite to a child
  process the signal does not reach, and that it is Linux-only while the cache runs on Macs too.

- **Remove the "Remove the bundled tmux" step from Add and Update.** Since tmux-drop-from-install (#3371 to #3374),
  every Add and Update plan for a remote Linux host includes a step that deletes the tmux copy earlier releases
  installed beside Farhelm. It is needed while Update can reach a host still carrying that copy: any release from 0.23.0
  (the current upgrade floor) up to the one before tmux was dropped may have installed it. Remove the step, its
  explanation, docs and tests once the upgrade floor passes the release that dropped tmux, or together with "phase 3"
  (Mid term), whichever comes first; a host updated after the step is gone keeps an unused copy, which is harmless.
  Decide the timing when this is picked up. Recorded by the maintainer on 2026-11-18 from the plan's review.

- **Finish the tmux cleanup: the known leftovers and one final sweep.** tmux-drop-from-docs (#3381 to #3385) audited
  every remaining tmux and xterm.js mention and left these, each with the plan's recommendation:
  - The 65-second "agent has not reported its identity" notice skips Enters inside a paste by scanning typed input for
    xterm.js-style paste markers. The desktop app sends pastes as separate messages, so the scan probably never matches
    now; confirm and remove it.
  - Delete runs an extra scan of every process on the host for every session that was ever restarted, because an earlier
    release's Restart could leave tmux tabs nothing lists; its cost was never measured. Goes with "phase 3".
  - The per-session terminal-kind tracking and the paths that read it, test fixtures that still create old-kind rows by
    default, and about 3,000 lines of test tooling for real tmux servers that now serves only the startup step's four
    tests. Goes with "phase 3".
  - Test-only: a half-second pause before an agent exits in the session lifecycle tests, and an extra error an
    Attach-racing-Delete test still accepts; both existed for tmux, and nobody has checked whether terminal hosts need
    them. Check and remove with "phase 3".
  - Renames when the code is next touched: the "pane" vocabulary across the supervisor and tests,
    `PRIVATE_TMUX_ENV_VARS` (now about the user's own tmux), and test names saying "new terminals" or "never in tmux".
  - Left as they are by the plan: status detection's 16-screen read budget per round and the 10-second prompt delivery
    timeout, both sized for tmux; revisit only if a better value appears.

  Then sweep the repository once more for tmux, xterm.js, pane and old-terminal mentions and remove or reword whatever
  is left that does not belong to "phase 3". Recorded by the maintainer on 2026-11-18 from the plan's review.

- **Cmd-P quick launch sometimes leaves the previous session active.** there is a bug whereby when i use Cmd-P to quick
  launch, sometimes, i'm not sure when, it just creates the session without making it the active session. observed on
  0.39.0. i cannot reproduce consistently, but twice it happened when i had an active codex agent. visigle, at least
  once after having typed in it, and then hitting cmd-p. Findings, 2026-11-18: code reading (no change to the path since
  0.40.0) found no ordinary cause. The maintainer did not recall a "Started" pill, which the designed "moved on" path
  shows. One rare helm-side corner case matches every symptom: the helm's best-effort cache write of a newly created
  session (`record_session`, called from `accept_created_session`) can fail when the host reconnected during the create,
  so the list pushed with the reply is complete but lacks the new session; the desktop app opens it, sees it proven gone
  (`listing_proves_gone`), and reselects the previous session silently. The helm log would then say "could not record
  the session for routing". Nothing ties that to an active Codex agent, so it stays unconfirmed; the maintainer chose to
  keep watching for it.

- **Consolidate the helpers duplicated across Farhelm.** Work through the repeated helpers from the 2026-11-21 review.
  Reuse the existing shared module where one fits and keep each helper scoped to its actual callers. Locations below are
  from the reviewed revision `c9575db61abb31bebe43da0f11a76712e18725c1`. Read GUI locations relative to
  `crates/gui/farhelm-gui/src/` and desktop locations relative to `crates/farhelm-desktop/src/`.

  - `approval-views-repeat-status-lookups.md`. In `approvals/host_add.rs:95–110` and
    `approvals/tunnel.rs:173–182,194–202`, share one approval-row lookup and one in-progress operation lookup by request
    ID in the approvals module.
  - `gui-command-paths-repeat-reply-classification.md`. In `operations.rs:546–556,633–643`, let the keyless command path
    project `Outcome::from_reply` into body or refusal words. Keep its command-error handling separate.
  - `hosts-menu-and-settings-repeat-operation-logic.md`. In `hosts/menu.rs:485–494,513–518,568–574`, have Remove use
    Delete's existing `close` helper after taking the focus mark. In `hosts/identity/phase.rs:409–417,575–586`, share
    the suspend/resume infinitive while keeping provider-specific subjects in their lines. In
    `hosts/progress.rs:151–180`, `hosts/status.rs:425–448` and `hosts/uninstall.rs:427–449`, share the operations-store
    ID check and conditional update that discards old-session state and notifies only on change. Keep separate globals
    for the observers. In `hosts/setup.rs:888–922` and `hosts/remove.rs:429–465`, share permanent-answer save admission,
    clearing on admission and handling a busy preferences key or refused reply. Pass each panel's wording.
  - `launcher-repeats-choices-and-summary-formatting.md`. Recheck these call sites against the landed
    `launcher-command-field` plan's redesign. Share the lowercase effort spelling and `default` fallback for search and
    the change notice beside the labels in `permission/words.rs:16–27`, used from `launcher/search.rs:413–419` and
    `launcher/view/changed.rs:104–110`. Share default-effort help across the select, radio group and search at
    `launcher/view/choices.rs:231–235,260–265` and `launcher/search.rs:361–362`. Share `gh:owner/name` on top of
    `repo_identifier` for `launcher/recents.rs:244–250`, `launcher/view/checkout.rs:1184–1191` and
    `launcher/draft.rs:194–197`, adding preview status where applicable. In `launcher/entry.rs:348–361,371–379`, share
    the harness-specific unsupported-resume formatter and uncaptured-conversation text, keeping the Agent and Command
    conditions. In `launcher/view/folder_list.rs:89–106` and `crates/farhelm-proto/src/text.rs:113–132`, use one
    iterator over whole escaped display units so truncation splits only between units. In
    `launcher/view/recents.rs:354–383` and `launcher/view/search.rs:654–655,683–694`, the two recent-setup surfaces
    repeated resolved permission rendering, including danger ink for YOLO. The old strip omitted the default. In the
    redesigned launcher, look at `recent_choice` in `launcher/view/search/start.rs` and `recent_lines` in
    `launcher/view/search.rs`. Share the permission projection and keep the Start column's chip and default omission,
    search's inline text, and each surface's summary local.
  - `gui-screenshot-scenes-repeat-fixture-wiring.md`. The landed `goldens-fast-and-targeted` plan changed the runner.
    The scene fixtures still need consolidation. Share one-shot dialog setup when a fake connection first has a UI
    session. Give the helper each scene's setup callback and retain the connection, fake helm and subscription. See
    `goldens/hosts.rs:55–85,125–146,154–176,188–209,795–817,960–988`,
    `goldens/other_dialogs.rs:235–253,282–299,332–350`, `goldens/templates.rs:52–69,129–167,208–228` and
    `goldens/settings.rs:122–183`. Share the session fixture's title and `TEST_NOW` activity/seen timestamps while
    scenes supply their status (`goldens/session_list.rs:42–47,114–119,154–160`, `goldens/other_dialogs.rs:112–117`,
    `goldens/mod.rs:301–307`).
  - `session-list-actions-repeat-admission-and-progress.md`. In `session_list/`, share operation admission across
    `menu.rs:601–615`, `environment.rs:625–642`, `restart.rs:278–306`, `stop.rs:226–247`, `delete.rs:366–392`,
    `environment_delete.rs:242–267` and `replace.rs:319–346`. The helper takes the command result and completion, finds
    the store, starts work under the session key and returns `NotStarted` on refusal. Keep guards, logging and progress
    clocks local. Share `outcome.rs:432–437`'s in-flight lookup with `stop.rs:285–290`, `restart.rs:314–319`,
    `replace.rs:414–419`, `delete.rs:435–442` and `environment.rs:323–344`, letting callers filter kinds. Share the
    status label, colors and spinner for the progress title in `delete.rs:484–516` and `stop.rs:296–327` with
    caller-supplied selector, label and flex sizing. Share the stale-cause mark's ID, name, role and tooltip in
    `row.rs:639–660,758–769`, with the clock or text styling supplied by compact and full rows.
  - `session-pane-repeats-last-known-status.md`. Share the last-known status and activity-age projections between the
    Details bar and unavailable-session view, keeping their visibility rules local. See
    `session_pane/details.rs:239–245,260–264` and `session_pane/unavailable.rs:561–569`.
  - `settings-repeat-preference-and-focus-wiring.md`. Share the sparse `PUT /api/preferences` constructor and diagnostic
    label in `settings/choices.rs:377–387`, `session_list/order.rs:212–220`, `session_list/host_rail/expansion.rs:71–79`
    and `terminal/tunnels.rs:605–617`. Let Order pass its object and the callers keep their write and completion
    policies. Share focus-mark restoration to the editor opener using the stored window when the callback has none in
    `settings/credentials.rs:362–371` and `settings/recipes.rs:717–727`, keeping editor and refresh bookkeeping local.
  - `templates-repeat-lookup-and-control-wiring.md`. In `templates/dispatch.rs:221–243,295–313`, share launcher and
    switcher template lookup and unavailable/missing-name wording, with each surface supplying its sentence ID. Share
    the available-name read in helm order in `templates/armed.rs:475–499` and `templates/editor.rs:1494–1507`. The
    sequence's Enter lookup still uses retained names. Share the Base name/Fixed name radio and help in
    `templates/editor/rows.rs:652–678` and `templates/save_panel.rs:308–340`, passing each control's ID, selection and
    handler. Share the typed `GET /api/launch-catalog` request and decode in `templates/dialog.rs:766–789` and
    `launcher/view.rs:477–500`, keeping retries, failure display and quick-launch continuation local.
  - `session-list-tests-repeat-frame-and-request-helpers.md`. Paths in this item are under `session_list/`. Share the
    mouse move, vertical pixel scroll and settle in `list_tests.rs:57–67` and `lifecycle_tests.rs:1707–1717`. Put the
    debug-bounds drawn predicate from `host_rail/tests.rs:1543–1546` and `menu_tests.rs:128–131` beside the existing
    test bounds helper. Share frame parent traversal across `host_rail/tests.rs:234–264,1759–1774`,
    `lifecycle_tests.rs:142–158,1142–1162` and `row_tests.rs:53–75`, keeping frame order, including the collection root
    and checking every match for reused IDs. Share fresh-frame painted-text collection in
    `environment_delete_tests.rs:94–97`, `environment_tests.rs:231–234` and `lifecycle_tests.rs:176–179`. Share the
    row-menu toggle, item click and settle in `environment_tests.rs:224–229`, `lifecycle_tests.rs:160–165` and
    `menu_tests.rs:146–151`, with the Delete wrapper supplying its item ID. Share a fake-helm drain returning commands
    acknowledged with empty 200 responses in order. Keep path filters and result shape at the callers in
    `seen_tests.rs:37–52`, `pin_drag_tests.rs:34–48`, `marking_tests.rs:396–409`, `pins_tests.rs:33–49`,
    `retired_tests.rs:119–129` and `host_rail/tests.rs:182–188`.

  Read the original findings with `git show 510f3b93721ee2393bd803d2cf91d73654415fbb:review_feedback_queue/<file>`.
  Plan: `plans/queue/helpers-hosts-and-sessions.md`. Plan: `plans/queue/helpers-launcher-and-templates.md`.

- **Report remote command status in-band, not through ssh's exit status.** Tailscale SSH on a Mac reports exit status 0
  for every command (tailscale/tailscale#18256, still open on 2026-11-17), so adding such a Mac fails at the first probe
  ("the supervisor probe closed before hello completion with exit status exit status: 0"), and past that a failed
  install step, a payload checksum mismatch included, would read as success. Have every provisioning command Farhelm
  runs on a host end with a status record carrying a per-run random tag, take the status from that record, and treat a
  missing or malformed record as failure with a clear error; keep ssh's own 255 as an exit status. The commands share
  one funnel, so this is a contained change. Decided 2026-11-17 over detecting Tailscale and refusing the host. Finish
  with a real end-to-end add of a Mac that has Tailscale SSH on, since Tailscale SSH on macOS has other open problems.
  Findings, upstream status and every place that reads exit statuses:
  `lore/2026-11-17-tailscale-ssh-macos-exit-status.md`. Plan: `plans/queue/in-band-host-command-status.md`.

- **Use the Tailscale SSH wrapper for Tailscale hosts.** If the user is using Tailscale, use the `tailscale ssh` wrapper
  instead of relying on the user manually getting known hosts initialized, so Farhelm can immediately use any Tailscale
  host it has ssh access to without fighting ssh known hosts. Separate from, and orthogonal to, the in-band status entry
  above. Not investigated yet, including how the wrapper fits Farhelm's non-interactive, connection-sharing ssh use.
  Plan: `plans/queue/tailscale-ssh-hosts.md`.

### Plan follow-up

- **Show Stop and suspend's continuing work and actual timeout failures.** The desktop drops every 504 from this
  command. A reply from the helm's 20-second bound means the operation is still running. Add a helm-only typed marker to
  that reply and show its program-neutral explanation in the session row and header, including while Stop runs before
  the host phase changes. A supervisor's own 504 has no continuation marker and must show as a failure. Cover both
  responses, and keep the other host and environment operations' reply handling separate from this fix.

- **Keep Tensorlake Suspend and Resume tied to the sandbox Farhelm created.** A failed setup can leave a sandbox name
  reserved before Farhelm sends Create. Refuse Suspend and Resume when the host has neither a saved sandbox ID nor an
  outstanding create. An outstanding create permits a name lookup. Save the resolved ID and clear the outstanding marker
  together before sending any mutation, then use the ID for status polls and retries. Keep the existing provider
  deadline across the lookup and mutation. Let the local write finish under the host's provisioning lock, since
  cancelling its wait can leave the SQLite write running after that lock is released. Cover both verbs on a failed row
  and confirm a retry uses the saved ID after recovery.

- **Keep all sandbox leftovers visible when Tensorlake discovery fails.** During unfinished cleanup, a failed refresh
  can replace the previously listed sandboxes with the cleanup rows alone. Have the helm report the discovery failure
  alongside the current cleanup snapshot, and keep the last discovered rows in Settings. Use the sandbox ID to show one
  row when cleanup supersedes discovery. A known empty cleanup snapshot after the last cleanup finishes should remove
  its old **Cleaning up…** or **Retry cleanup** row even during the outage. Keep the existing reply bound, and
  distinguish a discovery failure from an individual cleanup error in Settings. Cover refresh, cleanup completion during
  the outage, and recovery.

- **Move Codex recipe endpoint credentials out of its settings.** Farhelm can overwrite a concurrent save to
  `~/.codex/config.toml` when it delivers a recipe endpoint. Keep the `farhelm` provider ID so existing sessions can
  restart and the launcher can open saved conversations by ID. In an isolated Codex 0.171.0 loopback probe, a launch
  override that omitted the token field sent the stale settings bearer to the new endpoint (recorder
  `c235a277-a9fb-497d-a323-ebb0538a8976`). Explicitly clearing the field sent an empty Authorization header (recorder
  `59601200-f69b-4179-99a6-a94f4a685ec5`). Find an override or compatible isolation mechanism that preserves saved
  conversations, user settings and requests to tokenless endpoints without an Authorization header.

- **Explain stale launcher choices.** In the empty command field, Enter on a recent setup that has disappeared can do
  nothing without saying why. Clicking a search result for a host that just vanished can also clear the query without
  selecting anything. In `launcher/view/search.rs` and `search/start.rs`, use the existing stale-result warning before
  dropping either choice. Keep the search text, and keep a withdrawn keyboard selection from falling through to Launch.
  A missing explicit host or recent choice in typed results should use the same warning. Preserve the history reset when
  the host installation changes so an old choice cannot match a different installation.

- **Name the program a Stop or Delete approval would end.** An agent can request Stop or Delete of a Bash or Zsh
  session, but the approval card says it would stop an agent. The card in `approvals/model.rs` needs the resolved
  target's launch kind from `ApprovalSession`. Carry that fact compatibly and use program-neutral wording if an older
  helm cannot supply it. Say which processes the action would stop, including terminal tabs where the deletion guard
  permits it. Keep the approved deletion tied to the same target and guard. Test shell targets in both actions and check
  the approval wording with and without live processes.

- **Attach files pasted from a Wayland file manager.** A terminal paste can ignore copied files or type their displayed
  paths. In the vendored Wayland `clipboard.rs`, read `text/uri-list` before image or text, as SPEC.md's “Attachments”
  requires. Accept only local `file:` URLs. Keep ordinary paste working when the list has only web links or invalid
  entries by reading the next available flavor from the captured offer. Cancel the paste if that offer has expired. Keep
  the vendor patch and its tree in sync, and test both a file offer and a browser link offered as URI and plain text.

- **Use the desktop's reduced-motion setting for the attention cue and other motion.** On Linux the attention cue can
  disappear when the desktop portal and Farhelm's startup setting disagree. `layers/cue.rs` starts a flight from the
  startup value in `ticker.rs`, while GPUI can jump the flight to its transparent last frame. Follow the toolkit's
  preference everywhere and remove the separate desktop startup read. Use the stationary flash from SPEC.md's “Status”
  while reduced motion is on, including during a live change. Make ongoing cursor and status animations pause and resume
  with the preference while clocks and one-shot deadlines keep running. Test a preference change in an open window.

- **Show the main terminal when a remembered shell tab cannot attach.** Reopening a session can leave it stuck
  reconnecting to a remembered shell tab whose terminal the supervisor cannot read. Fall back to the main terminal after
  an explicit initial refusal, even if the shell tab remains listed. The terminal stream currently uses the same detach
  code for that refusal and for some dropped supervisor connections. Give the desktop app a way to tell them apart so a
  temporary outage keeps retrying. Route pending upload notices to the terminal the app ends up showing, and keep the
  shell tab available for a deliberate selection.

- **Retain failed Restart cleanup when the session was deleted.** When Delete removes the session row while Restart is
  launching its replacement, `tear_down_relaunch_deleted_meanwhile` reports an unconfirmed replacement scope and keeps
  the replacement's host record. That record carries the socket and starter identity, while the deleted row held the
  scope selection. Startup can remove the record after finding the host gone even if its scope still holds a detached
  descendant. Preserve the unresolved scope selection across row removal and supervisor restart. Retry cleanup using the
  session and generation's derived scope name, and retire the obligation once the scope is confirmed gone. Cover a gone
  host with an unconfirmed scope that becomes checkable on a later start. Plan:
  `plans/queue/restart-deleted-cleanup.md`.

- **Keep working container images when Rebuild fails.** Rebuild currently retires a recipe's Podman software and
  prepared images before building replacements (`crates/farhelm-helm/src/environment/podman/build.rs`). A package or
  installer outage can leave a previously usable recipe unable to start new sessions. Force actual rebuilding with a
  flag, retain the last usable images until replacement succeeds, and keep the ordinary freshness rules for later
  launches. Launches for the same recipe and place must still join an active Rebuild and receive its outcome, even while
  old images remain recorded. Coordinate source selection with the job registry and release any source guard before
  awaiting the rebuild's publication. Test both a failure with a usable fallback and a launch concurrent with Rebuild.
  Deferred from the Podman triage plan because preserving active-job joining exceeds the original size gate. Plan:
  `plans/queue/keep-images-on-failed-rebuild.md`.

- **Apply host identity rules to Podman places.** If an SSH alias starts reaching another machine or account, a Delete
  can see no container there and forget a live one on the original host. Two aliases for the same machine and account
  can also both appear usable for new containers. Use one stable machine/account identity for both outcomes. Record it
  for the execution endpoint before sending a new container create, and dispatch only to that verified endpoint. Verify
  the identity on the connection that reports absence before accepting that as proof of deletion. Refuse new containers
  on a second place for the same machine and account while preserving safe cleanup for existing bindings. Honor the
  ordinary supervisor's identity refusal when it is available and keep support for places without a supervisor. The
  relevant code is in `environment/podman.rs`, `environment/podman_probe.rs` and `store/podman.rs` in the helm. For an
  older binding whose create may have been sent, finding the owned container can establish its identity. If the
  container is absent, the helm cannot distinguish a removed container from a changed route. Decide how users can clear
  those rows without assuming the current endpoint is the original one. Cover both the retargeted alias and the
  duplicate place in tests and update SPEC. The Podman triage plan deferred the pair together because the older-row
  behavior is unsettled. Plan: `plans/queue/podman-place-host-identity.md`.

- **Evidence shows Muse Code conversation reporting is broken (1.4.24).** During the `harness-drift-tests` live run on
  Muse Code 1.4.24-R9131.1, Farhelm injected its hook flags and Muse replied to the prompt, but no conversation report
  arrived and Farhelm's hook helper never even wrote a diagnostic log line before teardown—indicating conversation
  reporting is broken on current Muse Code so Restart cannot resume Muse conversations. Find out why the callback did
  not fire and fix or adapt Farhelm's Muse integration.

- **Verify Restart actually resumes the same conversation in harness drift tests.** The `harness-drift-tests` Restart
  check currently verifies only that the CLI relaunches with Farhelm's saved resume arguments and accepts fresh
  unsubmitted input in the composer, so it would pass even if an agent silently opened a brand-new conversation instead
  of resuming. Update the drift tests for all eight tracked agents (Claude Code, Codex, Antigravity, Goose, Grok, Muse
  Code, OMP, Pi) to prove the resumed session actually stayed in turn 1's conversation — for example by checking turn
  1's history or recall and confirming the post-resume hook reports the same conversation ID. Plan:
  `plans/queue/drift-verify-resume.md`.

### Tensorlake

- **Agent-requested Tensorlake hosts.** Let an agent request a host by naming an existing recipe, behind the user's
  approval.

- **Investigate Tensorlake's ssh gateway leaving forwarded channels open, and report it if it is a bug.** Through the
  gateway, an ssh client holding a remote-forwarded connection never learned that the sandbox side was gone: the sandbox
  side was torn down within half a second, but the local ssh and its connection to the local service were still up 185
  seconds later, with keep-alives on or off, while an ordinary OpenSSH server ended the same setup in about 1.3 seconds
  and the gateway did end a connection with no forwarded channel open. Confirm what the gateway does (including whether
  it relays a forwarded channel's close in the ordinary case where only the sandbox-side client hangs up), and if it is
  a gateway bug, send Tensorlake a report with the minimal reproduction. Everything known so far, measured and inferred
  kept apart, is in `lore/2026-11-07-tensorlake-gateway-forwarded-channels.md`.

- **Skip Claude Code's first-run setup on new Tensorlake hosts.** Decided by the maintainer on 2026-11-07, answering the
  settings-credentials plan's block: Farhelm delivers a selected Claude subscription to a Tensorlake host but writes no
  other Claude configuration, so the first Claude session on a fresh host can stop at Claude Code's own first-run setup
  (its theme chooser and onboarding), which the user goes through once. Claude Code 2.1.318 stopped there in a fresh
  home and also checks a separate onboarding-completion setting. The fix the maintainer deferred: have Farhelm write the
  smallest set of non-secret first-run settings Claude needs, found by an isolated probe, keeping the user's other
  settings and the normal agent home, and check it with a valid subscription login.

### Drift test feature coverage

- **Drift tests: base launch, Codex process isolation, and Cursor/OpenCode interactive turns.** Verify that Codex's
  `--no-daemon` keeps hooks and commands inside the session's process tree and still launches cleanly when hooks are
  disabled, verify Cursor's default launch without a permission flag ("Agent's own setting"), and send a real prompt to
  Cursor and OpenCode to confirm interactive turns work under Farhelm's terminal host (today both stop at the initial
  TUI or trust screen).

- **Drift tests: model selection across all supporting agents.** Add live model-flag coverage for Cursor (`--model auto`
  and catalog models) and OpenCode (`--model opencode/<id>`), cover Goose, Pi, and OMP with both
  `--provider openrouter --model <id>` and no explicit provider, validate current catalog model IDs across Claude,
  Codex, Antigravity, and Muse (rather than only `haiku` or one model), and verify each CLI actually selects the
  requested model rather than silently ignoring it.

- **Drift tests: reasoning effort selection.** Add live coverage for Muse (`--reasoning-effort`) and Goose
  (`GOOSE_THINKING_EFFORT`), which currently have zero drift-test coverage, and exercise non-`low` effort values
  (especially `off`, `minimal`, `xhigh`, and `max`, which vendors rename or drop) across Claude, Codex, Antigravity, Pi,
  and OMP, confirming each CLI accepts and applies the requested effort.

- **Drift tests: permission modes and custom permission flags.** Cover YOLO launch flags on Claude, Codex, Antigravity,
  Grok, Muse, and OMP (currently only Guarded is exercised for those six); Goose's `approve`, `smart_approve`, and
  `chat` modes (`GOOSE_MODE`); OMP's `Approve` mode (`--approval-mode always-ask`); Cursor's "Agent's own setting"; and
  custom permission flags surviving Restart (including Antigravity's `--settings` and `--` argument ordering).

- **Drift tests: initial prompt delivery via launch arguments.** Cover all eight agents that accept a launch-time prompt
  (Claude, Codex, Cursor, Muse, OMP, and Grok via `-- <prompt>`; OpenCode via `--prompt=<prompt>`; Antigravity via
  `--prompt-interactive=<prompt>`), including leading-dash and multiline prompts, proving each starts interactively,
  submits the prompt automatically, does not re-send it on Restart, and works with a named conversation start where
  supported.

- **Drift tests: one-time named conversation starts.** Cover starting a session on a named conversation ID or file
  across all ten agents — especially Cursor's `--resume <chat-id>` and OpenCode's `--session <session-id>`, which have
  no drift coverage today — plus relative and absolute session file paths on Pi and OMP, verifying capture on tracked
  agents and Goose clearing prior capture.

- **Drift tests: conversation capture timing, Stop/UserPromptSubmit hooks, and hooks-disabled mode.** Verify
  `restart_readiness` timing (pre-prompt on Claude and Goose, first-prompt on Codex, Muse, Grok, and Antigravity,
  first-reply on Pi and OMP), end-to-end `Stop` and Grok `UserPromptSubmit` hook payloads through Farhelm's own hooks,
  OMP's 256-byte title-slot prelude, hook invisibility in the TUI (including Antigravity's `{}` stdout), and disabling
  hooks via `FARHELM_AGENT_HOOKS=none` (including Goose's resume-time disable and Codex keeping `--no-daemon`).

- **Drift tests: opt-in persistent hook installation for Grok and Antigravity.** Using isolated config directories so
  the real user home stays untouched, verify that the exact hook files Farhelm installs for Grok (`hooks/farhelm.json`)
  and Antigravity (`plugins/farhelm/{plugin.json,hooks.json}`) are discovered and invoked by the real CLIs inside
  Farhelm to capture conversations, and exit cleanly and silently when the CLIs run outside Farhelm.

- **Drift tests: in-session conversation rotation.** Wire the existing standalone Claude `/clear`, Codex `/new`, and
  Codex `/clear` checks into the drift runner (running Codex `/clear` end-to-end through Farhelm's own hook so pending
  withdrawal and `Stop` confirmation are tested), and add live rotation coverage for Muse (`/clear`, `/new`), Pi
  (`/new`, `/fork`, `/resume`), OMP (`/new`, `/fork`, `/resume`), Grok (`/new`), and Antigravity, proving Restart
  resumes the rotated conversation.

- **Drift tests: subagent and nested-agent report rejection.** Wire Claude's standalone shelled-out `claude -p` test
  into the drift runner and add live coverage for both native subagents and shelled-out nested agents across all six
  foreground-ownership harnesses (Claude, Codex, Muse, OMP, Grok, Antigravity), proving child invocations cannot replace
  or corrupt the foreground session's conversation ID.

- **Drift tests: workspace trust pre-trust settings.** Assert that enabling workspace trust actually prevents the trust
  prompt from appearing on Codex, Muse, and Antigravity (instead of silently answering a prompt that still appeared),
  and add live coverage for Antigravity's per-launch `--settings '{"trustedWorkspaces":[...]}'`, Pi's `--approve` and
  `--no-approve` with a project-local config or extension, Claude's `.claude.json` pre-trust edit in an isolated config
  directory, and Codex's explicit untrusted setting.

- **Drift tests: terminal screen status and dialog detection.** Verify live anchored `Running` and `Idle` readings on
  Claude, Codex, and Muse (catching silent fallback to generic change-counting when vendor TUI layout drifts), live
  workspace-trust prompt detection on an untrusted directory for Claude, Codex, Muse, and Antigravity, and live
  in-conversation `Waiting` approval/question dialogs and `Unknown` overlays (such as model pickers) on Claude and
  Codex.

- **Drift tests: permission-mode status-line footer detection.** Verify Farhelm's live screen-footer classifier reports
  YOLO vs non-YOLO accurately on both YOLO and Guarded launches for Claude, Grok, and Muse, and YOLO on `--force`
  launches for Cursor, catching vendor footer wording or layout drift.

- **Drift tests: `$farhelm` instructions pointer delivery.** Verify all six supporting agents (Claude, Codex, Muse via
  `SessionStart` hook stdout; Goose via MCP `initialize` instructions; Pi and OMP via `--append-system-prompt`) actually
  receive the `$farhelm` instructions pointer in model context, and that disabling instructions omits it without
  breaking conversation hooks.

- **Drift tests: running-agent prompt delivery via bracketed paste.** Verify live Claude and Codex TUIs enable terminal
  bracketed paste mode, and that Farhelm's bracketed-paste plus Enter (Claude) or Tab (Codex) submits cleanly at idle,
  preserves multiline prompts without premature submission, and queues cleanly while busy.

- **Drift tests: environment and recipe credential delivery.** Using isolated Claude (`CLAUDE_CONFIG_DIR`) and Codex
  (`CODEX_HOME`) directories and a local loopback stub HTTP endpoint (zero API cost), verify real Claude and Codex CLIs
  still honor Farhelm's generated credential configuration and send requests to the configured `base_url` with the
  expected auth header.

- **Drift tests: wrapper prefixes and declared command launches.** Verify shell (`bash -c`) and `env` wrapper prefixes
  and declared command launches (`{farhelm_args}`, `{cwd}`, `{codex:trusted-cwd}`) end to end across agents,
  specifically proving conversation capture and Restart survive `exec`-based wrappers on Grok and OMP and resident
  wrappers on Claude, Codex, Muse, and Antigravity.

- **Drift tests: remote `BROWSER` opener integration.** Verify how real agent CLIs on Linux — especially Codex and
  Claude Code, which differ in whether they accept a command template or only a single executable path — invoke
  Farhelm's remote `BROWSER` opener.

### Trivial

- **Scrub link-local and tailnet addresses in agent screen-capture privacy guards.** Neither the screen-capture script
  (`scripts/capture-agent-screens.py`) nor the Rust screen-fixture privacy check treats link-local addresses
  (`169.254.0.0/16`) or tailnet/CGNAT addresses (`100.64.0.0/10`) as private, so a captured screen showing one could
  reach a committed fixture. Teach both guards to scrub or refuse both ranges and add tests. Plan:
  `plans/queue/scrub-linklocal-tailnet.md`.

- **Decide which button is primary on the launcher's hook setup prompt.** When starting a Grok or Antigravity session
  with automatic Restart hook setup turned off in Settings → Agents, the launcher shows a prompt asking whether to let
  Farhelm install its hook files into that agent's configuration so **Restart** can resume conversations. Currently
  **Launch without** is styled as the primary button and **Set up and launch** is the secondary button (chosen during
  `agent-hook-setup` because the prompt asks consent to modify the agent's saved configuration). Revisit when planning
  whether **Set up and launch** should be the primary button instead (or keep **Launch without** primary), and update
  the prompt and its golden screenshots if changed. Plan: `plans/queue/hook-setup-primary-button.md`.

- **Replace on a guarded Codex session asks to confirm a YOLO launch.** it's a guarded session and i clicked replace and
  got a dialog indicating farhelm wants to replace with a yolo session instead of guarded. codex session. macos
  supervisor. (Screenshot, 0.44.0-rc.4: a Codex session on Local (this machine), shown with the guarded shield mark in
  the session list, is selected. After Replace, the session pane shows a red "Confirm YOLO launch" banner: "YOLO means
  the agent bypasses its safety checks: no person, classifier, AI reviewer or sandbox stops its actions on Local (this
  machine) without asking you. This launch uses the agent's own setting, which Farhelm cannot override or classify. It
  is treated as YOLO on this host. Local (this machine) asks before every YOLO launch. Nothing has been started." with
  Cancel, "Start, and don't ask again on this host" and "Start YOLO session anyway".)

## Mid term

- **Select-all key for Linux text fields.** Since the emacs-text-field-keys plan, Ctrl+A in a Linux text field moves to
  the start of the line, as on macOS, so Linux text fields have no select-all key (no Edit menu, and fields have no
  context menu). Decide how and whether to have one.

- **Built-in questions tool.** A built-in questions tool in the style of the interactive artifacts: how an agent asks
  the user a structured question (multiple choice, say), presented like those artifacts rather than as plain text.

- Initial version of agent2agent comms.

- **Remove the startup shutdown of the old private tmux server ("phase 3").** The tmux removal plans
  (`tmux-end-old-terminals` and its followers) keep one piece of tmux on purpose: a supervisor that finds the private
  tmux server an older release left running turns it off at startup, marks the sessions that ran in it as interrupted
  and tells the user. Once no installation can still hold such a server, remove that step and what exists only for it:
  the startup code and its tests, the stored terminal-kind column and its reads (a schema change), the old-terminal
  notification kinds, the scrub of `FARHELM_TMUX` from session environments, and the test fixtures that build a server
  in the old layout. Asked for by the maintainer on 2026-11-11 while planning the removal. When that is safe (a release
  count, or the upgrade floor passing the release that ended old terminals) is to be decided when this is picked up.

- **Re-assess letting agents prompt running sessions.** `farhelm agent prompt` (an agent sending a message into another
  session's running Claude or Codex agent) landed and was turned off before any release: the helm refuses every such
  request, and the command is hidden from agents. One gap remains where a delivered message could do something the user
  never agreed to. The "is it waiting on a question?" check uses a status reading up to about two seconds old, so a
  permission or trust question that appears in that window, or right after the agent starts, can receive the message and
  its Enter as its answer. Re-enabling means fixing that gap or deciding to accept it, then setting
  `AGENT_PROMPTS_ENABLED` in `crates/farhelm-helm/src/agent_requests.rs` to true, unhiding the CLI verb, restoring its
  agent instructions and docs, un-ignoring the end-to-end delivery tests, and writing a new changelog fragment. SPEC.md,
  Agent-spawned sessions, keeps the design.

- **Support Kimi Code.** Add Moonshot's Kimi Code CLI as an agent type: terminal launches with guarded and YOLO
  permission flags, model, effort and prompt choices once their command-line contracts are verified, a harness mark and
  docs page, then session tracking — per-launch hooks/instructions, conversation capture for resume, and waiting/status
  recognition.

- **Support DeepSeek's coding agent.** DeepSeek publishes models, but no clearly official terminal coding agent was
  found on 2026-10-28; the DeepSeek CLIs that turn up are third-party. Confirm the product and its binary first, then
  add it the way Kimi Code is to be added.

- **Consider auto-scaling the transfer window.** Uploads, downloads and tunnel relays between the helm and a supervisor
  each have a fixed 4 MiB credit window, so one stream's throughput is capped near 4 MiB per round trip whatever the
  link can carry. Asked for by the maintainer on 2026-11-08 while planning the chunk-size change
  (`plans/queue/mux-chunk-size.md`): consider sizing the window automatically from bandwidth and round-trip time,
  bearing in mind that on remote hosts ssh's own channel window, shared by every stream, may be the real cap.

- **Cap the Mac app's own localhost tunnels.** Recipe tunnels are capped at 64 tunneled connections per host, but the
  relays the helm opens for the desktop app's localhost tunnels take no permit, so their number is unbounded
  (`crates/farhelm-supervisor/src/service/tcp_relays.rs`, around the relay start). Asked for by the maintainer on
  2026-11-08 while planning the chunk-size change.

- **Search sessions by content or action.** Let the user find a session by what happened in it — which session ran a
  certain task, command, or produced a given output — rather than only by name. What gets indexed and how the search is
  exposed are TBD.

- **Make auto-tunneling safe.** Figure out how to make auto-tunneling properly safe: can we use some hostname trick so
  the service does not present as localhost? TBD. Pinggy? TBD.

- **Restore automatic recovery from a withdrawn waiting update.** On 2026-10-14 the signed release advisories plan was
  narrowed to warning about a withdrawn running version and refusing to install a withdrawn release; a withdrawn release
  that is already installed and waiting for a restart now needs manual recovery, and an ordinary relaunch may still open
  it. The plan, with the blocked question and the decision, is
  https://github.com/scode/farhelm/blob/d8f02862ce4014255d02d4383910999b61c85dbb/plans/queue/release-advisories.md.
  Automatic replacement was blocked by what execution found about supervisor startup: a supervisor claims its state and
  migrates its session database before it checks tmux and before it records its running version. If startup fails or
  stops in that window, the database is newer than the record says (or there is no record), so an update check that
  trusts the record could install an older release that then refuses the database. The ordering exists in shipped
  releases (v0.28.2 included) and was reproduced with a migration from schema 30 to 31 followed by a tmux refusal while
  the record still named 0.23.0. The follow-up: have a supervisor durably record the version it is about to run before
  it can migrate any state, handle compatibility conservatively when it cannot be established (existing installations
  and interrupted startups included, without treating old records as trustworthy retroactively), check how commands
  inside sessions select a binary under the new meaning of the record, and then restore the automatic replacement. Start
  with a focused review of supervisor startup and interruption before implementing.

- **Release stable from the approved RC without rebuilding.** When an RC has been built and declared correct, releasing
  stable should default to promoting that RC's artifacts, not rebuilding everything for the stable tag. Today a stable
  is cut from main and the tag-triggered workflow builds every target afresh, so stable binaries differ from the
  approved RC's even when nothing changed. First find out what prevents reusing the RC builds: whether cargo-dist (the
  generated `release.yml`, driven by `dist-workspace.toml`) can publish the RC's artifacts under the stable version, and
  what the release commit, version settlement, curated notes and the maintainer's signing of the six payloads demand of
  the build.

- **Scroll the grid terminal entirely from a back buffer it keeps itself, instead of history sent by the supervisor.**
  The page already keeps the scrollback it shows and scrolling sends nothing
  (`crates/farhelm-ui/assets/grid-terminal.js`, "Scrollback"), but that history arrives as `history` messages from the
  supervisor's emulator, one message per line that scrolled off, each line at the width it was written
  (`crates/farhelm-proto/src/grid.rs`, "History"), and a `history_reset` rebuilds it whenever the emulator resizes.
  Consider dropping that half of the stream: the grid client appends the rows a `frame.shift` pushed off the top to a
  back buffer of its own, keeping each line at the size it was shown at (the terminal's current size, not reflowed by a
  later resize), so scrolling never depends on the supervisor re-reading its emulator. Two constraints decide whether
  that works. Flow control skips screen frames while a client is behind but still sends every line that scrolled off as
  history, which is how it never loses output (SPEC.md), so a buffer fed only by the frames the client saw would have
  gaps; and a client attaching to a terminal that already has output has no local history to show. Both need an answer
  before the shape is settled. Related to, but not the same as, the grid stream's binary encoding (grid-binary-stream,
  landed 2026-11-09), which kept the split and changed how the same lines are encoded.

- **Look into the GPUI frame-source patch further.** The new desktop app carries a patch to its vendored GPUI that keeps
  GPUI's older macOS frame source (`CVDisplayLink`) on macOS 14 and later, instead of the `CAMetalDisplayLink` GPUI uses
  there by default. Without it, a key press took about 55 ms to reach the screen on the test Mac VM, against about 35 ms
  in the old app and about 30 ms with the patch, because the newer source presents a frame about three refreshes after
  it was drawn. The patch costs idle CPU (about 0.4% instead of 0.1%) and gives up macOS choosing the frame rate
  (ProMotion displays, low-power mode). Kept on 2026-11-13; look into whether Apple or GPUI may eventually drop the
  older path, whether the newer source can be made as fast (setting its frame latency to 1 made no difference on the
  VM), and how it behaves on real hardware with a ProMotion display.

- **Revisit drawing inactive windows at full frame rate.** The new desktop app draws a window at full rate even when
  another app is in front, matching the old app, instead of GPUI's default cap of 30 frames a second. That keeps output
  scrolling smoothly in a window you are watching but not using, at the cost of more drawing (and battery) while output
  scrolls in a background window, which is common while agents work. Decided on 2026-11-13 to match today; revisit the
  tradeoff.

- **Revisit the KDE-blur dependency decision.** GPUI's Linux backend pulls in `wayland-protocols-plasma`, a crate
  declared MIT that bundles KDE Wayland protocol XML files licensed LGPL-2.1-or-later, which are compiled into generated
  bindings in the Linux desktop app. GPUI uses it only as a fallback for blurring a window's background on older KDE
  Plasma, and Farhelm never asks for a blurred window. Decided on 2026-11-13: remove it with a small patch to the
  vendored GPUI if that needs no horrible hacks, otherwise accept it as protocol definitions rather than code. Revisit
  whichever was done, and whether the license check should catch GPL-family files bundled inside a crate whose declared
  license is permissive.

### Tensorlake

- **Live resize of Tensorlake hosts.** A host is created at its recipe's size, and a recipe's size change affects only
  hosts created afterwards. Tensorlake can resize a running sandbox in place (CAS image format only): it takes one to
  two seconds and keeps processes running; CPUs go up and down, but memory never shrinks below what the sandbox booted
  with, so a sandbox meant to flex has to boot small. Let the user resize a Tensorlake host (or apply its recipe's
  current size) without recreating it. Phase 1 left this out deliberately.

- **Per-repository warm snapshots for sandbox sessions.** Build on a recipe's prepared snapshot, then refresh each
  checkout with an incremental `git fetch` when a session starts. Decide how these snapshots are refreshed. Keep using
  the recipe's GitHub token delivery to each sandbox for private repositories. Design records:
  `lore/2026-11-03-tensorlake-sandbox-hosts-design.md` ("Phase 2, as far as it was thought through", plus the measured
  platform facts) and `lore/2026-09-29-per-session-sandboxes-as-the-remote-host-goal.md`.

- **Live resize for sandbox sessions.** Let a session's sandbox boot small and grow on demand, preserving its running
  processes. Build on "Live resize of Tensorlake hosts" above; expose it from the session's controls. The first version
  launches at the recipe's size and offers no resize control.

- **Possibly: shared scratch volumes for sandbox sessions.** Explore Tensorlake cloud volumes as scratch space shared by
  sandbox sessions.

- **Possibly: fork a warm sandbox session.** Explore a memory checkpoint or live copy that preserves running processes.
  A copied supervisor retains its identity and the sandbox's boot ID, so an independent fork needs an explicit answer to
  that ownership problem. Ordinary Clone starts a fresh sandbox from the recipe instead.

- **Sandbox-session conveniences left out of the first version.** Recipe recent setups, Save as template from a sandbox
  launch, and bulk Suspend, Resume and Stop and suspend. The maintainer deferred these when simplifying the first
  version on 2026-11-09. Sandbox launches record no recent setups, and Save as template is unavailable for a recipe or
  session-owned host target; recipe templates and individual lifecycle controls are supported.

- **Auto-suspend forgotten sandbox sessions.** A sandbox session bills its CPU and memory for as long as its sandbox
  runs. While the helm is connected, Tensorlake's idle timeout never fires, because the helm's connection counts as
  activity. A session the user forgot about therefore keeps billing until they suspend or delete it. The maintainer
  accepted that for the first version on 2026-11-08 and wants an automatic suspend later, for example after the agent
  has been idle for a while. The rule and its default are to be decided when this is picked up.

- **A UI activity log that feedback can include.** Keep a timestamped log of what the UI did (dialogs, focus, shortcuts,
  requests to the helm and how long they took), and add an "Include application logs" checkbox to Send feedback. The
  point is general: whenever "I did X and something odd happened" or "I did X and it took about 5 seconds" comes up, the
  maintainer files feedback right after noticing, and the log says what actually happened and where the time went,
  instead of an agent guessing from code. The motivating example: a launch typed as `gh:owner/repo` sometimes ended up
  as a session named exactly that in the wrong repository, and code reading could only speculate that the "Rename
  session" prompt after a quick launch took the typing. A log reading "shortcut ⌘P → quick launch; dialog opened: Rename
  session (after quick launch); focus → rename field; typed 11 characters; Enter → submitted" would have settled it in
  one report.

  What to log: semantic events, not raw input. Which shortcut matched and which action it dispatched; dialogs opening
  and closing, with their kind and why they opened where that differs (the post-quick-launch naming prompt versus an
  ordinary Rename); focus moving between named controls; submit, cancel and Escape; launches and other operations
  starting and completing; and every request to the helm with its start, end, outcome and duration. Timing is a
  first-class requirement, not a nicety: every event carries a monotonic timestamp with millisecond resolution (not only
  wall clock), and anything with a duration logs both ends, so "it took like 5 seconds" can be broken down into which
  request, wait or render was slow. Ideally the helm logs the same requests with its own timing under a request id the
  UI log also records, so a slow step can be split into UI, network, helm and host time; how far that correlation goes
  is a design question.

  Privacy, by construction rather than by care: never log typed characters, only counts ("typed 11 characters into
  Name"), since fields hold prompts, paths, repository and host names, and secrets such as API keys. Model events as a
  closed set of typed kinds rather than free-form strings, so a later edit cannot leak a session title or path by
  accident, and name controls by Farhelm's own labels, never by page content or peer-supplied values. Requests are
  logged by route shape, not by the ids, paths or names inside them. The checkbox reveals the exact log text that will
  be sent before Send, keeping the dialog's existing contract that what it shows is the whole submission. A SPEC.md
  "Feedback" update has to state this promise.

  Shape: an in-memory ring buffer per window (last few hundred events or about 10 to 15 minutes, to be tuned), never
  written to disk and never sent anywhere until the box is ticked. This covers the "only noticed afterwards" case as
  long as feedback is filed soon after. Most events can come from a few shared places rather than per-call-site logging:
  the shared modal dialog component, the shortcut dispatcher, one page-wide focus listener, and the API client for
  request timing.

  Caveats:
  - Size. Feedback is capped today at a 4000-character message (shared by the UI, the helm and the farhelm.io endpoint)
    and a 64 KiB request body (the helm and the endpoint); the maintainer plans to raise those limits. Compress the log
    in the client before sending. The dialog still shows the uncompressed text.
  - Where the log lands. The endpoint files each submission as an issue in a private GitHub repository, and an issue
    body is limited to roughly 65,000 characters and is not a place for a compressed blob. The endpoint will need to
    decompress and split the log across comments, or store it elsewhere and link it; decide before building the client
    half.
  - Who can read it. Whoever can see that private repository can read every included log, which is one more reason the
    privacy rules above must hold without exception.
  - The helm side has no file logging yet (stderr only, per SPEC_impl.md), so helm-side timing for a report would have
    to come from an in-memory buffer as well, or wait for file logging.

## Doc todo

- Tell users that a working-copy (checkout) root must be a location only they can write: Farhelm is not designed for
  roots shared with or writable by other local accounts, and does not check. SPEC.md's fresh-checkout section states the
  rule.
- Tell users that closing, exiting, or deleting a terminal tab also stops services the tab shell's startup files started
  there first (an `ssh-agent`, an editor daemon, a detached tmux server), and that services the agent's launch started
  first are not affected. A tmux autostart guarded by `TMUX` fires during the agent's launch, and one that execs tmux
  keeps the agent from starting. Likewise, a personal tmux server or an ssh ControlMaster that the agent is the first to
  start ends with Stop, Restart, Replace or Delete, and one a command in a tab is the first to start ends when the tab
  closes or the session is deleted, each with everything the user later ran in it or through it, including from outside
  Farhelm. SPEC.md's session view section and "Ownership during cleanup and provisioning" state the rules.
- Tell users what Stop, Restart, Delete and closing a tab can and cannot kill on a Mac, and on a Linux host whose
  supervisor runs without a usable systemd user manager. There, a background process that has detached from the
  session's terminal is found only through an environment marker, so one that hides or overwrites its environment
  survives: non-dumpable and setuid programs, and daemons that rewrite their process title, such as nginx with default
  settings or Postgres started through `pg_ctl`. Servers kept in the foreground under the agent are still stopped. Point
  Mac users at running such servers in the foreground, or stopping them themselves. SPEC.md's lifecycle operations
  section states the rule.
- Tell users that bash and zsh are the supported login shells on every host: agents and terminal tabs start through the
  login shell, and other shells, csh and tcsh included, may fail to launch them. SPEC.md's supported user environments
  section states the rule.

## Tricky bugs

## Deflake

### Difficult deflake

## Broken tests

## Code cleanup

### Turning rules into structure

A 2026-10-19 assessment of about 530 recent product fixes found that most were an agent missing a rule, written as prose
or per-call-site discipline, at one place when a sibling path already followed it. The five refactors chosen from it
landed on 2026-10-21. A second assessment that day found that recurring fixes had since moved mostly to UI state and
interaction. After an adversarial review, the maintainer chose the five behavior-preserving refactors below, each of
which makes one such rule something a type, a single owner or a default enforces. A behavior change aligned with the
problem a refactor targets (making drifted copies consistent, say) is allowed in its own `fix:` PR; unrelated changes
are not.

## Maybe later

- **Park the docs-shot pointer on a quiet point when no element qualifies.** Before each docs screenshot, the pointer is
  parked on an element with an id that nothing reacts to. A redesign that removes the last such element (as the
  launcher's title row went) breaks every shot of that surface until each gets its own park target. Fall back to a point
  in a dialog's padding, a gap between controls or the window margin, clear of every hover and tooltip area and of
  popovers. That needs the vendored GUI bridge (`vendor/gpui-agent`, changed together with its patch file) to hover at a
  coordinate, and on a Mac, where the real pointer moves, the point park has to work too.

- **Restore deleted sessions.** Support undeleting a session that is in the trash. Consider supporting undelete for
  sessions without a managed checkout too, by retaining their metadata.

- **Protect the repository against force pushes and similar, while agents can still merge.** Guard main and the release
  tags against force pushes, deletion and other history rewrites in a way that also binds agents, while still letting
  them open, publish and merge PRs as they do today. Main's branch protection already disallows force pushes and
  deletion, but it does not apply to administrators, and agents work through the maintainer's own GitHub account, so it
  does not stop them. Tags have no protection at all (the repository has no rulesets), though the release procedure says
  a tag is never deleted or reused. Work out what GitHub rulesets, bypass lists or a separate agent identity can enforce
  without blocking the PR flow.

- **Reduce the peak memory of snapshots made mostly of cell fixups.** The terminal snapshot encoder still builds cell
  fixups in their own buffer and copies them after the stream, so a fixup-heavy snapshot peaks near twice its size.
  Painting those bytes in place too needs a separate design; the ordinary stream-heavy case already reuses its final
  blob's allocation.

- **Crappy model warning.** Let the user pick a list of models they never want to run. Warn when an agent's TUI
  indicates that one of those models is being used anyway.

- **Input-method typing in grid terminals.** Terminals that run in a terminal host are drawn by the grid terminal
  client, which does not show an input method's composition at the cursor or place the candidate window there: composed
  text reaches the program once committed, but the composing itself happens out of sight. Doing it the way xterm.js does
  means porting its composition helper (a preview element and an input element kept at the cursor on every render, plus
  Chrome and Korean input that arrives outside composition events), which failed the complexity gate of the
  terminal-host work. Verifying it needs real input methods, not only synthetic composition events.

- **Let an agent delete its own session.** `farhelm agent delete` refuses a session's own id (plan
  `agent-delete-and-prompt`), because the request holds a lock keyed by the asking session that the delete would wait
  on, and the session's credential is revoked mid-request; SPEC.md declines `farhelm agent replace` for the same reason.
  Allowing it needs a shape where the helm answers first and deletes afterwards. Decided 2026-10-16 while planning agent
  deletion.

- **Reconsider an ordered WebSocket channel between the GUI and the helm.** On 2026-10-15 the maintainer chose ordering
  in the GUI instead (a request waits only for this GUI's unanswered saves; plan `ordered-gui-helm-requests`), after a
  planning deep dive. The deep dive found that the helm could replay WebSocket frames into its existing routes
  unchanged, but estimated about 5,000 to 8,000 lines over 7 to 10 PRs. The largest single cost is migrating the browser
  tests, which intercept API calls over HTTP in 461 places that would not see WebSocket frames. It also leaves lasting
  complexity: plain HTTP must stay for tabs left open across a helm upgrade, every new route needs an ordered-or-not
  decision, middleware must be kept in step across two ways into the helm, requests disappear from browser devtools as
  separate entries, and one stalled socket stalls every request. Its one gain over GUI-side ordering is fewer
  connections over a high-latency tunnel. Large transfers (attachments, downloads), terminals and the event feed would
  stay off the channel. Revisit if remote browser use over slow links becomes common, or if GUI-side ordering proves
  insufficient.

- **Further harden the release and update trust chain.** The chain is already hardened: installs and updates come from
  get.farhelm.io with signed checksums, and CI no longer signs. Further steps would raise the bar: a Vercel user that
  holds only the get.farhelm.io project; disconnect the docs project from Git (its automatic deploys are off only by a
  setting in agent-writable `website/vercel.json`); tag rulesets and GitHub immutable releases; lock down the DNS,
  registrar and email accounts; and review the source diff before signing, since the signature proves who published a
  release, not that its code is sound.

- **Experiment: a different allocator for the release binaries.** EXPERIMENT ONLY; nothing changes without a measured
  win. The release binaries are static musl, whose allocator hands memory back to the OS eagerly; about a quarter of the
  supervisor's CPU was the kernel's cross-CPU interrupts from those `munmap` calls, though most of the churn came from
  polling that has since been removed (#1738, #1766). Profile the supervisor again, try mimalloc or jemalloc, and keep
  it only if a before/after measurement on the same workload shows a clear CPU win worth building a C allocator for
  every release target. Baseline numbers and method: `lore/2026-10-08-supervisor-idle-cpu.md`.

- **Install Claude and Codex on a host for the user.** Offer to install Claude Code and Codex (only those two) on a host
  on the user's behalf, always through each vendor's one-line curl installer, and optionally copy the user's credentials
  for them over to the host. Choosing to copy credentials must come with a warning that it is the user's call, including
  the risk that a provider bans the account for use across hosts. Offer it in two places: as an action in the host's
  pop-up menu, and as an opt-in option (off by default) when adding a host. Relatedly, adding a host should probe it
  once Farhelm's own installation there is complete.

- **Guard the website's install command against a rogue change.** Add automated protection so that an agent cannot
  replace the `curl … | sh` install command on the docs website without the maintainer noticing. The command is what new
  users paste into a shell unread, and it appears in three places (the site header, the Install page, and the README)
  that nothing checks today. Agents land changes to main without the maintainer reviewing each one (the plans monitor
  does, for one) and can trigger a website deploy, so a changed command could reach the live site unseen. What the
  protection checks, and where (the website build, a check against the live site, an alert to the maintainer), is to be
  decided when the entry is planned.

- **Brick test a release before signing it.** Today the brick test can only run after the maintainer has signed the
  release and published it at its version path on get.farhelm.io (it does not need `/latest` moved). The signature is
  about security, not about the release being free of bugs, so needing it first is not a bug in itself. But it ties the
  two steps together, and ideally the brick test would not need a signature, so the maintainer could brick test a
  release separately from signing it. Several parts of the test lean on the signature. The installer and the helm only
  fetch releases from get.farhelm.io, which only serves signed ones. The helm checks the signature on the remote-host
  payloads it downloads. The old app's in-app updater checks it against the keys compiled into that old release. And the
  brick test's first check verifies the candidate against the previous release's keys. The trade-off is realism: every
  way around the signature (a mirror, a test-only key, skipping verification) makes the brick test exercise something
  other than what a user's upgrade actually does.

- **Require a systemd user manager on Linux, with no fallback.** Decided 2026-10-02: on Linux, a usable `systemd --user`
  is simply required, and Farhelm has no fallback for working without one; macOS has no systemd and keeps the portable
  process sweep. A Linux host whose user manager is missing, hung or broken is a broken host, like one with an I/O error
  or a full disk, not a mode Farhelm adapts to. What is left is the implementation, not the decision.

  Today there are two fallbacks. A Linux supervisor that finds no usable user manager uses the same sweep macOS uses,
  with the weaker cleanup guarantee SPEC.md's lifecycle operations section describes; a definite "no usable manager"
  answer is cached for the supervisor's lifetime, while a probe that times out is retried a minute later. And since
  #1279, a launch that meets a slow user manager still starts, just without a scope, so that session relies on the sweep
  for good while later launches check again. Both go. A launch or tab open on Linux that cannot get its scope fails
  visibly instead of starting unscoped, and a supervisor that once found the manager unusable checks again rather than
  keeping that verdict. The code saving is small, since the sweep itself is shared with macOS and only the `/proc`
  reader is Linux-specific. The real gain is that a Linux host stops flip-flopping between systemd and no systemd: scope
  use is decided per launch and per tab against a cached verdict, so one session can mix scoped and unscoped processes,
  and a stale "no usable manager" verdict can make cleanup skip a scope that does exist. That mixing breeds edge cases
  like these two, found in review-feedback triage on 2026-10-02 and folded into this entry rather than fixed separately:
  - Delete skips a tab's scope (`delete-skips-scoped-tab-on-stale-verdict.md`). An agent launched without a scope
    because systemd was briefly unreachable, a terminal tab later opened inside one, and a supervisor whose first
    systemd check after a restart failed: Delete then never checks the tab's scope and reports success, and a process
    the sweep cannot see (such as an `ssh-agent` started by the tab's shell startup files) outlives the session with
    nothing left that can find it.
  - A failed create orphans its scope (`create-rollback-orphans-unconfirmed-scope.md`). When a create fails after the
    agent started and systemd does not confirm the scope's kill, the rollback still removes the half-created session if
    the sweep came up clean, so a daemon the sweep cannot see keeps running with no session to Delete. Dropping the
    fallback does not change this by itself, because the trigger is a hung manager at rollback time. Make the rollback
    behave like every other cleanup Farhelm cannot confirm: the create fails visibly and the session is kept, so a later
    Delete can retry once the host is healthy.

  A related case was folded into this entry on 2026-10-11 for the shared rollback site and remedy, rather than scoped
  and unscoped process mixing:
  - A failed database confirmation discards cleanup ownership
    (`failed-confirmation-discard-session-despite-failed-process.md`). When a create's database confirmation fails and
    its process sweep also fails, a confirmed end of the session's terminal host still drops the session record. A
    surviving detached process then keeps running with no session left to Delete. The trigger is a local storage
    failure. Keep the session when cleanup cannot be confirmed, so a later Delete can retry.

  When executing, check that all three cases are actually gone.

  Once the fallback is gone, also assess what the process sweep still has to do on Linux (to look into, not decided,
  2026-10-05). With every launch and tab in a scope, the walk from the process numbers of a session's terminals (their
  hosts' children) has no Linux reason left, since its stated purpose is hosts without tab scopes, and dropping it there
  removes the short process-number reuse race SPEC_impl.md currently accepts as an interim measure. Whether the
  environment-marker sweep should stay as a backstop, for processes that left the scope but kept the session marker (an
  agent running `systemd-run --user --scope`, say), is part of the assessment.

- **Decide what process cleanup promises on macOS.** macOS offers Farhelm no equivalent of a cgroup scope. launchd
  tracks a job by its process group, which `setsid` escapes, and the kernel's coalitions, the closest analogue, are as
  far as we know private API (not verified). So Stop and Delete there rely on the walk down from the terminal's process
  and the environment marker, with the residuals SPEC.md's lifecycle section lists. Work out what Farhelm should promise
  and do on macOS, and how much of that machinery earns its keep: for example whether the walk from the terminals'
  process numbers is worth it, and whether the short process-number reuse races SPEC_impl.md accepted on 2026-10-05 as
  an interim measure should stay accepted for good.

- **Goose foreground ownership.** Keep the basic Goose reporter for now; it does not distinguish a native or shelled-out
  child that inherits the reporter from the foreground conversation. The stricter database-backed implementation is
  preserved at the `goose-capture-complex-2026-09-23` tag for comparison. The research and proposed smaller replacement
  are recorded in [the Goose session-tracking assessment](lore/2026-09-23-goose-session-tracking.md). Revisit only if
  reliable child isolation becomes a product requirement or Goose exposes a direct root/subagent role signal.

- **Pi foreground ownership.** Assess whether native or shelled-out Pi children can replace or withdraw the foreground
  conversation's restart target, then define the smallest admission check that preserves legitimate foreground
  transitions. This is an assessment task, not a claim that every vendor path has been reproduced.

- **OMP foreground ownership.** Assess whether native or shelled-out OMP children can replace or withdraw the foreground
  conversation's restart target beyond the checks now landed in #814, and define any remaining smallest admission check.
  Preserve legitimate foreground transitions and avoid extending the reporter's scope without evidence. The earlier
  cross-harness evidence for this and the Pi entry is preserved in
  [the historical ownership assessment](lore/2026-09-20-harness-conversation-ownership.md).

- Reconsider the first-use configuration experience for `gh:` launches when no working-copy root is configured. The
  first version refuses the launch and points to the CLI command; consider an inline GUI flow on initial use or another
  improvement that makes setup easier. Keep the general preference for CLI configuration of rarely changed settings.

- Reconsider agent parent/child relationships: either remove them or make them useful. Current parent tracking is
  optional, and fleet `agent create`/`clone` do not record the asking session, so the parent filter cannot reliably
  answer which sessions an agent created. This is largely unused complexity today; assess whether useful tracking is
  worth keeping before extending it. The current limitation is explicitly accepted in SPEC.md.

- Close the read side of what attached hosts can learn about the fleet. Acting across hosts already needs the user's
  approval (or the requesting host's "run farhelm commands without asking" setting), but any attached host can still
  read the fleet-wide session and host listings and, through an approved spawn that applies a template, any template's
  full contents including command lines, and through an approved clone any session's command line; SPEC.md's Local
  authority section accepts that only until this lands. The end state: only explicitly trusted environments may
  interrogate other hosts' session and template data. Preserve the eventual ability for agents to orchestrate sessions
  across hosts through an explicitly authorized launch policy, potentially trusted templates, without a card for every
  step.

- Make agent hook installation an explicit step surfaced to the user: tell them which hooks Farhelm installs for which
  agents, and have them accept specific hooks, so Codex launches no longer need `--dangerously-bypass-hook-trust` and
  its `-c` overrides. Today that bypass lets unreviewed hooks in the user's Codex config home and in a trusted
  workspace's `.codex/` run; SPEC_impl.md accepts it only until this step exists.

- Use the per-host yolo safe / sensitive setting (from the Near term entry "Guard against yolo launches on sensitive
  hosts") to decide which hosts appear red in the session list. The launch warnings this entry originally also proposed
  moved into that Near term entry.

- Support agents' native voice modes by piping audio through the helm to the supervisor and onward to the agent. Assess
  feasibility later, including how agents accept audio and what transport or audio-device integration would be needed;
  this entry does not commit to a design.

- Take a pre-upgrade backup of the on-disk state so a release can be rolled back. Rerunning the installer pinned to an
  older `FARHELM_VERSION` swaps the binaries back cleanly, but it does not make a downgrade work: both stores refuse to
  open a database whose `user_version` is above what the binary understands (deliberately — misreading is worse than
  refusing), so any release that bumps a schema (0.3.0 takes both the helm and the supervisor from 14 to 15) leaves the
  older binary unable to open the state it finds. Today the only rollback is a state-directory backup taken by hand
  before upgrading (the 2026-08-31 upgrade kept one next to the old binary), and remote hosts updated by the helm have
  the same problem for their own supervisor state with nobody taking a backup at all. Wanted: the upgrade paths — the
  installer for the local machine, and the helm's host update for remote ones — snapshot the state directory (a copy, or
  SQLite's backup API, taken while the old version is stopped) before the new version first opens it, keep a bounded
  number of such snapshots, and document the restore. Noted 2026-09-03 while cutting 0.3.0-rc.1.

- Consider dropping the race-proofing around host identity, keeping the identity itself. To be clear about what stays:
  the per-install identity the supervisor mints on first run and stores in its own database, independent of hostname and
  address, so a retargeted row or a state directory moved to another machine is recognized as the same install; "never
  silently merge" as a user-visible rule; and the mismatch surfaced with both identities and an adopt choice. What goes
  is the machinery that closes millisecond windows in RECORDING it: the empty-slot-only compare-and-swap in
  `record_first_contact`, the dialed-configuration check inside the same transaction (a retarget straddling a
  handshake), the separate adopt CAS with in-transaction cache purge, the never-reused connection tokens that every
  session-cache and mutation write carries, and the split of one "something is off" situation into three connection
  states (`identity-mismatch`, `duplicate`, `identity-unverified`) each with its own remedy text and re-probe policy —
  about 1,500 lines of helm implementation plus ~2,000 of tests across `store.rs`, `manager.rs`, and `hosts.rs`. The
  replacement is check-and-ask: on connect, compare the reported identity with the stored one; equal or empty means
  record and proceed, different means freeze and ask. The races it stops defending against need a retarget or a second
  first-contact to land within one handshake of another, and even then the consequence is a wrong identity the next
  connect flags as a mismatch, not a merge nobody sees. SPEC_impl.md's "structurally impossible at the storage layer"
  and "SCHEMA invariant" paragraphs under Helm internals would be rewritten to say the check is a check. Write-up:
  https://claude.ai/code/artifact/c3d3a74b-ae55-45c7-b3ca-fe30f9f97432

- Replace `install.sh`'s park/journal/rollback with plain idempotency. The floor to keep: re-running the installer from
  ANY intermediate state converges to a correct install, and no single binary is ever torn — download and extract into a
  staging directory inside `$INSTALL_DIR`, verify, then `mv` each binary into place, which is an atomic rename on one
  filesystem. Keep the outer brace group too; it is two lines and is what makes a truncated `curl | sh` execute nothing.
  What goes is the transactional layer built on top of that: the `mkdir` lock with ownership checks, the
  `PARK`/`INSTALL`/`UNDONE` journal, `rollback_from_journal`, the two rollback branches in the replacement loop, the
  `.old` parking files and the refuse-unless helpers around them — about 220 of the script's 527 logic lines (the other
  ~570 lines of the file are comments). Be honest about the size: the ~300 logic lines that remain are things any
  correct installer needs — target detection with the Rosetta case, prerequisite probing, latest-version discovery,
  `SHA256SUMS` handling, tar member validation, the `--version` cross-check, PATH advice, the closing messages — and
  `test-install-sh.sh` mostly tests THOSE (404, checksum mismatch, malformed archives, versions, prerequisites, the
  closing-message contract, the nothing-outside-`$INSTALL_DIR` diff); only the forced-failure rollback leg goes. Given
  up: a kill between the two macOS binaries' renames leaves one new and one old until the next run (the desktop shell
  finds its sibling by path, so a mismatch shows as a refusal, not silence), and a failure after placement no longer
  restores the previous binaries — re-run instead. A modest cut, worth taking when someone is in the file anyway rather
  than on its own.

- Fly.io Sprites as a host kind: a session backed by a per-second-billed microVM that freezes when idle, with "pause
  this host" in the UI meaning "stop paying for it". Assessed 2026-08-30 against a real sprite; the findings, the code
  mapping (a `HostKind::Sprite` over the existing ssh transport via the sprite CLI's ProxyCommand emulation, a
  provisioning flavor for a host with no systemd and no sftp, a `Paused` host state), the SPEC conflicts to surface, and
  a build order are in `lore/2026-08-30-fly-sprites-as-a-host-kind.md`. The same entry sizes the related "native app
  attaches to a remote helm" mode and the cheaper installed-web-app alternative.

- **Pluggable host creation.** Let users plug in their own scripts that spawn and destroy hosts for sessions to run on,
  rather than Farhelm knowing each provider itself. Related to Tensorlake hosts and the Sprites host-kind entry above,
  which build specific providers in.

- Sandboxed agents with scoped GitLab credentials, especially on Tensorlake hosts and the proposed Fly.io Sprites hosts:
  give an agent a token that reaches only the repositories it works on, minted per sandbox. GitLab rather than GitHub
  because GitLab lets the token creation be automated, while GitHub's scoped tokens have to be created by hand in a
  browser.

- Extend container sessions to macOS container hosts, help users prepare or obtain images, and consider stronger
  isolation beyond the experimental Linux rootless Podman support.

- **"Learn more" links from the GUI to the docs.** Once the official documentation lives at a stable URL, add direct
  "Learn more…" style links in various places in the GUI that point at the relevant documentation page. Which places get
  one is to be decided when this is picked up.

- **In-app intro guide.** Build an introductory guide into the app itself, so a new user can learn Farhelm's core
  concepts and first steps without leaving it. Form and content are to be decided when this is picked up.

- **Consider allowing exactly one UI attached to the helm.** A single GUI attached to the helm is the supported user
  surface, and several concurrent GUIs (browser tabs, the desktop app, other devices) are best effort. Consider going
  further and refusing a second concurrent UI outright, for simplicity: races between clients would stop being possible
  rather than merely unsupported. Part of deciding is what replaces today's multi-client machinery that SPEC.md's
  session view section specifies (one attached client per session, takeover with a displaced snapshot and take-control
  action), including how moving between devices would work, and that agents acting through fleet operations are not a UI
  and would still act concurrently. Came up in review-feedback triage on 2026-10-01.

- **Inject commands into agents from the UI.** A button such as "stop all work and restart" could send a message to a
  running agent. `farhelm agent prompt` holds the delivery path for Claude and Codex but is turned off for now (see
  "Re-assess letting agents prompt running sessions", Mid term); a UI button and delivery to other agents remain
  separate work. See SPEC.md, Agent-spawned sessions.

- **Make copying work out of the box with Codex.** Since Codex 0.157 its fullscreen interface captures the mouse, so a
  plain drag in its prompt box highlights text without copying it; the drag-copy hint plan only explains this to the
  user. Consider a launcher option to turn Codex's fullscreen mode off (`-c tui.fullscreen_transcript=false`, the
  setting `/tui` → Scrollback saves), or another way to make copying work without the user having to know Codex's copy
  key. Decided 2026-10-02 while planning the hint.

- **Whole-app zoom.** The font size plan covers terminal text only. Consider a zoom for the whole UI as well: the
  desktop app's webview has a native zoom Farhelm never uses (Cmd/Ctrl +/− do nothing there today), while on the web the
  browser's own page zoom already works but cannot be driven from a button. The rest of the UI uses about 110 hard-coded
  pixel sizes, so a CSS-based scale would be a refactor. Deferred 2026-10-02 as more work than the terminal control.

- **Fix Mac window restoration across display scales.** On a Mac whose displays use different scaling (for example a
  Retina laptop and a non-Retina monitor), the desktop app can reopen at the wrong size or position and save that wrong
  geometry again on close. The saved size is converted with the new window's scale and the position through the current
  scale (`crates/farhelm-ui/src/desktop/window_state.rs`). SPEC_impl.md's desktop window-persistence paragraph already
  says a real macOS display check is still needed. This needs work and verification on a real Mac with mixed-scale
  displays; do not fix it blind. Deferred in review-feedback triage on 2026-10-11
  (`window-restoration-uses-wrong-display-scale-macos.md`).

- **Let the Mac's supervisor outlive the desktop app.** The local supervisor now keeps running after the app quits, but
  the helm lives in the app, so until the app is reopened the agents' `farhelm` commands that need it (`farhelm spawn`,
  and the `farhelm agent` verbs) fail inside those sessions, and nothing restarts a supervisor that crashes while the
  app is closed or starts one at login. Consider a launchd agent like the Linux systemd units, so that those commands,
  and the skills and command-line features built on them, keep working with the app closed. Deferred 2026-10-04 while
  planning `plans/queue/hook-report-files.md`, which keeps conversation tracking working without a supervisor and states
  this gap in the spec as expected for now.

- **Keep a nested Grok from erasing the session's conversation switch.** A `grok` started inside a Grok session can make
  Restart resume the wrong conversation. Here's the sequence. You are in conversation A in a Grok session and run
  `/new`, which starts conversation B, and Grok's hook saves a report saying "this session is now in B". Before Farhelm
  reads that report, the session's Grok starts another `grok` (through its shell tool, say). That second `grok` runs
  your configured Farhelm hooks too, because Grok's hooks live in its configuration rather than on the launch's command
  line, and its hook saves a report for the same session that replaces the first one. Farhelm then reads the second
  report, sees that it comes from a `grok` nested below the session's own, and refuses it, which is correct, but B's
  report is already gone. The session stays on A, and Restart resumes A although you were in B. In a brand-new session
  whose first report was the one replaced, nothing is recorded, and Restart stays unavailable. Farhelm never records the
  nested `grok`'s conversation; the failure is a stale or missing conversation, never a wrong one.

  The window is short while the supervisor runs, since it reads reports every two seconds, but covers the whole outage
  while it does not (on the Mac, while the app is closed). It exists because each session keeps only its newest unread
  report (Grok keeps two: the one naming the conversation and the newest per-prompt one), and the check that refuses a
  nested report runs when Farhelm reads the file, not when the hook writes it. Reporting over the supervisor's socket,
  before #1594, checked each report as it arrived, so a refused report could never displace a good one. Claude, Codex,
  Pi and OMP are not affected: Farhelm adds their reporter to the one launch it builds, so a copy of them started inside
  a session reports nothing. Unverified: whether B's later per-prompt reports correct the session on their own; they
  only add detail to a conversation already named, so the session may stay on A until the next `/new` or a restart.
  Possible fixes: keep every report as its own file and apply them in order, so a refused report never replaces another
  (the plan ruled out only a capped queue, which would evict reports during a long outage); keep a slot per reporting
  process; or have the hook check its own process chain before writing. The accepted gap is recorded in SPEC_impl.md's
  "Report files". Found 2026-10-05 reviewing the `hook-report-files` plan's report.

- **Investigate harness-native notifications.** Claude Code and Codex can raise desktop notifications themselves, by
  writing terminal escape codes that a supporting terminal turns into a system notification. Claude Code picks the
  channel with its `preferredNotifChannel` setting (in `/config`; values such as `iterm2`, `ghostty`, `kitty` or
  `terminal_bell`) and fires when it needs the user: a permission prompt, or sitting idle waiting for input. Codex does
  it with `notifications = true` under `[tui]` in `~/.codex/config.toml`, and fires when a turn finishes and on approval
  requests. Inside Farhelm those codes reach the supervisor's own terminal emulator, which does nothing with them, so
  they end in Farhelm's terminal rather than a native terminal app. Investigate what Farhelm could do with them: surface
  them as client notifications, use them as a status signal, or nothing. Related to the ntfy.sh entry under Near term.

- **"Phone home" supervisors.** Allow supervisors to come up and register themselves with the helm, instead of being set
  up or pushed from the helm, so hosts show up dynamically as they come and go.

- **Tagging/capabilities for hosts and sessions.** Add tagging/capabilities so an agent can say "do X on a session on a
  host with capabilities Y" rather than naming specific hosts or sessions.

- **Trust zones.** A trust zone is scoped to a project and carries certain credentials; a session in a trust zone may
  spawn other sessions in the same zone, on hosts that allow that zone.

- **Consider a "project" concept that is not a session or a repo.** A project would be an object that lives across
  sessions and can span repositories, owning things that belong to the work rather than to any one session or checkout:
  its own identity, context, skills, a favicon, memory, and similar. Whether Farhelm has such a concept, what it owns,
  and how it is created and selected are all TBD.

- **Make the session list scale past its 500-session ceiling.** Each host lists at most 500 sessions and the helm's list
  across hosts at most 500, by design (SPEC.md, "The list is served and rendered WHOLE"), and the `session-limit-notice`
  plan only tells the user through the rail bell when a ceiling is hit. If people do run that many sessions, make the
  list scale instead. Recorded by the maintainer on 2026-11-18 while planning that notice.

## Unbucketized
