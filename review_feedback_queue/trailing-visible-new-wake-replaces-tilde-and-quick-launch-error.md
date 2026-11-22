# Visible New wake silently replaces displayed ~ with a remembered folder and shows quick-launch wording on failure

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

When you open New manually on a suspended or stopped host without editing the Folder field, the dialog displays `~` with
no warning that it will change; pressing Launch wakes the host and silently launches your session in whatever remembered
folder was last used on that host instead of `~` — or, if reading the host's history fails, reopens New with a "Couldn't
prepare quick launch: ..." message even though you did not use a quick launch.

## Details

**high.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/request.rs:268-273`.

Bucket note: placed in `high` for silent destination substitution and quick-launch-only error wording; all three
reviewers surfaced that this borders `highest` because `~` is excluded from Claude workspace auto-trust whereas silently
switching to a remembered repository enables auto-trust or runs a YOLO/command launch in unintended files.

Expanded from Turn-1 `f4` after cross-group check with `g11` (`f2`):

1. In `crates/gui/farhelm-gui/src/launcher/request.rs:268-273`, `LaunchDraft::draft_refusal` exempts `can_wake(host)`
   from `FOLDER_DEFAULT_WAITING` unconditionally — not only for hidden quick launches (`quick_launch_unseen`), but also
   for a visible New dialog where `draft.folder_default == FolderDefault::Pending` and the Folder field literally
   displays `~` with no notice that the directory is pending or will be replaced after wake (contrast managed checkout's
   `checkout_allocation_notice`, SPEC.md:1832).
2. On pressing Launch, `begin_host_wake` (`view.rs:2825-2827`) tries `settle_folder_default` (returns `false` when no
   open session is on that host and history is absent) and starts the wake with `folder_default` still `Pending`.
3. Once the host connects, `Launcher::continue_after_wake` (`view.rs:2847-2849`) calls
   `self.start_or_wait_for_folder(mark, window, cx)` (`view.rs:389-419`), which reads
   `GET /api/launch-history?host=<id>`, calls `settle_folder_default` (`view/history.rs:280-310`) to overwrite `~` with
   `remembered_folder(history)`, and immediately submits the create into that unshown directory — violating SPEC_impl.md
   ("Questions: the prompt": actions act on what the user was shown).
4. If the `GET /api/launch-history` read fails, `start_or_wait_for_folder` (`view.rs:411-417`) unconditionally checks
   `self.draft.folder_default_unread()` (without checking `self.quick_launch_unseen`) and sets
   `self.note = Some(Note::Refusal(QUICK_LAUNCH_FOLDER_UNREAD))` ("Couldn't prepare quick launch: this host's recent
   setups could not be read, so its last folder is unknown; check the folder and launch"), showing quick-launch-only
   wording below the search field (SPEC.md:1562-1564) for an ordinary New launch. Fix direction: For a visible New
   dialog (`!quick_launch_unseen`) on a resumable host with `FolderDefault::Pending`, either treat the user's Launch
   press on the displayed `~` as launching into `~` (settling `folder_default` to `~` at press time, or explicitly
   telling the user in the Folder area before press that the host's last folder will be used after wake), and ensure
   non-quick-launch history failures never display `QUICK_LAUNCH_FOLDER_UNREAD`.

Reviewer metadata (g10-wake-launch-state-and-requests/f4): suggested bucket: high (could arguably cross into highest
when workspace auto-trust is enabled or on Tensorlake sandboxes where YOLO is allowed without asking by default: `~` is
excluded from auto-trust per SPEC.md:1782, whereas silently substituting an unshown remembered repository directory
after wake starts the agent in a different directory than drawn and enables auto-trust / repo hooks there; bucketed
`high` as the established render-binding / silent-destination-substitution defect, surfacing the workspace-trust / YOLO
boundary for triage); confidence: confirmed; possible cover: none (cross-group overlap with g11 finding f2).

Additional source record at `66ad3aaa95ef03a2130e2d0194d58353912ae9da`,
`crates/gui/farhelm-gui/src/launcher/view.rs:2847` (g11-wake-launch-views-and-panes/f2):

In `crates/gui/farhelm-gui/src/launcher/draft.rs:1016-1022`, picking a suspended host in New (with no open session on
that host) sets `folder` to untouched `~` and `folder_default = FolderDefault::Pending`. Because the host is suspended,
`history_key(cx)` (`crates/gui/farhelm-gui/src/launcher/view/history.rs:79-90`) is `None`, so history is not read while
the dialog is open. `request.rs:269-270` exempts `can_wake(host)` from `FOLDER_DEFAULT_WAITING`, so Launch is enabled
while the visible Folder input displays `~` and Launch's accessible name (`view.rs:1769-1784`) names `~` (with no notice
like managed checkout's "picked automatically when the host is ready"). When the user presses Launch, `begin_host_wake`
(`view.rs:2825-2827`) cannot settle `folder_default` yet and starts the wake. Once the host connects,
`continue_after_wake` (`view.rs:2847-2849`) checks `if self.draft.folder_default_waiting()` and calls
`self.start_or_wait_for_folder(mark, window, cx)` (`view.rs:389-419`), which was designed for unseen quick launches:

1. If `GET /api/launch-history` returns a remembered plain folder, `settle_folder_default` (`history.rs:280-310`)
   silently overwrites `~` with that remembered folder and immediately creates the session there, violating the
   render-binding rule (`SPEC_impl.md`, "Questions: the prompt", lines 993-997) because the user pressed Launch on a
   visible form displaying `~`.
2. If `GET /api/launch-history` fails, `settle_folder_default` sets `FolderDefault::Unread` (`draft.rs:1050`).
   `start_or_wait_for_folder` (`view.rs:411-417`) does not check `self.quick_launch_unseen`, so it aborts the visible
   New launch and reopens the dialog with `Note::Refusal(QUICK_LAUNCH_FOLDER_UNREAD)` ("Couldn't prepare quick launch:
   this host's recent setups could not be read, so its last folder is unknown; check the folder and launch") below the
   search box — directly contradicting `draft.rs:436-438` ("A visible launcher launches into it as any `~`; a quick
   launch prepared out of sight shows New instead") and `SPEC.md:1559-1564`. Fix direction: Distinguish a visible
   launcher (`!self.quick_launch_unseen`) from an unseen quick launch: either settle a visible New press onto the `~`
   that was drawn before waking (so the launched folder matches what the user saw and never hits
   `QUICK_LAUNCH_FOLDER_UNREAD`), or — if resolving the host's remembered folder after wake is wanted for visible New —
   show an explicit allocation hint in place of `~` and guard `QUICK_LAUNCH_FOLDER_UNREAD` with
   `self.quick_launch_unseen` (falling back to `~` or a normal `WakeRefusal` for visible launches).

Reviewer metadata (g11-wake-launch-views-and-panes/f2): suggested bucket: high (could arguably border highest if an
agent with YOLO or workspace trust starts in an unexpected remembered repository directory instead of `~`, though no
destructive action is directly triggered by Farhelm itself; surfacing this boundary explicitly); confidence: confirmed;
possible cover: none.

Additional source record at `9ece1d1f889a47c5dd6bd82b5f384c8fd5e87e75`,
`crates/gui/farhelm-gui/src/launcher/wake_tests.rs:32-43` (g12-wake-folder-launch-tests/f3):

Verified against 9ece1d1f889a47c5dd6bd82b5f384c8fd5e87e75 and pinned 66ad3aaa95ef03a2130e2d0194d58353912ae9da
(cross-checked with g11 finding f2).

1. Test gap in g12: Every visible New test in crates/gui/farhelm-gui/src/launcher/wake_tests.rs goes through open_on
   (lines 32-43), which calls draft.choose_host(host.id, "~".into()) and then unconditionally calls
   draft.edit_folder("/home/user/work".into()) (line 40), marking the folder touched and clearing
   FolderDefault::Pending. crates/gui/farhelm-gui/src/launcher/request_tests.rs:133-166
   (a_resumable_environment_allows_the_press_before_it_can_create) starts from draft() (LaunchDraft::new, which
   initializes FolderDefault::Settled at draft.rs:473) and mutates draft.host = Some(2) directly without choose_host, so
   it never tests request.rs:270's can_wake bypass of FOLDER_DEFAULT_WAITING.
   crates/gui/farhelm-gui/src/goldens/launcher.rs:69 also calls edit_folder.
   crates/gui/farhelm-gui/src/templates/dispatch_tests.rs:806-866 only tests hidden quick launches (where deferring the
   folder default to post-wake history is intended), never a visible New launcher.
2. Shipped defect masked by this gap: request.rs:268-273 allows a visible New draft with FolderDefault::Pending
   (displaying '~~') to press Launch on a resumable host. After wake connects, Launcher::continue_after_wake
   (crates/gui/farhelm-gui/src/launcher/view.rs:2847-2849) checks `if self.draft.folder_default_waiting()` and routes
   the visible launch into start_or_wait_for_folder (view.rs:389-419), which reads GET /api/launch-history and either
   (a) silently replaces the displayed '~~' with the host's remembered folder (violating render-binding / SPEC.md "a
   refresh of the recent setups never rewrites an open draft's folder") or (b) on history failure aborts the manual
   launch with QUICK_LAUNCH_FOLDER_UNREAD ("Couldn't prepare quick launch: this host's recent setups could not be read,
   so its last folder is unknown; check the folder and launch"). Fix direction: In a visible launcher, settle or bind
   the displayed folder at press time (or refuse before wake if an unseen history default is required rather than
   silently swapping '~' post-wake, and never emit QUICK_LAUNCH_FOLDER_UNREAD for a non-quick-launch submit); add
   unit/window tests in request_tests.rs and wake_tests.rs for a visible New draft with FolderDefault::Pending on a
   resumable host.

Reviewer metadata (g12-wake-folder-launch-tests/f3): suggested bucket: high (could be in highest — surface boundary to
user: a command or YOLO agent the user confirmed in the visible New dialog against the displayed '~' working directory
silently runs in a different remembered directory from host history, which could act on unintended files; route is
established in shipped code); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g10-wake-launch-state-and-requests/f4`,
`g11-wake-launch-views-and-panes/f2`, `g12-wake-folder-launch-tests/f3`.
