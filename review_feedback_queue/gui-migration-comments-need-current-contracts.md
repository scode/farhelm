# GUI comments still describe migration assignments

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Comments across the desktop and GUI still describe temporary assignments, milestones, and unfinished ports. Several now
contradict behavior already in the app.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

### Desktop startup and updates

#### Point the reopen and updater comments to the maintained contract

The comments point to numbered sections of an unnamed design. Cite the named sections in `SPEC_impl.md` on side-by-side
versions and desktop updates, keeping the reopen-marker and installation-lock rationale in place.

Reviewed locations: `crates/farhelm-desktop/src/bundle.rs:52-54` and :159-161,
`crates/farhelm-desktop/src/lib.rs:1284-1288`, `crates/farhelm-desktop/src/updater.rs:141-146`, :1069-1072 and
:1865-1867. Haiku out-P1.md §2. Suggested bucket **other**.

#### Keep the platform behavior and drop migration setup details

These comments mix the current platform contract with coordination, prototype provenance, or the setup of a private test
run. Keep the useful behavior, including the absent motion preference and intermediate zoom reports, and remove the
project and machine details.

Reviewed locations: `crates/farhelm-desktop/src/lib.rs:300-304`, `crates/farhelm-desktop/src/second_launch.rs:8-17`,
`crates/farhelm-desktop/src/sound.rs:6-7` and :20-21, `crates/farhelm-desktop/src/tab_reach.rs:3-7`,
`crates/farhelm-desktop/src/reduced_motion.rs:13-15`, `crates/farhelm-desktop/src/window_state.rs:356-360` and :938-941.
Haiku out-P1.md §2. Suggested bucket **other**.

#### Document the Linux Help entry for Open-source notices

The licenses module says Linux only has `--licenses` until the Help item arrives. Document the shipped Help item and its
action dispatch.

Reviewed locations: `crates/gui/farhelm-gui/src/desktop/licenses.rs:21-24`,
`crates/gui/farhelm-gui/src/help_menu.rs:407-418`. Haiku out-P2.md §2. Suggested bucket **other**.

### Terminal client and terminal

#### Keep the reconnect policies in the terminal client comments

The comments describe transient 5xx retries, seat handoff, and the two reconnect cadences through dated instructions
from the migration’s orchestrator and terminal area. Keep the policies and their reasons in terms of the helm, terminal
and active client.

Reviewed locations: `crates/farhelm-helm-client/src/terminal.rs:18-19`, :29-30, :381 and :395-397,
`crates/farhelm-helm-client/src/reconnect.rs:14-17`. Haiku out-P1.md §2. Suggested bucket **other**.

#### Terminal comments still use first-milestone scope

Document what the attached terminal owns today and what Reconnect now retries. Keep the explanation of the one-frame
measurement delay and describe the disclosure panel's bounds in terms of window content.

Reviewed locations: `crates/gui/farhelm-gui/src/terminal/attached.rs:3-12`,
`crates/gui/farhelm-gui/src/terminal/attached.rs:353-357`, `crates/gui/farhelm-gui/src/terminal/attached.rs:1219-1220`,
`crates/gui/farhelm-gui/src/terminal/attached.rs:1804-1806`, `crates/gui/farhelm-gui/src/terminal/set.rs:23-26`,
`crates/gui/farhelm-gui/src/terminal/text_size.rs:9-10`, `crates/gui/farhelm-gui/src/terminal/disclosure.rs:41-45`.
Haiku out-P8.md §2. Suggested bucket **other**.

The comment at `crates/gui/farhelm-gui/src/terminal/disclosure.rs:41–45` also records an implementation gap. Preserve
that caveat while the full overlay-visibility requirement in SPEC.md:3145–3148 and :3189–3192 remains unmet.

#### The terminal-client test header rules out waits that the suite uses

The header says the suite never waits on real time, but the watchdog and silence cases deliberately do. Describe
event-driven readiness, bounded waits and the cases that deliberately observe a period of silence.

Reviewed locations: `crates/farhelm-helm-client/src/terminal/tests.rs:1-6,685-686,736-737,1122-1123`. Haiku out-T1.md
§3. Suggested bucket **other**.

#### Reconnect test comments attribute current rules to migration coordination

Remove the orchestrator attributions. Keep the reasons the tests already give for retrying a helm's 5xx, preserving a
stalled screen and leaving the other tabs alone, as well as GPUI's test-platform limit for native window actions.

Reviewed locations: `crates/farhelm-helm-client/src/terminal/tests/ladder.rs:100-105`,
`crates/gui/farhelm-gui/src/terminal/attached/tests.rs:1000,2842-2843,3725`,
`crates/gui/farhelm-gui/src/terminal/set/tests.rs:624-630`. Haiku out-T1.md §4, out-T8.md §4. Suggested bucket
**other**.

### Shared controls, focus and overlays

#### Remove work-area ownership from the component contracts

Dated migration approvals and work-area ownership interrupt current contracts such as which module owns the window
chrome and how the keyboard controls behave. Rewrite those passages around the implementation responsibilities and the
behavior they protect.

Reviewed locations: `crates/gui/farhelm-gui/src/actions.rs:265-267`, :377-379 and :692-693,
`crates/gui/farhelm-gui/src/bootstrap.rs:27-30` and :255-256, `crates/gui/farhelm-gui/src/cache.rs:21-22`,
`crates/gui/farhelm-gui/src/assets.rs:9-13`, `crates/gui/farhelm-gui/src/components/button.rs:619-621` and :843-845,
`crates/gui/farhelm-gui/src/components/select.rs:46-51`, `crates/gui/farhelm-gui/src/components/menu_header.rs:1-5`,
`crates/gui/farhelm-gui/src/desktop.rs:6-19`, `crates/gui/farhelm-gui/src/desktop/linux_title_bar.rs:8-9` and :22-25,
`crates/gui/farhelm-gui/src/desktop/menu.rs:25-26`, `crates/gui/farhelm-gui/src/focus.rs:72-76` and :1773-1775. Haiku
out-P2.md §2, out-P3.md §2. Suggested bucket **other**.

#### Replace the obsolete popover menu note

The layer docs defer real menu items and their roles to a later port. Describe the current split between the popover
surface and the menu controls already used by session rows.

Reviewed locations: `crates/gui/farhelm-gui/src/layers.rs:124-127` and :2538-2540,
`crates/gui/farhelm-gui/src/components/menu_item.rs:65`. Haiku out-P5.md §6. Suggested bucket **other**.

#### Describe the overlay and operation rules directly

These comments cite work-area approvals or maintenance rules from the migration to explain current stacking, keyboard,
and operation behavior. Keep the concrete reasoning, such as keeping approval cards reachable and allowing each kind of
operation to classify itself, and cite the maintained specifications where they carry the rule.

Reviewed locations: `crates/gui/farhelm-gui/src/layers.rs:145-150`, :560-565, :678-680, :1783-1789, :1887-1890 and
:2257-2262, `crates/gui/farhelm-gui/src/operations.rs:52-54`, :222-226, :229-234, :268-273 and :734-740,
`crates/gui/farhelm-gui/src/prompt.rs:60-67`, :104-107, :123-129 and :396-403. Haiku out-P5.md §2. Suggested bucket
**other**.

#### Store and ticker comments use migration roles and a scan label

Keep the store's cache lifetime rule and the measured frame counts. Remove the foundation-owner attribution and the
unexplained TE-P2 label.

Reviewed locations: `crates/gui/farhelm-gui/src/store.rs:13-16`, `crates/gui/farhelm-gui/src/ticker.rs:55-59`. Haiku
out-P8.md §2, out-P9.md §2. Suggested bucket **other**.

### Hosts and launch workflows

#### Document the existing SSH help and Resume actions

The Hosts docs describe SSH help as waiting for the Help menu, and the attention docs say Hosts lacks Resume. Update
them to point to the shipped Help item and the Hosts menu’s Resume offer when the host phase permits it.

Reviewed locations: `crates/gui/farhelm-gui/src/hosts.rs:29-31`, `crates/gui/farhelm-gui/src/hosts/ssh_help.rs:27`,
`crates/gui/farhelm-gui/src/session_list/attention.rs:84-85`, `crates/gui/farhelm-gui/src/help_menu.rs:401-403`,
`crates/gui/farhelm-gui/src/hosts/menu.rs:338-357`. Haiku out-P3.md §2, out-P4.md §2, out-P5.md §6. Suggested bucket
**other**.

#### Remove migration ownership from host-operation rules

The comments justify current behavior with the settings-and-hosts goal, M2.9, or an orchestrator ruling. State the
actual contracts, such as what counts as a conflicting registry change and how long a panel retains progress.

Reviewed locations: `crates/gui/farhelm-gui/src/hosts/offers.rs:7-18` and :445-448,
`crates/gui/farhelm-gui/src/hosts/add.rs:35-38`, `crates/gui/farhelm-gui/src/hosts.rs:106-111` and :1040-1068,
`crates/gui/farhelm-gui/src/hosts/progress.rs:30-34`, `crates/gui/farhelm-gui/src/hosts/remedies.rs:31-35`,
`crates/gui/farhelm-gui/src/hosts/remove.rs:31-36`, `crates/gui/farhelm-gui/src/hosts/update.rs:23-27`,
`crates/gui/farhelm-gui/src/hosts/setup.rs:53-57`, `crates/gui/farhelm-gui/src/hosts/settings.rs:237-240`,
`crates/gui/farhelm-gui/src/hosts/settings/appearance.rs:13`,
`crates/gui/farhelm-gui/src/hosts/progress/card.rs:597-599`. Haiku out-P3.md §2, out-P4.md §2. Suggested bucket
**other**.

#### Update the launcher’s implementation map

The module map still describes template equality rules and recent-folder seeding as upcoming, and gives “the rest” of
managed checkouts as an unspecified future change. Point to the implemented template and history handling and name any
concrete remaining gap.

Reviewed locations: `crates/gui/farhelm-gui/src/launcher.rs:5` and :28-40,
`crates/gui/farhelm-gui/src/launcher/view.rs:39-46`, `crates/gui/farhelm-gui/src/launcher/view/host_picker.rs:21-23`,
`crates/gui/farhelm-gui/src/launcher/view/history.rs:193-222`, `crates/gui/farhelm-gui/src/templates/apply.rs:302` and
:368. Haiku out-P4.md §2, out-P5.md §6. Suggested bucket **other**.

#### Remove launch migration work-area references

These comments cite a launcher work area, a migration gate review, or an unnamed work item as context for current
behavior. Keep the actual rules for picking recipes, cloning across hosts, and handling the sandbox confirmation.

Reviewed locations: `crates/gui/farhelm-gui/src/environment_ui.rs:319-320` and :752-754,
`crates/gui/farhelm-gui/src/launcher/entry.rs:32-34`, :71-75, :421 and :427-428,
`crates/gui/farhelm-gui/src/launcher/request.rs:15-16`, `crates/gui/farhelm-gui/src/launcher/view/recents.rs:53-54`,
`crates/gui/farhelm-gui/src/launcher/view/sandbox_question.rs:28-29`, `crates/gui/farhelm-gui/src/permission.rs:10-15`.
Haiku out-P3.md §2, out-P4.md §2, out-P5.md §2. Suggested bucket **other**.

#### Host removal's test describes the intended refusal as unfinished work

The comment says the quick-removal refusal dialog is “not ported yet,” while the test expressly requires the refusal
under the host row and no dialog. State that current behavior directly.

Reviewed locations: `crates/gui/farhelm-gui/src/hosts/remove/tests.rs:489-502`. Haiku out-T2.md §4. Suggested bucket
**other**.

#### Host-operation tests cite a migration goal for the moved-on rule

Replace the opaque “goal” attribution with the Keyboard focus rule that a late result shows where it belongs once the
user has moved on. Keep each case's concrete destination for the result.

Reviewed locations: `crates/gui/farhelm-gui/src/hosts/remedies/tests.rs:305-307`,
`crates/gui/farhelm-gui/src/hosts/setup/tests.rs:637-640,782-786`, `SPEC.md:3266-3276`. Haiku out-T2.md §4. Suggested
bucket **other**.

#### Launcher tests retain ownership labels from the migration

The rules for host-probe exclusion, Clone's placement and sandbox consent are already described in the comments. Remove
the “launcher area,” orchestrator and gate-review attributions while keeping the behavior and the reason each case
matters.

Reviewed locations: `crates/gui/farhelm-gui/src/launcher/entry_tests.rs:341-343,524`,
`crates/gui/farhelm-gui/src/launcher/sandbox_view_tests.rs:86-91,383-388,415-419`. Haiku out-T3.md §4. Suggested bucket
**other**.

### Session list and pane

#### Correct the session-list focus note and GUI module inventory

The list still says New cannot be home when the fleet is empty, and the crate docs say no sound player is installed and
the rest of actions and keys are coming. Describe the session pane’s current home selection, the platform player
installed at startup, and the modules as they exist.

Reviewed locations: `crates/gui/farhelm-gui/src/session_list.rs:150-152` and :434-437,
`crates/gui/farhelm-gui/src/session_pane/card.rs:120-129`, `crates/gui/farhelm-gui/src/lib.rs:7`, :53-55 and :67,
`crates/gui/farhelm-gui/src/bootstrap.rs:333`. Haiku out-P5.md §2, §6. Suggested bucket **other**.

#### Session pane comments describe the migration handoffs

Strip the orchestrator and work-area attributions and describe when Restart and Suspend hold the reconnect card back,
which actions return to Sessions, and when the trust band refreshes. The stale band already exists, so remove “a later
piece.”

Reviewed locations: `crates/gui/farhelm-gui/src/session_pane.rs:50-62`,
`crates/gui/farhelm-gui/src/session_pane/card.rs:55-59`, `crates/gui/farhelm-gui/src/session_pane/header.rs:44-47`,
`crates/gui/farhelm-gui/src/session_pane/details.rs:24-26`,
`crates/gui/farhelm-gui/src/session_pane/trust_band.rs:8-10`,
`crates/gui/farhelm-gui/src/session_pane/trust_band.rs:21-25`. Haiku out-P6.md §2, out-P7.md §2. Suggested bucket
**other**.

#### Session list comments describe the migration's review decisions

Replace references to the orchestrator, the milestone review, and foundation ownership with the current rules and their
reasons. Keep the rationale for the Delete notice surviving its row and for the bulk queue stopping after a timed-out
environment delete. Keep the source’s explanation of the separate 504 outcomes while the implementation distinguishes
them only by status.

Reviewed locations: `crates/gui/farhelm-gui/src/session_list/bulk.rs:81-87`,
`crates/gui/farhelm-gui/src/session_list/bulk.rs:143-147`, `crates/gui/farhelm-gui/src/session_list/bulk.rs:708-712`,
`crates/gui/farhelm-gui/src/session_list/delete_notice.rs:37-53`,
`crates/gui/farhelm-gui/src/session_list/delete_notice.rs:59-63`,
`crates/gui/farhelm-gui/src/session_list/outcome.rs:295-298`,
`crates/gui/farhelm-gui/src/session_list/queued_preference.rs:9-16`,
`crates/gui/farhelm-gui/src/session_list/filters.rs:531-535`, `crates/gui/farhelm-gui/src/session_list/order.rs:19-26`,
`crates/gui/farhelm-gui/src/session_list/selection.rs:62-66`,
`crates/gui/farhelm-gui/src/session_list/host_rail.rs:18-20`, `crates/gui/farhelm-gui/src/session_list/menu.rs:620-625`,
`crates/gui/farhelm-gui/src/session_list/row_marks.rs:82-84`. Haiku out-P6.md §2. Suggested bucket **other**.

#### Environment comments give outdated port status

Hosts already offers Delete sandbox, and the rail still renders a dot for a changing sandbox even though Suspend and
Resume are implemented. Remove the work-area references and describe the current menu and rail behavior.

Reviewed locations: `crates/gui/farhelm-gui/src/session_list/environment_delete.rs:54-58`,
`crates/gui/farhelm-gui/src/session_list/host_rail/status.rs:26`. Haiku out-P6.md §2. Suggested bucket **other**.

#### Session-pane fixtures refer to a future migration area

The module and fixture docs describe the session filter as work belonging to a later area. Describe the synthetic field
as the fixture for proving that a newly live terminal leaves focus where the user is typing.

Reviewed locations: `crates/gui/farhelm-gui/src/session_pane/tests.rs:5-10,335-336`. Haiku out-T6.md §4. Suggested
bucket **other**.

### Settings and sidebar

#### Settings comments still refer to team decisions and goals

State the rules for shared settings, reads on opening, and busy writes directly. The reason for a single ntfy draft is
that Save submits the whole form, so keep that and drop the work-area decision and date.

Reviewed locations: `crates/gui/farhelm-gui/src/settings.rs:58-63`, `crates/gui/farhelm-gui/src/settings.rs:182-185`,
`crates/gui/farhelm-gui/src/settings/choices.rs:5-9`, `crates/gui/farhelm-gui/src/settings/choices.rs:307-310`,
`crates/gui/farhelm-gui/src/settings/choices.rs:389-392`, `crates/gui/farhelm-gui/src/settings/agent_trust.rs:11-13`,
`crates/gui/farhelm-gui/src/settings/agent_trust.rs:19-22`,
`crates/gui/farhelm-gui/src/settings/checkout_field.rs:13-15`, `crates/gui/farhelm-gui/src/settings/ntfy.rs:17-20`,
`crates/gui/farhelm-gui/src/settings/ntfy.rs:28-31`, `crates/gui/farhelm-gui/src/settings/refused.rs:9-13`,
`crates/gui/farhelm-gui/src/settings/refused.rs:19-23`, `crates/gui/farhelm-gui/src/sidebar.rs:26-28`,
`crates/gui/farhelm-gui/src/sounds/owner.rs:3-5`. Haiku out-P7.md §2. Suggested bucket **other**.

#### Recipe and sandbox comments still describe migration assignments

Describe how recipe data refreshes, when freshness is rechecked, what a cleanup timeout means, and where sandbox
failures show. Remove the dates, work-area names, and instructions to the porting team.

Reviewed locations: `crates/gui/farhelm-gui/src/settings/recipes.rs:17-20`,
`crates/gui/farhelm-gui/src/settings/recipes.rs:65-70`,
`crates/gui/farhelm-gui/src/settings/recipes/editor/leaving.rs:33-43`,
`crates/gui/farhelm-gui/src/settings/recipes/places.rs:13-16`,
`crates/gui/farhelm-gui/src/settings/tensorlake_sandboxes.rs:15-18`. Haiku out-P7.md §2. Suggested bucket **other**.

#### Sidebar comments still say the update readout is pending

The sidebar already mounts the desktop update readout when available and otherwise draws the plain client build.
Describe the two branches and point to the readout for the helm build comparison.

Reviewed locations: `crates/gui/farhelm-gui/src/sidebar.rs:61-64`, `crates/gui/farhelm-gui/src/sidebar/header.rs:15-19`,
`crates/gui/farhelm-gui/src/sidebar/header.rs:90-100`, `crates/gui/farhelm-gui/src/update_readout.rs:301-317`. Haiku
out-P7.md §2. Suggested bucket **other**.

### Templates and shortcuts

#### Window shortcuts still describe their scope as a page

Use “window” for the quick switcher's scope and “main view” for dispatch outside a keyboard-taking layer. The
distinction matters when a dialog or menu owns the keyboard.

Reviewed locations: `crates/gui/farhelm-gui/src/session_list/quick_switcher.rs:8-10`,
`crates/gui/farhelm-gui/src/templates/dispatch.rs:14-16`, `crates/gui/farhelm-gui/src/templates/dispatch.rs:29-44`,
`crates/gui/farhelm-gui/src/templates/dispatch.rs:125-131`, `crates/gui/farhelm-gui/src/templates/dispatch.rs:387-395`,
`crates/gui/farhelm-gui/src/templates/armed.rs:229-233`. Haiku out-P6.md §2, out-P8.md §2. Suggested bucket **other**.

#### Template module comments still describe work in progress

Introduce the module map as the current implementation and refer to the draft field by its actual module or type.

Reviewed locations: `crates/gui/farhelm-gui/src/templates.rs:4-6`,
`crates/gui/farhelm-gui/src/templates/editor.rs:20-21`. Haiku out-P8.md §2. Suggested bucket **other**.

#### Template shortcut tests call their main-window cases “the page”

The section title, test names and cross-reference use “page” for shortcuts in the main window outside the launcher. Name
that surface directly and keep the rule that a hidden preparation reserves the launch slot without blocking other window
interaction.

Reviewed locations: `crates/gui/farhelm-gui/src/templates/launcher_modes_tests.rs:9-12`,
`crates/gui/farhelm-gui/src/templates/dispatch_tests.rs:369-372,630-638,703,2038,2077`. Haiku out-T7.md §4. Suggested
bucket **other**.

#### The template-menu test describes completed test coverage as work to port

The comment calls the case something a foundations parity row “leaves to port,” but the test now exercises removal of
the open menu's own template. Describe that case and point directly to “Things changing underneath” in the spec.

Reviewed locations: `crates/gui/farhelm-gui/src/templates/dialog_tests.rs:1037-1042`. Haiku out-T7.md §4. Suggested
bucket **other**.

### Notices, Trash, theme and welcome

#### Update the notice center’s inventory

The notice center describes the rail panel as still to come. Explain the current distinction between notices owned by
the window and the rail’s pushed records, which the rail already draws.

Reviewed locations: `crates/gui/farhelm-gui/src/notices.rs:9-13`, `crates/gui/farhelm-gui/src/rail_notices.rs:14-18`,
`crates/gui/farhelm-gui/src/lib.rs:62-65`. Haiku out-P5.md §2. Suggested bucket **other**.

#### Trash, theme, update, and welcome comments cite their porting coordinators

Remove the milestone attribution and review-gate instructions while keeping the rationale for trash rereads and raw
color ownership. State what the archive cue actually does now and which layer capability the outline animation would
need.

Reviewed locations: `crates/gui/farhelm-gui/src/trash.rs:315-319`, `crates/gui/farhelm-gui/src/trash.rs:612-617`,
`crates/gui/farhelm-gui/src/trash/listing.rs:14-18`, `crates/gui/farhelm-gui/src/trash/cue.rs:20-24`,
`crates/gui/farhelm-gui/src/theme.rs:12-15`, `crates/gui/farhelm-gui/src/theme.rs:682-684`,
`crates/gui/farhelm-gui/src/update_readout.rs:6-10`, `crates/gui/farhelm-gui/src/welcome.rs:33-37`. Haiku out-P9.md §2.
Suggested bucket **other**.

#### Trash recheck tests credit a migration coordinator

Drop the “other-dialogs area's coordinator” attribution. Keep the reason for suppressing more reads until a fresh trash
epoch arrives, since an unnecessary recheck could hold up the window's ordered commands.

Reviewed locations: `crates/gui/farhelm-gui/src/trash/dialog/tests.rs:371-376`,
`crates/gui/farhelm-gui/src/trash/tests.rs:343-348`. Haiku out-T8.md §4. Suggested bucket **other**.

### Screenshot scenes

#### Remove milestone and merge narration from the screenshot docs

The comments name M1, work-area ownership, and individual rebases. Keep the constraints they explain, including the
bundled font’s coverage and why adding a scene should only add an entry.

Reviewed locations: `crates/gui/farhelm-gui/src/goldens/terminal.rs:74-78`,
`crates/gui/farhelm-gui/src/goldens/mod.rs:402-405`, `crates/gui/farhelm-gui/src/goldens/foundations.rs:1017-1019`.
Haiku out-P3.md §2. Suggested bucket **other**.
