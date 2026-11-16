# GPUI foundation ownership

Each app-wide concern of the GPUI app has one owner (SPEC_impl.md, "GUI architecture"). This table says which paths each
owner holds and how an area may change them. The lint exceptions column counts the `#[allow]`s that let an owner make
the one call `crates/gui/clippy.toml` bans everywhere else; `scripts/check-gui-lints.py` fails on such an `#[allow]` on
any other item (a new one is added to its list along with this column), and on a crate-wide switch, which no crate
holds: a ban allowed in a GUI crate's `[lints]` table, or a second clippy configuration under `crates/gui/`.

Owned paths come in two tiers, and which tier a path is in decides what an area does before touching it:

- **Refused**: the paths that carry a rule (the focus coordinator, the layer manager, the prompt, the store and the
  UI-session owner, the operation store's and the notice center's cores, the enforcement files, the helm client, the
  bridge's Rust side, and the like). An area changes one of these only through an extension point, in the shape its rule
  admits. Every other change is a foundation-change request to the orchestrator (in the area's outbox), never made
  directly by an area.
- **Reported**: the shared wiring (the main view, the crate root and manifest, the golden scenes, the icon list, the
  theme's tokens, the components, the key bindings, and the bridge's scripts and their docs). An area may change these
  without a request when the change follows the pattern already there, such as a new builder method on an existing
  component or a layout change in the main view. The check lists every such touch and the review gate judges it. A
  reported touch that changes a rule the file carries, or that does not follow the pattern, is still a request.

The migration's orchestrator split the tiers on 2026-11-14, after the milestone M1 review ran the check's rules over the
slice PRs: refusing every change to the wiring would have made most of their owned touches requests (13 of 20 for the
session list, 6 of 7 for Rename), for changes as ordinary as a rewrapped `use` line or a new builder method. A request
per wiring change turns requests into rubber stamps or a queue, and pushes areas to work around the check (a second
`use` line instead of a rewrap, a copied component instead of a builder method). The alternative, keeping the check
strict and promising same-hour answers to requests, was rejected because it ties every area's pace to the
orchestrator's.

The table covers every foundation file as milestone M1 left them. From M1 on, the landing step of every area PR runs
`scripts/check-foundation-paths.py`, which refuses the PR when it touches a refused path in a way no extension point
admits, and prints every reported touch for the review gate without refusing it. A foundation PR made by the owner runs
it with `--owner`, which reports the same findings without refusing.

This file is the check's only source. It reads each foundation's owned paths from the main table's second column (the
backticked paths there), the rules, the reported tier's included, from "Extension point rules" below, and each area's
paths from the area table under "What is not a foundation", always from the base revision's copy of this file, so a PR
cannot widen the rules it is judged by. For the same reason the landing step runs the base's copy of the script, from
stdin, after rebasing the PR onto that base:

    git show <base>:scripts/check-foundation-paths.py | python3 -I - --repo <checkout> --base <base> --head <PR commit>

`--head` is the PR's commit id or bookmark, never `HEAD`: in a jj workspace git's `HEAD` is the working copy's parent,
which need not be the PR's tip. Every path the main and rules tables name must exist in the base revision, so an owner
who renames a foundation file updates the table in the same PR, or the check stops.

Paths are relative to the repository root; a path ending in `/` covers everything under it.

The main table keeps to short cells, so that most new or edited rows leave the others alone: only a cell wider than its
column's widest re-pads every row, and the owned paths column's widest cell is still the Enforcement row's. Its third
column names, in a word or two, what the foundation's extension points are (the rules they carry, "none" when there are
none, or "from area code" when they touch no owned path) and links to the foundation's subsection under "Extension
points and lint exceptions, by foundation", which holds the whole text; its fourth column names the lint exceptions'
owner items, and the subsection says why each exists. The check reads neither column, but it reads every row as four
cells, so a row keeps all four. Before 2026-11-14 the whole text sat in the table, and with cells over two thousand
characters long every edit that widened a column re-padded every row, so any two PRs that each touched the table
conflicted across all of it.

| Foundation                      | Owned paths                                                                                                                                                                                                                                                                                                                    | Extension points                         | Lint exceptions                       |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------- | ------------------------------------- |
| Bootstrap                       | `crates/gui/farhelm-gui/src/bootstrap.rs`, `crates/gui/farhelm-gui/src/lib.rs`, `crates/gui/farhelm-gui/Cargo.toml`                                                                                                                                                                                                            | [reported](#bootstrap)                   | none                                  |
| Theme                           | `crates/gui/farhelm-gui/src/theme.rs`                                                                                                                                                                                                                                                                                          | [reported](#theme)                       | 1: `theme::raw`                       |
| Component layer                 | `crates/gui/farhelm-gui/src/components/`                                                                                                                                                                                                                                                                                       | [reported](#component-layer)             | 1: `components::transition::one_shot` |
| Focus coordinator               | `crates/gui/farhelm-gui/src/focus.rs`, `crates/gui/farhelm-gui/src/focus_tests.rs`                                                                                                                                                                                                                                             | [none](#focus-coordinator)               | 2: `focus::apply`, `focus::track_raw` |
| Layer manager                   | `crates/gui/farhelm-gui/src/layers.rs`, `crates/gui/farhelm-gui/src/layers_tests.rs`, `crates/gui/farhelm-gui/src/layers/`                                                                                                                                                                                                     | [none](#layer-manager)                   | 1: `layers::overlay`                  |
| Root view wiring                | `crates/gui/farhelm-gui/src/root.rs`                                                                                                                                                                                                                                                                                           | [none](#root-view-wiring)                | none                                  |
| Prompt                          | `crates/gui/farhelm-gui/src/prompt.rs`, `crates/gui/farhelm-gui/src/prompt_tests.rs`                                                                                                                                                                                                                                           | [none](#prompt)                          | none                                  |
| Actions and keymap              | `crates/gui/farhelm-gui/src/actions.rs`, `crates/gui/farhelm-gui/src/actions_tests.rs`                                                                                                                                                                                                                                         | [reported](#actions-and-keymap)          | 1: `actions::install_interceptor`     |
| Host text                       | `crates/gui/farhelm-gui/src/host_text.rs`, `crates/farhelm-proto/src/text.rs`                                                                                                                                                                                                                                                  | [from area code](#host-text)             | none                                  |
| Cache wrapper                   | `crates/gui/farhelm-gui/src/cache.rs`                                                                                                                                                                                                                                                                                          | [none](#cache-wrapper)                   | 1: `cache::element`                   |
| Store                           | `crates/gui/farhelm-gui/src/store.rs`, `crates/gui/farhelm-gui/src/store_tests.rs`                                                                                                                                                                                                                                             | [none](#store)                           | none                                  |
| UI-session owner and connection | `crates/gui/farhelm-gui/src/session.rs`, `crates/gui/farhelm-gui/src/session_tests.rs`                                                                                                                                                                                                                                         | [none](#ui-session-owner-and-connection) | none                                  |
| Operation store                 | `crates/gui/farhelm-gui/src/operations.rs`, `crates/gui/farhelm-gui/src/operations_tests.rs`                                                                                                                                                                                                                                   | [append-only](#operation-store)          | none                                  |
| Drafts                          | `crates/gui/farhelm-gui/src/drafts.rs`                                                                                                                                                                                                                                                                                         | [from area code](#drafts)                | none                                  |
| Selected-session survivor       | `crates/gui/farhelm-gui/src/local_state.rs`                                                                                                                                                                                                                                                                                    | [none](#selected-session-survivor)       | none                                  |
| Ticker                          | `crates/gui/farhelm-gui/src/ticker.rs`, `crates/gui/farhelm-gui/src/ticker_tests.rs`                                                                                                                                                                                                                                           | [none](#ticker)                          | 1: `ticker::timer`                    |
| Notice center                   | `crates/gui/farhelm-gui/src/notices.rs`                                                                                                                                                                                                                                                                                        | [append-only](#notice-center)            | none                                  |
| Render counter                  | `crates/gui/farhelm-gui/src/render_count.rs`                                                                                                                                                                                                                                                                                   | [none](#render-counter)                  | none                                  |
| Test harness                    | `crates/gui/farhelm-gui/src/test_support.rs`, `crates/gui/farhelm-gui/src/goldens.rs`, `scripts/check-gui-goldens.py`                                                                                                                                                                                                          | [reported, append-only](#test-harness)   | none                                  |
| Assets                          | `crates/gui/farhelm-gui/src/assets.rs`, `crates/gui/farhelm-gui/assets/`                                                                                                                                                                                                                                                       | [reported, new files](#assets)           | none                                  |
| Main view                       | `crates/gui/farhelm-gui/src/main_view.rs`                                                                                                                                                                                                                                                                                      | [reported](#main-view)                   | none                                  |
| Harness marks                   | `crates/gui/farhelm-gui/src/marks.rs`, `crates/gui/farhelm-gui/src/marks/`                                                                                                                                                                                                                                                     | [append-only, new files](#harness-marks) | none                                  |
| Enforcement                     | `crates/gui/clippy.toml`, `crates/gui/lint-demo/`, `scripts/check-gui-lints.py`, `scripts/test-check-gui-lints.py`, `scripts/check-crate-layering.py`, `scripts/test-check-crate-layering.py`, `deny.toml`, `docs/gpui/foundation-ownership.md`, `scripts/check-foundation-paths.py`, `scripts/test-check-foundation-paths.py` | [none](#enforcement)                     | none                                  |
| Helm client                     | `crates/farhelm-helm-client/`                                                                                                                                                                                                                                                                                                  | [none](#helm-client)                     | the crate itself                      |
| Test-automation bridge          | `crates/farhelm-desktop/src/agent.rs`, `crates/gui/farhelm-gui/src/agent_startup.rs`, `scripts/gui-agent.py`, `scripts/test-gui-agent.py`, `docs/gui-agent.md`, `scripts/gui-screenshot-batch.py`, `scripts/test-gui-screenshot-batch.py`, `scripts/gui-e2e-smoke.py`, `scripts/test-gui-e2e-smoke.py`                         | [reported](#test-automation-bridge)      | none                                  |
| Host identity                   | `crates/gui/farhelm-gui/src/hosts/identity.rs`, `crates/gui/farhelm-gui/src/hosts/identity/`                                                                                                                                                                                                                                   | [reported](#host-identity)               | none                                  |
| Harness answers                 | `crates/gui/farhelm-gui/src/harness.rs`                                                                                                                                                                                                                                                                                        | [reported](#harness-answers)             | none                                  |
| Permission shield               | `crates/gui/farhelm-gui/src/permission.rs`                                                                                                                                                                                                                                                                                     | [reported](#permission-shield)           | none                                  |
| Permission words                | `crates/gui/farhelm-gui/src/permission/words.rs`                                                                                                                                                                                                                                                                               | [reported](#permission-words)            | none                                  |
| Provider wording                | `crates/gui/farhelm-gui/src/environment_ui.rs`                                                                                                                                                                                                                                                                                 | [reported](#provider-wording)            | none                                  |
| Template summary                | `crates/gui/farhelm-gui/src/templates/summary.rs`                                                                                                                                                                                                                                                                              | [reported](#template-summary)            | none                                  |
| Save as template                | `crates/gui/farhelm-gui/src/templates/save.rs`, `crates/gui/farhelm-gui/src/templates/save_panel.rs`                                                                                                                                                                                                                           | [reported](#save-as-template)            | none                                  |
| Terminal tunnels                | `crates/gui/farhelm-gui/src/terminal/tunnels.rs`                                                                                                                                                                                                                                                                               | [reported](#terminal-tunnels)            | none                                  |
| Terminal text size              | `crates/gui/farhelm-gui/src/terminal/text_size.rs`                                                                                                                                                                                                                                                                             | [reported](#terminal-text-size)          | none                                  |
| Terminal set                    | `crates/gui/farhelm-gui/src/terminal/set.rs`                                                                                                                                                                                                                                                                                   | [reported](#terminal-set)                | none                                  |
| Add host dialog                 | `crates/gui/farhelm-gui/src/hosts/add.rs`                                                                                                                                                                                                                                                                                      | [reported](#add-host-dialog)             | none                                  |
| Hosts panel                     | `crates/gui/farhelm-gui/src/hosts.rs`                                                                                                                                                                                                                                                                                          | [reported](#hosts-panel)                 | none                                  |
| Host offers                     | `crates/gui/farhelm-gui/src/hosts/offers.rs`                                                                                                                                                                                                                                                                                   | [reported](#host-offers)                 | none                                  |
| Approval region                 | `crates/gui/farhelm-gui/src/approvals/region.rs`                                                                                                                                                                                                                                                                               | [reported](#approval-region)             | none                                  |
| Approval answers                | `crates/gui/farhelm-gui/src/approvals/answers.rs`, `crates/gui/farhelm-gui/src/approvals/host_add.rs`                                                                                                                                                                                                                          | [reported](#approval-answers)            | none                                  |
| Title band                      | `crates/gui/farhelm-gui/src/desktop/title_band.rs`                                                                                                                                                                                                                                                                             | [reported](#title-band)                  | none                                  |
| Updater window side             | `crates/gui/farhelm-gui/src/desktop/updates.rs`                                                                                                                                                                                                                                                                                | [reported](#updater-window-side)         | none                                  |
| Launcher entry                  | `crates/gui/farhelm-gui/src/launcher/entry.rs`                                                                                                                                                                                                                                                                                 | [reported](#launcher-entry)              | none                                  |
| Launcher view                   | `crates/gui/farhelm-gui/src/launcher/view.rs`                                                                                                                                                                                                                                                                                  | [reported](#launcher-view)               | none                                  |
| Launch draft                    | `crates/gui/farhelm-gui/src/launcher/draft.rs`                                                                                                                                                                                                                                                                                 | [reported](#launch-draft)                | none                                  |
| Launch models                   | `crates/gui/farhelm-gui/src/launcher/model.rs`                                                                                                                                                                                                                                                                                 | [reported](#launch-models)               | none                                  |
| Session replace                 | `crates/gui/farhelm-gui/src/session_list/replace.rs`                                                                                                                                                                                                                                                                           | [reported](#session-replace)             | none                                  |
| Operation outcomes              | `crates/gui/farhelm-gui/src/session_list/outcome.rs`                                                                                                                                                                                                                                                                           | [reported](#operation-outcomes)          | none                                  |
| Session age                     | `crates/gui/farhelm-gui/src/session_list/age.rs`                                                                                                                                                                                                                                                                               | [reported](#session-age)                 | none                                  |
| Host settings                   | `crates/gui/farhelm-gui/src/hosts/settings.rs`                                                                                                                                                                                                                                                                                 | [reported](#host-settings)               | none                                  |
| SSH help                        | `crates/gui/farhelm-gui/src/hosts/ssh_help.rs`                                                                                                                                                                                                                                                                                 | [reported](#ssh-help)                    | none                                  |
| Open-source notices             | `crates/gui/farhelm-gui/src/desktop/licenses.rs`                                                                                                                                                                                                                                                                               | [reported](#open-source-notices)         | none                                  |
| Version readout view            | `crates/gui/farhelm-gui/src/update_readout/view.rs`                                                                                                                                                                                                                                                                            | [reported](#version-readout-view)        | none                                  |
| Trash cue                       | `crates/gui/farhelm-gui/src/trash/cue.rs`                                                                                                                                                                                                                                                                                      | [reported](#trash-cue)                   | none                                  |
| Notices bell                    | `crates/gui/farhelm-gui/src/notices_bell.rs`                                                                                                                                                                                                                                                                                   | [reported](#notices-bell)                | none                                  |
| Help menu                       | `crates/gui/farhelm-gui/src/help_menu.rs`                                                                                                                                                                                                                                                                                      | [reported](#help-menu)                   | none                                  |
| Trash button                    | `crates/gui/farhelm-gui/src/trash.rs`                                                                                                                                                                                                                                                                                          | [reported](#trash-button)                | none                                  |
| Templates dialog entry          | `crates/gui/farhelm-gui/src/templates.rs`                                                                                                                                                                                                                                                                                      | [reported](#templates-dialog-entry)      | none                                  |
| Settings dialog                 | `crates/gui/farhelm-gui/src/settings.rs`                                                                                                                                                                                                                                                                                       | [reported](#settings-dialog)             | none                                  |
| Terminal cell metrics           | `crates/gui/farhelm-gui/src/terminal.rs`                                                                                                                                                                                                                                                                                       | [reported](#terminal-cell-metrics)       | none                                  |
| Session selection               | `crates/gui/farhelm-gui/src/session_list/selection.rs`                                                                                                                                                                                                                                                                         | [reported](#session-selection)           | none                                  |
| Session list                    | `crates/gui/farhelm-gui/src/session_list.rs`                                                                                                                                                                                                                                                                                   | [reported](#session-list)                | none                                  |
| Status sounds                   | `crates/gui/farhelm-gui/src/sounds.rs`, `crates/gui/farhelm-gui/src/sounds/rules.rs`                                                                                                                                                                                                                                           | [reported](#status-sounds)               | none                                  |
| Session restart                 | `crates/gui/farhelm-gui/src/session_list/restart.rs`                                                                                                                                                                                                                                                                           | [reported](#session-restart)             | none                                  |
| Session menu                    | `crates/gui/farhelm-gui/src/session_list/menu.rs`                                                                                                                                                                                                                                                                              | [reported](#session-menu)                | none                                  |
| Session delete                  | `crates/gui/farhelm-gui/src/session_list/delete.rs`                                                                                                                                                                                                                                                                            | [reported](#session-delete)              | none                                  |
| Rail notices                    | `crates/gui/farhelm-gui/src/rail_notices.rs`                                                                                                                                                                                                                                                                                   | [reported](#rail-notices)                | none                                  |
| Launching                       | `crates/gui/farhelm-gui/src/launcher/launch.rs`                                                                                                                                                                                                                                                                                | [reported](#launching)                   | none                                  |
| Template shortcuts              | `crates/gui/farhelm-gui/src/templates/shortcuts.rs`                                                                                                                                                                                                                                                                            | [reported](#template-shortcuts)          | none                                  |
| Session rename                  | `crates/gui/farhelm-gui/src/session_list/rename.rs`                                                                                                                                                                                                                                                                            | [reported](#session-rename)              | none                                  |
| Template rows                   | `crates/gui/farhelm-gui/src/templates/rows.rs`                                                                                                                                                                                                                                                                                 | [reported](#template-rows)               | none                                  |
| Template dispatch               | `crates/gui/farhelm-gui/src/templates/dispatch.rs`                                                                                                                                                                                                                                                                             | [reported](#template-dispatch)           | none                                  |
| Template checkout name          | `crates/gui/farhelm-gui/src/templates/checkout_name.rs`                                                                                                                                                                                                                                                                        | [reported](#template-checkout-name)      | none                                  |
| Shortcuts overview              | `crates/gui/farhelm-gui/src/shortcuts.rs`                                                                                                                                                                                                                                                                                      | [reported](#shortcuts-overview)          | none                                  |
| Host status                     | `crates/gui/farhelm-gui/src/hosts/status.rs`                                                                                                                                                                                                                                                                                   | [reported](#host-status)                 | none                                  |

## Extension points and lint exceptions, by foundation

One subsection per row of the main table, in its order, with the text its third and fourth columns link to.

### Bootstrap

Extension points: reported: `lib.rs`, for a new area module (its `pub mod` line and its bullet in the module docs), and
`Cargo.toml`, for a dependency an area needs. `bootstrap.rs` has none, since it carries the startup order every app-wide
global depends on; its desktop integration seams (menus, quitting on close, the window's saved frame and chrome, the
notices, the updater, second launches) are calls into the area module `desktop.rs`, and its one status-sound seam, the
host's player, is handed to other-dialogs' `sounds::install_player`; the bootstrap reads no field of their types, so the
areas fill them in without touching `bootstrap.rs`.

Lint exceptions: none.

### Theme

Extension points: reported: a palette token copied from the old stylesheet at the reference commit, with the CSS
variable it came from in its doc (a change to how `theme::raw` builds colors is a request, and so is a change to the
projection of Farhelm's tokens onto GPUI Kit's theme, `kit_config` and `apply`, the one place outside the component
layer that configures Kit).

Lint exceptions: 1: `theme::raw`, the one place raw colors are built: `theme::color` for palette tokens, and
`TerminalColor::hsla` for the terminal's resolved colors, which include a program's truecolor and the 256-color cube
(the migration's orchestrator decided on 2026-11-13 that the terminal palette lives in the theme, with no second
exemption).

### Component layer

Extension points: reported, the whole directory. An area may add, without a request:

- a new control in its own file under `components/`, declared with its `mod` and `pub use` lines in `components/mod.rs`
  and tested in its own file under `components/tests/`, whose constructor takes a label, hover help and a role, which
  wires its help through `with_hover_help` and tracks its focus through `focus::track` (a control over a GPUI Kit input,
  which tracks its own focus, records its position with `focus::record_position` instead);
- or an `OptionRow` list driven by the field that owns it, built in the area's own code (the layer's first exception, in
  SPEC_impl "The component layer and theme"; `option_row.rs` says where it may be drawn);
- or the layer's second exception, decided by the migration's orchestrator on 2026-11-14: a pointer control whose action
  its owner offers by keyboard may take no focus, as a `MarkToggle` (its row's menu item offers the action) and the
  terminal's `Scrollbar` (its terminal offers Shift+PageUp and Shift+PageDown) do. Desktop's Linux title bar buttons
  (Minimize, Maximize, which reads Restore while the window is maximized, and Close, in
  `crates/gui/farhelm-gui/src/desktop/linux_title_bar.rs`) are under it too: a click on one leaves keyboard focus where
  it was, as a compositor's title buttons do, through `Button::keeps_focus_on_press` and
  `IconButton::keeps_focus_on_press`, on Linux only, while Tab still reaches them (decided with the migration's
  orchestrator on 2026-11-14);
- or an additive builder method on an existing control, built the way its siblings are;
- or a keyboard stop for a place where focus rests, through `focus_stop` in `components/mod.rs` with a focus handle the
  area owns, a role, a label and a `Ring`, as the session pane's interrupted card is to be: the scroll stop's ring, role
  and Tab reach without its keys, so the area can make the handle home before the stop is first drawn (a region that
  scrolls takes a `ScrollBox` or a dialog's `DialogFrame::scroll_stop` instead, and one that scrolls and is also where
  focus rests, as the session pane's unavailable card is, takes `scroll_stop_on` with a handle the area owns);
- or keyboard scrolling for a dialog's notices region that holds a question, through `DialogFrame::notices_stop` and a
  label: the region is a stop of its own whenever it holds no tab stop, as a body given `DialogFrame::scroll_stop` is,
  and Page Up, Page Down, Home and End scroll it from a control inside it (`scroll_from_controls`), as the old Trash
  dialog's global confirmation was scrolled around its focused answer. It is the one region whose controls' keys scroll
  it; any other region taking keys from a control inside it is a request.

A drawing-only shared piece (no input, no focus, nothing to name) belongs in the layer too and is exempt from the label,
hover help and role rule, decided by the migration's orchestrator on 2026-11-14: `pulse.rs` (`pulse_opacity`),
`menu_separator.rs` (`menu_separator`) and `spinner.rs` (`Spinner` and `spinner_icon`) are three.

`HoverHelp`, `with_hover_help`, `activate_on_enter_or_space`, `activate_on_click`, `enter_choice`, `on_enter_choice`,
`focus_stop`, `scroll_stop`, `scroll_stop_on` and `scroll_from_controls` in `components/mod.rs` are the layer's rules: a
change to them is a request. An element that fades in as it appears is drawn through `components::entrance`, from the
area's own code, and `components/transition.rs` (tested in `components/tests/transition.rs`), the one owner of one-shot
transitions, is a rule too: a change to it, a new transition included, is a request.

Lint exceptions: 1: GPUI's `with_animation` in `components::transition::one_shot`, the one-shot transitions the ticker's
ten ticks a second cannot draw (SPEC_impl.md, "Animation time: the ticker"; the migration's orchestrator approved this
owner on 2026-11-14).

### Focus coordinator

Extension points: none (a scrolling container opts in to having the control focus lands on scrolled into view by drawing
through `focus::scroll_region` in its own code, and a dialog or menu a button opens names that button as where closing
returns focus with `focus::ReturnTo::Control` in its own `Dialog` or `Popover`, neither a change here).

Lint exceptions: 2: the focus moves in `focus::apply`; `track_focus` in `focus::track_raw`.

### Layer manager

Extension points: none. A feature opens a popover from its own code through one of the presets, `layers::Popover::menu`
or `layers::Popover::floating`, and changes a default only through their builders, per their docs, not a change here: a
menu a toggle opens names the toggle with `returns_to(focus::ReturnTo::Control(..))`, and a popover opened from a toggle
may let Tab through, walking its controls in the page's order after the toggle and closing once focus leaves it, with
`tab_through()` and `returns_to` naming the toggle. A feature draws one display-only element above every layer, the
pill, through `layers::show_pill` and `layers::hide_pill` from its own code, not a change here; its view must take no
input, per the `layers::Pill` docs. The launcher draws the second, the launch pill (its progress pill and "Started"
report), at the window's bottom through `layers::show_launch_pill` and `layers::hide_pill` from its own code, under the
same rule for its view; the two show at once, and neither replaces the other's. And the approvals feature installs the
region that draws agent approval cards above every dialog but a raised one, once per window, through
`layers::install_approval_region` from its own code; its view must occlude what it draws and draw nothing with a size or
a listener when empty, per that function's docs. The terminal's tunnel warning raises its dialog's box (not its veil)
above that region as it opens, through `layers::above_approval_region` from its own code (`terminal/tunnels.rs`), so a
card that asks for it cannot cover it; that function's docs say why only that warning is raised, and raising another
layer is a decision of this foundation's owner. The sidebar draws the edge that hover help placed beside it sits
against, once per window, through `layers::beside_edge` from its own code; a control asks for that placement through its
hover help (`HoverHelp::beside` in the component layer), which only SPEC.md's session-row hovers and host progress card
do, and only for a control drawn inside the column that draws the edge: the help is placed against the edge wherever the
control is, where the old app checked that the control was inside the sidebar.

Lint exceptions: 1: `deferred` and `anchored` in `layers::overlay`.

### Root view wiring

Extension points: none (a root view keeps a `root::Attachment` and draws through `root::wrap`).

Lint exceptions: none.

### Prompt

Extension points: none. A new question is a `Question` built by the feature, not a change here, with any of:

- a relabelled Cancel (`Question::cancel_answer`), extra answers (`Question::alternative`), or Escape sending focus home
  (`Question::returns_home_on_escape`);
- an answer whose action can fail and keeps the question up with the reason, an extra one
  (`Question::fallible_alternative`, on an inline question or a centered one built with
  `Question::keeps_focus_on_empty_click`) or the confirming one (`prompt::ask_fallible`, only on such a centered
  question), settled through `prompt::Attempt`;
- the YOLO confirmation's loud look (`Question::loud`) and its outlined permanent answer (`Tier::DangerOutline`);
- for a centered question, a body the feature draws, built once from the frozen value as the question is asked
  (`Question::body`), and the host dialogs' header tile, footer note and width (`Question::tile`, `Question::note`,
  `Question::width`) where SPEC.md asks for them.

It is asked centered with `prompt::ask`, beside its control with `prompt::ask_anchored`, or inside the owner's own
surface with `prompt::ask_inline` (the owner draws the `prompt::Inline` it returns).

Lint exceptions: none.

### Actions and keymap

Extension points: reported:

- declaring a new action and its key binding in `CONTEXT`;
- a row in `TERMINAL_KEYS` for a chord that a binding takes where a terminal has focus (an app shortcut that works from
  a terminal, or a key the terminal or its program keeps);
- a row in `PHYSICAL_KEYS` for a shortcut SPEC.md matches by key position;
- a terminal command's action as the `bind` of its `KeyOwner::Terminal` row in `TERMINAL_KEYS`;
- an action in `ONCE_PER_PRESS` for a shortcut SPEC.md says ignores repeat;
- a chord in `TEXT_FIELD_KEYS` for an app shortcut SPEC.md says a text field keeps;
- arming the key capture slot (`arm_key_capture`, disarmed with `disarm_key_capture`) for a mode SPEC.md says takes
  every key without taking focus;
- the tests of a new binding in `actions_tests.rs`.

The tables are rules: the review gate checks that a new row has the reason its table names, and a change to how a table
is applied is a request.

Lint exceptions: 1: GPUI's keystroke interceptor (`App::intercept_keystrokes`) in `actions::install_interceptor`, the
one place in the GUI crates that watches keys app-wide (GPUI Kit's text fields add their own, which only pause the
cursor blink), so key capture, the physical-key rows and dropped repeats apply to every key. A control that needs more
about its own keys, such as whether Enter was held, reads its key events and reports that through its own events.

### Host text

The owned paths are the host text module and `crates/farhelm-proto/src/text.rs`, the escape table and its `<U+XXXX>`
notation, shared with the command line.

Extension points, all from area code:

- a view wrapping a new host-supplied field with `HostText::from_host` where it reads it (an identity the user compares,
  such as an installation or session id, with `HostText::identity`, or with `HostText::identity_abbreviated` where a
  compact line shows only its ends) and drawing it with `host_text::element` (or `host_text::marked`, with some
  characters marked, such as a search's matches) or a `HostSentence` (a technical value such as a path or destination as
  a monospace part, `HostSentence::mono`);
- a component taking `&HostText` and drawing it the same way, or taking a `host_text::Line` where its wording may name a
  host value;
- a control's hover help that names a host value, built with `HoverHelp::sentence` from a `HostSentence` (or, for help
  in paragraphs, with `HoverHelp::paragraphs` from one `HostSentence` per paragraph).

Lint exceptions: none.

### Cache wrapper

Extension points: none.

Lint exceptions: 1: GPUI view caching (`Entity::cached`) in `cache::element`.

### Store

Extension points: none (views read `HelmStore::collections`; only the UI-session owner changes the store).

Lint exceptions: none.

### UI-session owner and connection

Extension points: none (a view that acts on the current UI session's pushed state as it changes follows it by holding
`session::observe_current`'s handle in its own code, not a change here; an area's tests belong in the area's own test
modules, not here).

Lint exceptions: none.

### Operation store

Extension points: a `Work` variant for a new kind of work, with one arm in each of `Work::creates_a_session` and
`Work::changes_host_registry` (the review gate checks both values against the methods' docs) and its own area's tests
pinning them. The following touch no owned path; each is built in the feature's own module:

- the commands themselves, built with `Command::new`, keeping today's API semantics for their route, and started under
  the `OpKey` of what they act on;
- a read of what the helm does not push, sent with `operations::read`;
- a command that is not work on a key (nothing draws it busy and the store records no refusal for it), sent with
  `operations::send`.

A plain new kind of `OpKey` that only names a distinct resource (a private `Key` variant and its constructor, with no
supersede rule and no change to how the store starts, refuses or settles work) is an extension point too, added in the
area's own PR, which says so in its description. Latest-wins for supersedable work, a key shared across kinds and any
change to how the store starts, refuses or settles work are changes here, not extension points. `operations_tests.rs` is
not one: an area's tests belong in the area's own test modules.

Lint exceptions: none.

### Drafts

Extension points, from area code: an editor holding a `Draft` per field and saving through `drafts::save`, or a
`DraftInput` per text field and saving through `DraftInput::save`, under the `OpKey` of what it saves (partial saves are
a change here, made by the first editor whose route needs them).

Lint exceptions: none.

### Selected-session survivor

Extension points: none (it holds only the selected session's id across a reset, written only through `select` and
`clear`: the session list owns the selection rules, the connection only its snapshot check; per-device settings are helm
preferences, not fields here).

Lint exceptions: none.

### Ticker

Extension points: none (each of these is a use of the ticker from a view's own code, not a change here):

- a view animates by holding `ticker::animate`'s handle, `ticker::blink`'s for a cursor blink, or `ticker::pulse`'s for
  the running pulse;
- a deadline is `ticker::after`;
- a view that shows an age holds `ticker::clock`'s handle, one for all its rows, and a view that shows a count of
  seconds `ticker::every_second`'s, and each reads the time from `ticker::wall_now`.

A one-shot entrance is not the ticker's: an area draws it through the component layer's `components::entrance`, from its
own code (the entrance reads `ticker::reduced_motion`).

Lint exceptions: 1: the executor timer in `ticker::timer`.

### Notice center

Extension points: a new `Notice` variant with its text, posted and withdrawn only through `notices::post` and
`notices::withdraw`, with the surface that draws it.

Lint exceptions: none.

### Render counter

Extension points: none.

Lint exceptions: none.

### Test harness

Extension points:

- reported: `goldens.rs`, for a golden scene and its reference PNGs under `crates/gui/farhelm-gui/tests/goldens/` for a
  new control or dialog;
- append-only: `test_support.rs`, for a new shared test helper with a docstring and its test, added only inside its
  "Shared helpers, appended on request" section, any `use` it needs on a line of its own (the other helpers and the
  per-step invariant checks change only by request; the review gate checks that an addition stays inside the section,
  since the rule itself admits an added line anywhere in the file);
- `check-gui-goldens.py` has none.

An area's own tests and helpers belong in the area's own test modules.

Lint exceptions: none.

### Assets

Extension points: reported: `assets.rs`, for an icon's `Icon` variant; the icon itself is a new file under
`crates/gui/farhelm-gui/assets/icons/`; or a brand file (the wordmark SVGs under `packaging/farhelm-desktop/`) embedded
byte for byte under a `brand/...` path for an image, the brand files staying the single source. Fonts are not an
extension point: the terminal's JetBrains Mono faces came with its renderer, by the migration's orchestrator's decision
of 2026-11-13, and the maintainer decided the same day that the shipped app's fonts stay embedded in the binary.

Lint exceptions: none.

### Main view

Extension points: reported: a view drawn inside the main view's ground, and the ground's layout around it (the overlay
is not drawn here: `root::wrap` draws it after the ground).

The main view also holds an owner it never draws, made in `MainView::new` and kept for the window's life: the
`_start_dialogs` field, the window-start dialogs' owner (`welcome::StartDialogs`), which decides when the Welcome card
and the first batch of announcements open. Such a field is reported wiring too, the same way: an area's owner that must
live as long as the window, made where the main view makes its other parts, in the order its docs need (this one after
the session pane, so the selected session's terminal is home before a card opens over it).

Lint exceptions: none.

### Harness marks

Extension points: a new harness's mark, following `docs/harness-marks.md`.

Lint exceptions: none.

### Enforcement

Extension points: none.

Lint exceptions: none.

### Helm client

Extension points: none.

Lint exceptions: the crate itself: the GUI's one async runtime, outside `crates/gui/` (SPEC_impl.md, "Where the GUI
meets the helm").

### Test-automation bridge

Extension points: reported:

- a command in `gui-agent.py` with its tests in `test-gui-agent.py` and its documentation in `docs/gui-agent.md`;
- a change to the screenshot batch and its tests;
- a step of the end-to-end smoke test for an area's feature: a `test_<nn>_...` method of `Smoke` in `gui-e2e-smoke.py`,
  numbered with two digits after the last, because the steps run in name order against one app run and unittest sorts
  names as strings (`test_10_` before `test_1_`); each area's steps seed and use sessions of their own, set up the open
  session and focus they depend on, and never touch a session another area's steps seed or use (the script's module docs
  say how). One step needs a window start of its own and so starts and stops a second app run beside the shared one:
  step 53, the other-dialogs area's Welcome card opened by a window start that asks for it. The script's module docs
  give the pattern such a step follows (its run directory, `--no-build`, and a `stop` in a `finally`).

The Rust side (`agent.rs`, and `agent_startup.rs`, which also holds the guard that refuses the `agent` feature in
optimized builds) has none. The `agent` feature itself is the bridge's too but is declared in Bootstrap's manifest: a
change to it is a request although the manifest is reported.

Lint exceptions: none.

### Host identity

Extension points: reported, the whole module: owned by the settings-and-hosts area, which built it, and shared with the
areas that show a host (its display name, mark, color, phase words and `utc_stamp`); another area's change follows the
pattern already there, such as an additive answer for a new kind of host or phase, and the review gate judges it.

Lint exceptions: none.

### Harness answers

Extension points: reported, the whole file: owned by the sidebar-and-session-view area, which built it, and shared with
the areas that show an agent (each harness's name, mark, ink and status-line words); another area's change follows the
pattern already there, an additive per-harness answer under its exhaustive match, as AGENTS.md's harness rule requires,
and the review gate judges it.

Lint exceptions: none.

### Permission shield

Extension points: reported, the whole file: owned by the sidebar-and-session-view area, which built it, and shared with
the areas that show a session's permission shield (the session row and the approval cards): the classification of a
launch into the shield's four drawings, and the drawings themselves; what a surface says about the shield stays that
surface's own. Another area's change follows the pattern already there, such as an additive answer for a new kind of
launch permission, and the review gate judges it.

Lint exceptions: none.

### Permission words

Extension points: reported, the whole file: owned by the launcher area, which writes and maintains that wording, and
shared with every surface that shows a launch's choices (the launcher's controls and footer, a session row's permission
shield, a template's summary and fields, and an approval card): Farhelm's names and help for permission and effort
choices. Another area's change follows the pattern already there, such as an additive answer for a new permission or
effort choice, and the review gate judges it.

Lint exceptions: none.

### Provider wording

Extension points: reported, the whole file: owned by the approvals-and-agent-spawned-sessions area, which built it, and
shared with the other areas that show a disposable environment (the launcher's and host management's environment
surfaces add their own methods here as they are ported, rather than starting a second module): the GUI's one exhaustive
per-provider wording module, which AGENTS.md's provider rule asks for. Another area's change follows the pattern already
there, an additive per-provider answer under its exhaustive match, and the review gate judges it.

Lint exceptions: none.

### Template summary

Extension points: reported, the whole file: owned by the templates-and-quick-launch area, which built it, and shared
with the areas that show a template's summary or the permission label in it (the approvals area's cards import
`permission_label`; the sidebar's quick switcher draws a template row's summary through `line` and sizes its Quick
launch row's ⚡ chord with `CHORD_TEXT`). Another area's change follows the pattern already there, and the review gate
judges it.

Lint exceptions: none.

### Save as template

Extension points: reported, the whole files: owned by the templates-and-quick-launch area, which built them, and shared
with the launcher, which hosts the Save as template panel (`save_panel::SavePanel`) and builds what it saves from
(`save::Candidate`). Another area's change follows the pattern already there, and the review gate judges it.

Lint exceptions: none.

### Terminal tunnels

Extension points: reported, the whole file: owned by the terminal area, which built it, and shared with the areas that
ask for tunnel consent outside a terminal (Settings' tunnel switch, through `ask_consent`, for settings-and-hosts; an
agent's tunnel request, through `consented`, `ask_consent_about` and `opened_elsewhere`, for
approvals-and-agent-spawned-sessions). Another area's change follows the pattern already there, and the review gate
judges it. The warning's raise above the approval region stays the layer manager's rule ("Layer manager").

Lint exceptions: none.

### Terminal text size

Extension points: reported, the whole file: owned by the terminal area, which built it, and shared with
sidebar-and-session-view, whose session header draws the size controls (`controls`) and whose pane measures its cells at
the current size (`current`). Another area's change follows the pattern already there, and the review gate judges it.

Lint exceptions: none.

### Terminal set

Extension points: reported, the whole file: owned by the terminal area, which built it, and shared with
sidebar-and-session-view, whose session pane holds the open session's terminals through it (`TerminalSet`,
`SharedHold`). Another area's change follows the pattern already there, and the review gate judges it.

Lint exceptions: none.

### Add host dialog

Extension points: reported, the whole file: owned by the settings-and-hosts area, which built it, and shared with the
areas that open the dialog (the host rail's Add host, through `open` and `ADD_HOST_HELP`, for sidebar-and-session-view;
an agent's host add, through `open_for_agent` and `NotOpened`, for approvals-and-agent-spawned-sessions). Another area's
change follows the pattern already there, and the review gate judges it.

Lint exceptions: none.

### Hosts panel

Extension points: reported, the whole file: owned by the settings-and-hosts area, which built it. Other areas use three
of its items: `HostsPanel`, which the session list shows in place of its rows (sidebar-and-session-view), and, for the
launcher, `host_change_in_flight`, the check that refuses a launch while a host registry change is in flight, and
`start_yolo_without_asking_command`, re-exported here from `hosts/settings.rs`. The table holds only whole paths, so the
row covers the whole panel and the owner's own changes to it are reported too; the review gate passes those that leave
the shared items alone. Another area's change follows the pattern already there, and the review gate judges it.

Lint exceptions: none.

### Host offers

Extension points: reported, the whole file: owned by the settings-and-hosts area, which built it, and shared with the
session list's status line, which says Too new from `runs_newer_version` (sidebar-and-session-view), and the launcher's
entry points, which read `HostKind`. Another area's change follows the pattern already there, and the review gate judges
it.

Lint exceptions: none.

### Approval region

Extension points: reported, the whole file: owned by the approvals-and-agent-spawned-sessions area, which built it, and
shared with sidebar-and-session-view, whose session pane draws the terminal area the region places its card against
(`track_terminal_area`); the main view installs the region and names its pane (`install`, `set_pane`). Another area's
change follows the pattern already there, and the review gate judges it. What the region's view must do as the layer
manager's approval slot stays that foundation's rule ("Layer manager").

Lint exceptions: none.

### Approval answers

Extension points: reported, both files: owned by the approvals-and-agent-spawned-sessions area, which built them, and
shared with settings-and-hosts, whose Add host dialog holds and answers an agent's host add (`host_add`'s
`AgentRequest`, `Answered`, `NOT_CONFIRMED` and `approved_fields`) and posts a late answer's notice (`answers`'s
`post_late` and `Sent`). Another area's change follows the pattern already there, and the review gate judges it.

Lint exceptions: none.

### Title band

Extension points: reported, the whole file: owned by the desktop-integration-and-linux-packaging area, which built it,
and shared with sidebar-and-session-view, whose app bar, session top row and empty pane's strip are rows of the band
(`Row`, `row`, `strip`, `claims_presses`, `traffic_light_room`, `ON`, `HEIGHT`), and with templates-and-quick-launch,
whose armed sequence reads `ON`. Another area's change follows the pattern already there, and the review gate judges it.

Lint exceptions: none.

### Updater window side

Extension points: reported, the whole file: owned by the desktop-integration-and-linux-packaging area, which built it,
and shared with the areas that show or drive the updater (the sidebar's version readout, through `entity`, for
sidebar-and-session-view; Settings' App section and a host's menu, through `runs`, `automatic_updates`,
`set_automatic_updates` and `check_now`, for settings-and-hosts; the Help menu, through `runs` and `check_now`, for
other-dialogs). Another area's change follows the pattern already there, and the review gate judges it.

Lint exceptions: none.

### Launcher entry

Extension points: reported, the whole file: owned by the launcher area, which built it as the ordinary public functions
other surfaces call to open the launcher, and shared with them (the session list's New and its plain Replace's YOLO
question, through items such as `open_new`, `new_available` and `ask_yolo_for_replace`, for sidebar-and-session-view; a
quick launch, through items such as `open_launcher`, `new_prefill` and `quick_launch_available`, for
templates-and-quick-launch). Another area's change follows the pattern already there, and the review gate judges it.

Lint exceptions: none.

### Launcher view

Extension points: reported, the whole file: owned by the launcher area, which built it, and shared with
templates-and-quick-launch, whose dispatch prepares a launch out of sight (`prepare_unseen`, `Prepared`, `Note`) and
applies templates to the open dialog's draft (`Launcher::apply_batch`). Another area's change follows the pattern
already there, and the review gate judges it.

Lint exceptions: none.

### Launch draft

Extension points: reported, the whole file: owned by the launcher area, which built it, and shared with
templates-and-quick-launch, which applies templates to a draft through its edits (items such as `LaunchDraft`, `Catalog`
and `Field`) and names a harness with `harness_label` in a template's summary and editor. Another area's change follows
the pattern already there, and the review gate judges it.

Lint exceptions: none.

### Launch models

Extension points: reported, the whole file: owned by the launcher area, which built it, and shared with
templates-and-quick-launch, whose template editor offers the same model list (`ModelOption`, `model_options`). Another
area's change follows the pattern already there, and the review gate judges it.

Lint exceptions: none.

### Session replace

Extension points: reported, the whole file: owned by the sidebar-and-session-view area, which built it, and shared with
the launcher, whose Replace with settles the way a plain Replace does (`settle`) and warns from the same rule
(`launcher_warning`). Another area's change follows the pattern already there, and the review gate judges it.

Lint exceptions: none.

### Operation outcomes

Extension points: reported, the whole file: owned by the sidebar-and-session-view area, which built it, and shared with
the launcher, whose Replace with starts the source's waiting line (`start_clock`). Another area's change follows the
pattern already there, and the review gate judges it.

Lint exceptions: none.

### Session age

Extension points: reported, the whole file: owned by the sidebar-and-session-view area, which built it, and shared with
other-dialogs, whose notification list shows each notice's age the way a row does (`seconds`, `of`). Another area's
change follows the pattern already there, and the review gate judges it.

Lint exceptions: none.

### Host settings

Extension points: reported, the whole file: owned by the settings-and-hosts area, which built it. Another area uses one
of its items: the launcher's YOLO question sends `start_yolo_without_asking_command`, re-exported through `hosts.rs`.
The table holds only whole paths, so the row covers the whole host settings dialog and the owner's own changes to it are
reported too; the review gate passes those that leave the shared item alone. Another area's change follows the pattern
already there, and the review gate judges it.

Lint exceptions: none.

### SSH help

Extension points: reported, the whole file: owned by the settings-and-hosts area, which built it, and shared with
other-dialogs, whose Help menu opens it (`open`). Another area's change follows the pattern already there, and the
review gate judges it.

Lint exceptions: none.

### Open-source notices

Extension points: reported, the whole file: owned by the desktop-integration-and-linux-packaging area, which built it,
and shared with other-dialogs, whose Help menu names its item after the window (`TITLE`). Another area's change follows
the pattern already there, and the review gate judges it.

Lint exceptions: none.

### Version readout view

Extension points: reported, the whole file: owned by the desktop-integration-and-linux-packaging area, which built it,
and shared with sidebar-and-session-view, whose sidebar header draws the readout (`VersionReadout`). The directory
beside it holds only its tests, so the row names the file. Another area's change follows the pattern already there, and
the review gate judges it.

Lint exceptions: none.

### Trash cue

Extension points: reported, the whole file: owned by the other-dialogs area, which built it, and shared with
sidebar-and-session-view, whose session Delete plays the archive cue from its completion (`archived`). Another area's
change follows the pattern already there, and the review gate judges it.

Lint exceptions: none.

### Notices bell

Extension points: reported, the whole file: owned by the other-dialogs area, which built it. Another area uses one of
its items: the session list draws each row's bell (`bell`, sidebar-and-session-view). The table holds only whole paths,
so the row covers the bell's list too and the owner's own changes to it are reported; the review gate passes those that
leave the shared item alone. Another area's change follows the pattern already there, and the review gate judges it.

Lint exceptions: none.

### Help menu

Extension points: reported, the whole file: owned by the other-dialogs area, which built it. Another area uses one of
its items: the session list and its host rail place the rail foot's `?` button (`HelpButton`, sidebar-and-session-view).
The table holds only whole paths, so the row covers the whole menu and the owner's own changes to it are reported too;
the review gate passes those that leave the shared item alone. Another area's change follows the pattern already there,
and the review gate judges it.

Lint exceptions: none.

### Trash button

Extension points: reported, the whole file: owned by the other-dialogs area, which built it. Another area uses one of
its items: the session list places the Trash button (`TrashButton`, sidebar-and-session-view). The table holds only
whole paths, so the row covers the whole Trash module root and the owner's own changes to it are reported too; the
review gate passes those that leave the shared item alone. Another area's change follows the pattern already there, and
the review gate judges it.

Lint exceptions: none.

### Templates dialog entry

Extension points: reported, the whole file: owned by the templates-and-quick-launch area, which built it. Other areas
use five of its items: the session list's Templates button opens the dialog and shows its help (`open`, `button_help`,
sidebar-and-session-view), its quick switcher names the quick-launch shortcut (`QUICK_LAUNCH_SHORTCUT_NAME`), and the
launcher opens the dialog on a template its Save as template just stored and hands a quick launch's new session over to
be named (`open_at`, `name_quick_launched`). The table holds only whole paths, so the row covers the whole module root
and the owner's own changes to it are reported too; the review gate passes those that leave the shared items alone.
Another area's change follows the pattern already there, and the review gate judges it.

Lint exceptions: none.

### Settings dialog

Extension points: reported, the whole file: owned by the settings-and-hosts area, which built it. Another area uses one
of its items: the host rail's gear opens Settings (`open`, sidebar-and-session-view). The table holds only whole paths,
so the row covers the whole dialog and the owner's own changes to it are reported too; the review gate passes those that
leave the shared item alone. Another area's change follows the pattern already there, and the review gate judges it.

Lint exceptions: none.

### Terminal cell metrics

Extension points: reported, the whole file: owned by the terminal area, which built it. Another area uses one of its
items: the session pane measures the terminal's cells (`CellMetrics`, sidebar-and-session-view). The table holds only
whole paths, so the row covers the whole terminal module root and the owner's own changes to it are reported too; the
review gate passes those that leave the shared item alone. Another area's change follows the pattern already there, and
the review gate judges it.

Lint exceptions: none.

### Session selection

Extension points: reported, the whole file: owned by the sidebar-and-session-view area, which built it, and shared with
the launcher, which selects the session a launch created (`choose`). Another area's change follows the pattern already
there, and the review gate judges it.

Lint exceptions: none.

### Session list

Extension points: reported, the whole file: owned by the sidebar-and-session-view area, which built it. Another area
uses one of its items: the status sounds read which view the listing answers (`SessionList::listing_view`,
other-dialogs), so that status changes seen while a host or status filter change is unanswered are recorded without a
sound. They rely on the list notifying when such an answer arrives, so the sounds see the view change. The templates
dispatcher returns the list from the Hosts panel after a quick launch from the page (`show_sessions`,
templates-and-quick-launch). The table holds only whole paths, so the row covers the whole session list module root and
the owner's own changes to it are reported too; the review gate passes those that leave the shared item alone. Another
area's change follows the pattern already there, and the review gate judges it.

Lint exceptions: none.

### Status sounds

Extension points: reported, the whole files: owned by the other-dialogs area, which built them, and shared with
settings-and-hosts, whose Settings → Notifications shows each sound's switch as the status sounds read it
(`rules::Switches`, for each `Sound`). Another area's change follows the pattern already there, and the review gate
judges it.

Lint exceptions: none.

### Session restart

Extension points: reported, the whole file: owned by the sidebar-and-session-view area, which built it, and shared with
the launcher, whose Restart with's reason for an uncaptured conversation ends with the same feedback invitation as the
Restart control's hover help (`feedback_note`). Another area's change follows the pattern already there, and the review
gate judges it.

Lint exceptions: none.

### Session menu

Extension points: reported, the whole file: owned by the sidebar-and-session-view area, which built it, and shared with
the terminal area, whose shell tab's × closes the tab without asking when the helm-wide Skip session confirmations
preference is on (`skips_questions`). Another area's change follows the pattern already there, and the review gate
judges it.

Lint exceptions: none.

### Session delete

Extension points: reported, the whole file: owned by the sidebar-and-session-view area, which built it, and shared with
the terminal area, whose New terminal button is refused while the session has a request in flight (`busy`). Another
area's change follows the pattern already there, and the review gate judges it.

Lint exceptions: none.

### Rail notices

Extension points: reported, the whole file: owned by the other-dialogs area, which built it, and shared with
sidebar-and-session-view, whose host rail holds and places the rail's notices bell (`NoticesButton`). Another area's
change follows the pattern already there, and the review gate judges it.

Lint exceptions: none.

### Launching

Extension points: reported, the whole file: owned by the launcher area, which built it, and shared with
sidebar-and-session-view, whose Restart and Replace give the same reason as Launch while a host change is in flight
(`HOST_CHANGE_IN_FLIGHT`, through `session_list::menu::launch_wait`). Another area's change follows the pattern already
there, and the review gate judges it.

Lint exceptions: none.

### Template shortcuts

Extension points: reported, the whole file: owned by the templates-and-quick-launch area, which built it, and shared
with other-dialogs, whose Keyboard shortcuts overview lists the ⚡ chord and the number keys from the templates that
hold them (`Bindings`, `KEYS`). Another area's change follows the pattern already there, and the review gate judges it.

Lint exceptions: none.

### Session rename

Extension points: reported, the whole file: owned by the sidebar-and-session-view area, which built it, and shared with
templates-and-quick-launch, whose quick launch opens the Rename editor in naming mode on the session it just created and
opened (`open_naming`). Another area's change follows the pattern already there, and the review gate judges it.

Lint exceptions: none.

### Template rows

Extension points: reported, the whole file: owned by the templates-and-quick-launch area, which built it as the template
search the template lists share, and shared with the launcher, whose `tl:` search lists and matches templates through it
(`rows`, `Row`, `matches`), and with sidebar-and-session-view, whose quick switcher lists templates and acts on a pick
through it (`template_query`, `rows`, `Row`, `pick_from_switcher`, `quick_launch_row_label`,
`quick_launch_from_switcher`), and whose templates-only mode of that switcher, the Cmd+J list, starts the chosen
template through it (`launch_from_list`). Another area's change follows the pattern already there, and the review gate
judges it.

Lint exceptions: none.

### Template dispatch

Extension points: reported, the whole file: owned by the templates-and-quick-launch area, which built it, and shared
with the launcher, whose `tl:` search applies a taken template row through it (`search_row_batch`). Another area's
change follows the pattern already there, and the review gate judges it.

Lint exceptions: none.

### Template checkout name

Extension points: reported, the whole file: owned by the templates-and-quick-launch area, which built it, and shared
with the launcher, whose Name field shows a template's base name as numbered if taken, and says when the base was taken
(`template_base`, `pill`, `taken_note`). Another area's change follows the pattern already there, and the review gate
judges it.

Lint exceptions: none.

### Shortcuts overview

Extension points: reported, the whole file: owned by the other-dialogs area, which built it as the Keyboard shortcuts
overview. Another area uses one of its items: the sidebar's quick switcher, in its Cmd+J mode, draws its ⚡ and
number-key chips as the overview does (`chip`, sidebar-and-session-view). The table holds only whole paths, so the row
covers the whole overview and the owner's own changes to it are reported too; the review gate passes those that leave
the shared item alone. Another area's change follows the pattern already there, and the review gate judges it.

Lint exceptions: none.

### Host status

Extension points: reported, the whole file: owned by the settings-and-hosts area, which built it as what the host rail
and the attention list read about each host, and shared with sidebar-and-session-view, which draws the host rail from it
(`host_status`, `HostStatus`, `Activity`, `Urgency`, `observe_changes`). Another area's change follows the pattern
already there, and the review gate judges it.

Lint exceptions: none.

## What is not a foundation

The rest of `crates/gui/farhelm-gui/src/` is area code, owned by the area that builds it and outside this table: the
session list (`session_list.rs` and `session_list/`) and the session pane (`session_pane.rs` and `session_pane/`) belong
to the sidebar-and-session-view area, the terminal (`terminal.rs` and `terminal/`) to the terminal area, and the desktop
integration (`desktop.rs` and `desktop/`) to the desktop-integration area. The reference PNGs under
`crates/gui/farhelm-gui/tests/goldens/` and the screenshot scene list (`docs/gpui/screenshot-scenes.json`) are outside
it too: an area adds its own scenes and references and updates them, with no request and no rule from the check.

Area-built modules other areas share are in the table all the same, so their changes to it are reported to the gate:
host identity (`hosts/identity.rs` and `hosts/identity/`), owned by settings-and-hosts; the harness answers
(`harness.rs`) and the permission shield's classification (`permission.rs`), both owned by sidebar-and-session-view; the
permission words (`permission/words.rs`), owned by the launcher; the provider wording (`environment_ui.rs`), owned by
approvals-and-agent-spawned-sessions; a template's summary (`templates/summary.rs`), owned by
templates-and-quick-launch; and the rows after it in the main table, each of whose subsections names its owner and the
areas that use it.

When an area starts importing a module it does not own, the notice it posts about that touch also files a reported row
for that module here, and the foundations owner builds the row. The migration's orchestrator decided this on 2026-11-14,
after host identity, `harness.rs`, `permission.rs` and `environment_ui.rs` each needed an ask of its own; until its row
lands, the check reports nothing about another area's edit to such a module. So that the notice is not forgotten, the
check prints as `reported` each module an area's file starts importing with `use crate::`, or starts naming with a
`crate::` path written out in its code, from another area's paths in the area table, unless an owned path of the main
table already covers it; it never refuses for one. A drift scan found about ten such imports without a row before the
check looked for them, and three written-out paths reached main without a row, which a check reading only `use` lines
would never have reported. Both forms count a module the file names directly: a file that imports a module whole
(`use crate::{launcher}`) and reaches a submodule through it (`launcher::entry::open_new`) is counted as using the
module itself, so a new submodule reached through an import the file already had is not reported at all.

The area table says which paths each area owns, for that check alone: it reads the second column's backticked paths,
which never overlap one another, though one may hold a foundation's owned path, as `hosts/` holds host identity, whose
row keeps it reported. The import check is advisory, so a malformed area table makes the check say that cross-area
imports were not checked, never stop, and an area path that names no file (an area may delete its own files without a
request) covers nothing; the script's tests require every area path to exist in the checkout. The check compares each
file's imports with the same path's at the base, so a file that is moved or split reports again every cross-area import
it already had without a row. A new top-level module an area adds is in no area, and its own imports go unchecked, until
someone adds it to the table; an area file that imports a module in no area's paths and no row is reported as
unassigned, so the orchestrator hears of it. An import from or of a test file is never reported (`*_tests.rs`,
`tests.rs`, or a file under a `tests/` directory), since an area's tests borrowing another's fixtures owe no row. The
map was confirmed by the migration's orchestrator on 2026-11-15.

| Area                                 | Area paths                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| sidebar-and-session-view             | `crates/gui/farhelm-gui/src/session_list.rs`, `crates/gui/farhelm-gui/src/session_list/`, `crates/gui/farhelm-gui/src/session_pane.rs`, `crates/gui/farhelm-gui/src/session_pane/`, `crates/gui/farhelm-gui/src/sidebar.rs`, `crates/gui/farhelm-gui/src/sidebar/`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| terminal                             | `crates/gui/farhelm-gui/src/terminal.rs`, `crates/gui/farhelm-gui/src/terminal/`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| launcher                             | `crates/gui/farhelm-gui/src/launcher.rs`, `crates/gui/farhelm-gui/src/launcher/`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| templates-and-quick-launch           | `crates/gui/farhelm-gui/src/templates.rs`, `crates/gui/farhelm-gui/src/templates/`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| settings-and-hosts                   | `crates/gui/farhelm-gui/src/settings.rs`, `crates/gui/farhelm-gui/src/settings/`, `crates/gui/farhelm-gui/src/hosts.rs`, `crates/gui/farhelm-gui/src/hosts/`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| desktop-integration                  | `crates/gui/farhelm-gui/src/desktop.rs`, `crates/gui/farhelm-gui/src/desktop/`, `crates/gui/farhelm-gui/src/update_readout.rs`, `crates/gui/farhelm-gui/src/update_readout/`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| approvals-and-agent-spawned-sessions | `crates/gui/farhelm-gui/src/approvals.rs`, `crates/gui/farhelm-gui/src/approvals/`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| other-dialogs                        | `crates/gui/farhelm-gui/src/announcements.rs`, `crates/gui/farhelm-gui/src/announcements/`, `crates/gui/farhelm-gui/src/welcome.rs`, `crates/gui/farhelm-gui/src/welcome/`, `crates/gui/farhelm-gui/src/trash.rs`, `crates/gui/farhelm-gui/src/trash/`, `crates/gui/farhelm-gui/src/feedback.rs`, `crates/gui/farhelm-gui/src/feedback/`, `crates/gui/farhelm-gui/src/help_menu.rs`, `crates/gui/farhelm-gui/src/help_menu/`, `crates/gui/farhelm-gui/src/notices_bell.rs`, `crates/gui/farhelm-gui/src/notices_bell/`, `crates/gui/farhelm-gui/src/rail_notices.rs`, `crates/gui/farhelm-gui/src/rail_notices/`, `crates/gui/farhelm-gui/src/sounds.rs`, `crates/gui/farhelm-gui/src/sounds/`, `crates/gui/farhelm-gui/src/shortcuts.rs`, `crates/gui/farhelm-gui/src/shortcuts/` |

## Extension point rules

The landing-step check knows three rules. `new-files` sits on a directory and admits only files the diff adds under it;
changing or deleting a file already there is refused. `append-only` sits on a file and admits a change that adds at
least one line and removes none, so a changed line (a removal plus an addition), a binary change, a mode change on its
own and the file's deletion are refused. `reported` sits on a file or a directory and is the reported tier: it admits
any change but a deletion and prints it as `reported`, which never fails the check. A deletion is refused because the
table names those files, or other areas build on them. A rename counts as the deletion of its old path and the addition
of its new one. Where one extension point sits inside another, the more specific one decides. An owned path no rule
covers is in the refused tier with no extension point.

A rule admits a shape of change, not its content. `append-only` on `operations.rs` cannot tell a new `Work` variant from
any other added line, and `reported` cannot tell wiring from a rule change at all, so the review gate checks that every
admitted change follows the pattern the table describes. One that does not is a foundation-change request even though
the check let it through.

The Host text and Drafts extension points have no row here, because they are uses of the foundation from area code (a
view calling `HostText::from_host`, an editor holding a `Draft`) and touch no owned path. The Test harness's reference
PNGs are not owned, so only the golden scene in `goldens.rs` needs a rule.

NOTE: An append-only file only works as an extension point when an addition to it leaves every existing line alone, so
`HarnessGlyph::ALL` in `marks.rs` is a slice with one entry per line. Two traps refused real PRs before the wiring was
reported, and they still apply to the append-only files that remain (`operations.rs`, `notices.rs`, `marks.rs`,
`test_support.rs`): extending a grouped `use` statement rewraps it, so an area adds a separate `use` line instead, which
rustfmt leaves alone; and adding to a wrapped doc paragraph rewraps it too, so an area adds its own sentence or doc line
instead. In a reported file both are fine. The lists areas append to there (`Icon::ALL` in `assets.rs`, `SCENES` in
`goldens.rs`, `ONCE_PER_PRESS` in `actions.rs`) stay slices with one entry per line all the same: a length in their type
changed with every addition, and during two rebases jj's automatic merge left the golden scenes' length wrong without a
conflict marker.

| Foundation             | Extension path                                          | Rule          |
| ---------------------- | ------------------------------------------------------- | ------------- |
| Bootstrap              | `crates/gui/farhelm-gui/src/lib.rs`                     | `reported`    |
| Bootstrap              | `crates/gui/farhelm-gui/Cargo.toml`                     | `reported`    |
| Theme                  | `crates/gui/farhelm-gui/src/theme.rs`                   | `reported`    |
| Component layer        | `crates/gui/farhelm-gui/src/components/`                | `reported`    |
| Actions and keymap     | `crates/gui/farhelm-gui/src/actions.rs`                 | `reported`    |
| Actions and keymap     | `crates/gui/farhelm-gui/src/actions_tests.rs`           | `reported`    |
| Operation store        | `crates/gui/farhelm-gui/src/operations.rs`              | `append-only` |
| Notice center          | `crates/gui/farhelm-gui/src/notices.rs`                 | `append-only` |
| Test harness           | `crates/gui/farhelm-gui/src/goldens.rs`                 | `reported`    |
| Test harness           | `crates/gui/farhelm-gui/src/test_support.rs`            | `append-only` |
| Assets                 | `crates/gui/farhelm-gui/src/assets.rs`                  | `reported`    |
| Assets                 | `crates/gui/farhelm-gui/assets/icons/`                  | `new-files`   |
| Main view              | `crates/gui/farhelm-gui/src/main_view.rs`               | `reported`    |
| Harness marks          | `crates/gui/farhelm-gui/src/marks.rs`                   | `append-only` |
| Harness marks          | `crates/gui/farhelm-gui/src/marks/`                     | `new-files`   |
| Test-automation bridge | `scripts/gui-agent.py`                                  | `reported`    |
| Test-automation bridge | `scripts/test-gui-agent.py`                             | `reported`    |
| Test-automation bridge | `docs/gui-agent.md`                                     | `reported`    |
| Test-automation bridge | `scripts/gui-screenshot-batch.py`                       | `reported`    |
| Test-automation bridge | `scripts/test-gui-screenshot-batch.py`                  | `reported`    |
| Test-automation bridge | `scripts/gui-e2e-smoke.py`                              | `reported`    |
| Test-automation bridge | `scripts/test-gui-e2e-smoke.py`                         | `reported`    |
| Host identity          | `crates/gui/farhelm-gui/src/hosts/identity.rs`          | `reported`    |
| Host identity          | `crates/gui/farhelm-gui/src/hosts/identity/`            | `reported`    |
| Harness answers        | `crates/gui/farhelm-gui/src/harness.rs`                 | `reported`    |
| Permission shield      | `crates/gui/farhelm-gui/src/permission.rs`              | `reported`    |
| Permission words       | `crates/gui/farhelm-gui/src/permission/words.rs`        | `reported`    |
| Provider wording       | `crates/gui/farhelm-gui/src/environment_ui.rs`          | `reported`    |
| Template summary       | `crates/gui/farhelm-gui/src/templates/summary.rs`       | `reported`    |
| Save as template       | `crates/gui/farhelm-gui/src/templates/save.rs`          | `reported`    |
| Save as template       | `crates/gui/farhelm-gui/src/templates/save_panel.rs`    | `reported`    |
| Terminal tunnels       | `crates/gui/farhelm-gui/src/terminal/tunnels.rs`        | `reported`    |
| Terminal text size     | `crates/gui/farhelm-gui/src/terminal/text_size.rs`      | `reported`    |
| Terminal set           | `crates/gui/farhelm-gui/src/terminal/set.rs`            | `reported`    |
| Add host dialog        | `crates/gui/farhelm-gui/src/hosts/add.rs`               | `reported`    |
| Hosts panel            | `crates/gui/farhelm-gui/src/hosts.rs`                   | `reported`    |
| Host offers            | `crates/gui/farhelm-gui/src/hosts/offers.rs`            | `reported`    |
| Approval region        | `crates/gui/farhelm-gui/src/approvals/region.rs`        | `reported`    |
| Approval answers       | `crates/gui/farhelm-gui/src/approvals/answers.rs`       | `reported`    |
| Approval answers       | `crates/gui/farhelm-gui/src/approvals/host_add.rs`      | `reported`    |
| Title band             | `crates/gui/farhelm-gui/src/desktop/title_band.rs`      | `reported`    |
| Updater window side    | `crates/gui/farhelm-gui/src/desktop/updates.rs`         | `reported`    |
| Launcher entry         | `crates/gui/farhelm-gui/src/launcher/entry.rs`          | `reported`    |
| Launcher view          | `crates/gui/farhelm-gui/src/launcher/view.rs`           | `reported`    |
| Launch draft           | `crates/gui/farhelm-gui/src/launcher/draft.rs`          | `reported`    |
| Launch models          | `crates/gui/farhelm-gui/src/launcher/model.rs`          | `reported`    |
| Session replace        | `crates/gui/farhelm-gui/src/session_list/replace.rs`    | `reported`    |
| Operation outcomes     | `crates/gui/farhelm-gui/src/session_list/outcome.rs`    | `reported`    |
| Session age            | `crates/gui/farhelm-gui/src/session_list/age.rs`        | `reported`    |
| Host settings          | `crates/gui/farhelm-gui/src/hosts/settings.rs`          | `reported`    |
| SSH help               | `crates/gui/farhelm-gui/src/hosts/ssh_help.rs`          | `reported`    |
| Open-source notices    | `crates/gui/farhelm-gui/src/desktop/licenses.rs`        | `reported`    |
| Version readout view   | `crates/gui/farhelm-gui/src/update_readout/view.rs`     | `reported`    |
| Trash cue              | `crates/gui/farhelm-gui/src/trash/cue.rs`               | `reported`    |
| Notices bell           | `crates/gui/farhelm-gui/src/notices_bell.rs`            | `reported`    |
| Help menu              | `crates/gui/farhelm-gui/src/help_menu.rs`               | `reported`    |
| Trash button           | `crates/gui/farhelm-gui/src/trash.rs`                   | `reported`    |
| Templates dialog entry | `crates/gui/farhelm-gui/src/templates.rs`               | `reported`    |
| Settings dialog        | `crates/gui/farhelm-gui/src/settings.rs`                | `reported`    |
| Terminal cell metrics  | `crates/gui/farhelm-gui/src/terminal.rs`                | `reported`    |
| Session selection      | `crates/gui/farhelm-gui/src/session_list/selection.rs`  | `reported`    |
| Session list           | `crates/gui/farhelm-gui/src/session_list.rs`            | `reported`    |
| Status sounds          | `crates/gui/farhelm-gui/src/sounds.rs`                  | `reported`    |
| Status sounds          | `crates/gui/farhelm-gui/src/sounds/rules.rs`            | `reported`    |
| Session restart        | `crates/gui/farhelm-gui/src/session_list/restart.rs`    | `reported`    |
| Session menu           | `crates/gui/farhelm-gui/src/session_list/menu.rs`       | `reported`    |
| Session delete         | `crates/gui/farhelm-gui/src/session_list/delete.rs`     | `reported`    |
| Rail notices           | `crates/gui/farhelm-gui/src/rail_notices.rs`            | `reported`    |
| Launching              | `crates/gui/farhelm-gui/src/launcher/launch.rs`         | `reported`    |
| Template shortcuts     | `crates/gui/farhelm-gui/src/templates/shortcuts.rs`     | `reported`    |
| Session rename         | `crates/gui/farhelm-gui/src/session_list/rename.rs`     | `reported`    |
| Template rows          | `crates/gui/farhelm-gui/src/templates/rows.rs`          | `reported`    |
| Template dispatch      | `crates/gui/farhelm-gui/src/templates/dispatch.rs`      | `reported`    |
| Template checkout name | `crates/gui/farhelm-gui/src/templates/checkout_name.rs` | `reported`    |
| Shortcuts overview     | `crates/gui/farhelm-gui/src/shortcuts.rs`               | `reported`    |
| Host status            | `crates/gui/farhelm-gui/src/hosts/status.rs`            | `reported`    |
