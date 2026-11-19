//! The Settings dialog: the helm's shared choices, grouped in sections, opened from the gear at the foot of the host
//! rail.
//!
//! SPEC.md, "Session list" (the paragraph that starts "The gear at the foot of the host rail opens a settings dialog").
//! Ported from the old app's `settings.rs` (`SettingsDialog`) with its header from `hosts/settings_dialog.rs`
//! (`HostDialogHeader`) and its layout from the `.app-settings-*` and `.host-dialog-framed` rules in `app.css`, all at
//! reference commit `24bc0580`. Here's the TLDR of its rules:
//!
//! - **The frame.** The host dialogs' framed header (a gear tile, the title "Settings", the summary, and its own header
//!   close control, "Close settings") over a body that scrolls, and a pinned footer with its note and Close. It is
//!   760 pixels wide and 780 tall, capped by the window, whatever section is shown, so switching sections never moves
//!   the frame or its close controls (`.app-settings-dialog[aria-label="Settings"]`).
//! - **The sections.** A list of section buttons on the left, each with its icon and full label and the solid selection
//!   fill on the one shown, and the shown section beside it under a heading of the same name. Only the sections this
//!   build has are listed, in SPEC.md's order ([`Section::ALL`]), and App only while the desktop app's updater runs
//!   ([`Section::listed`]). Each section's state lives in the dialog for as long as it is open, so a hidden section
//!   keeps its pending saves, failure lines and unsaved text (Checkouts' fields); only the shown one is drawn, so Tab
//!   never reaches a hidden control.
//! - **Opening and closing.** [`open`] is the gear's entry point. It opens at Sessions with focus on its first switch
//!   (SPEC.md: "The gear always opens sessions and focuses its first switch"). [`open_at_agent_trust`] is a trust
//!   hint's: it opens at Agent trust and puts focus on that agent's switch once the switch can take it. Escape, the
//!   header's close control and the footer's Close all close it, and focus goes back to the gear. Closing discards what
//!   the dialog holds; a save already sent goes on, since it is the operation store's work, not the dialog's (the
//!   helm's reply then settles nothing on screen, and the store already shows the result). Every close sets
//!   [`Closings`], for the trust hint band, which reads trust again then.
//! - **Saving a choice.** Every switch here, Appearance's choice and Terminal's link choice save one field of the
//!   helm's preference row at once, with [`crate::drafts::save`] under [`OpKey::preferences`]; Agent trust's trust
//!   switches and Checkouts' spare switch save the same way through their own routes ([`configured`]). While the save
//!   is in flight the control shows the value being saved (its draft); otherwise it shows what the helm stores, so a
//!   refused save shows the stored value again, with the failure line under the control until its next save starts
//!   (SPEC.md: "A failed save restores the last known value and shows an adjacent error"). Nothing is optimistic,
//!   queued, retried or read back. Checkouts' folder and post-clone command are the exception: each is a text field the
//!   user saves explicitly, a [`CheckoutField`] of its own. It discards a refused draft and reads back after either
//!   a done or refused save ([`checkout_field`]). So are Notifications' ntfy alerts: a form the user saves
//!   explicitly, a view of its own ([`ntfy`]). So is Terminal's file-types field, which saves its preference only when
//!   the user presses Save, keeping a refused or malformed draft ([`file_types`]). So is Credentials, whose named
//!   secrets each save under their own key, with the Tensorlake API key form above them ([`credentials`],
//!   [`tensorlake_key`]). Tensorlake draws a second copy of that form over the same account, so a save in either shows
//!   in both, and saves its create-without-asking switch through the account's route, showing only the choice the helm
//!   stored ([`tensorlake`]). Recipes lists the helm's recipes, whose snapshot builds are accepted work under each
//!   recipe's own key, and edits one in place, saving the whole recipe under that key ([`recipes`]); while Recipes
//!   shows an open editor, the dialog pins the editor's Cancel and Save above its Close row, so they stay in view below
//!   a long form and go when another section is shown.
//!   Terminal's tunnel switch saves like a switch only when turned off; turned on, it asks the terminal's tunnel
//!   warning, whose Allow saves the consent, and shows the stored choice until the helm pushes it ([`terminal`]). App's
//!   switch is not the helm's at all: it writes the desktop app's own setting through its updater, at once and with a
//!   final answer, so it has no draft and never holds the preference row's key ([`app`]). Containers saves no choice at
//!   all: it shows what the helm reads from its container hosts, and its Refresh and confirmed Deletes are plain
//!   commands, not operations on a key ([`containers`]).
//!
//! NOTE: Quick flips do not follow the old app's "newest-choice" rule. Its preference queue coalesced a field's clicks,
//! sent only the newest, and reported a failure only for the newest click. Here the operation store admits one request
//! on the preference row's key at a time, and every Settings save holds that key, so a press on any control while any
//! Settings save is in flight sends nothing: the control goes on showing what it showed, with a line beside it saying
//! that it was not saved because another operation was in progress ([`refused`]), and the next press after the reply
//! acts. On the radios (Appearance, Terminal's link choice) an arrow press moves focus before it reports the choice, so
//! a refused arrow press still moves focus while the choice stays where it was. The helm is embedded and answers a
//! preference write in milliseconds, so the window in which a press is refused this way is that short. The migration's
//! orchestrator accepted the refusal on 2026-11-14, to be revisited only if a user meets it, and asked the same day
//! that it say why; closing it would take a key per field or a queue, both changes to the operation store, not here.
//!
//! NOTE: Not ported yet, each recorded in `docs/gpui/parity/settings-and-hosts.md`. The one-pane layout at 620 pixels
//! wide and below ("‹ Settings" back to the list) is not built, since the window's minimum width is above it. A
//! section switch does not move focus to the shown section's heading, as the old app did so a keyboard user went on
//! from there: the focus coordinator moves focus to a control only while no layer is open, and the heading is not a
//! control. The section list is not pinned while a long section scrolls (the old `position: sticky`). Checkouts is
//! taller than the dialog's body even at the dialog's full height, by about 30 pixels, so its body scrolls: unscrolled,
//! its last control, the docs link, sits at the body's visible edge with its descenders and underline below it, and
//! scrolled to the end it lies wholly above the footer. The header is taller than the old one, and the docs link has a
//! line of its own.

mod agent_trust;
mod app;
mod appearance;
mod checkout_field;
mod checkouts;
mod choices;
mod configured;
mod containers;
#[cfg(test)]
mod containers_tests;
#[cfg(test)]
mod credential_tests;
mod credentials;
mod file_types;
#[cfg(test)]
mod file_types_tests;
mod ntfy;
#[cfg(test)]
mod recipe_credential_tests;
#[cfg(test)]
mod recipe_editor_tests;
#[cfg(test)]
mod recipe_leave_tests;
#[cfg(test)]
mod recipe_place_tests;
#[cfg(test)]
mod recipe_tests;
mod recipes;
/// A recipe's one-line summary (size, base, packages, tunnels), which Add host's Tensorlake sandbox shows under its Recipe
/// choice as Recipes shows it under the recipe's name, so one recipe never has two descriptions.
pub(crate) use recipes::summary as recipe_summary;
/// The recipe editor, which Add host's Tensorlake sandbox also hosts for its "New recipe…" ([`recipes::Host`]).
pub(crate) use recipes::{
    Departed as RecipeDeparted, Departure as RecipeDeparture, Editor as RecipeEditor,
};
mod refused;
mod search;
mod tensorlake;
mod tensorlake_key;
#[cfg(test)]
mod tensorlake_sandbox_tests;
mod tensorlake_sandboxes;
mod tensorlake_ssh_key;
#[cfg(test)]
mod tensorlake_tests;
mod terminal;
#[cfg(test)]
mod tests;

use std::rc::Rc;

use farhelm_helm_client::PathError;
use farhelm_input_mapping::Platform;
use farhelm_proto::ui_channel::{PREFERENCES_KEY, Preferences};
use gpui::{
    AnyElement, AnyWindowHandle, App, AppContext as _, Context, Entity, FocusHandle, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Role,
    StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity, Window, div,
    prelude::FluentBuilder as _, px, svg,
};

use crate::assets::Icon;
use crate::components::{Button, DialogFrame, HoverHelp, Tier};
use crate::drafts::{self, Draft};
use crate::focus::{self, Mark, ReturnTo};
use crate::layers::{self, Dialog, Dismissal, LayerId};
use crate::operations::{Command, OpKey, Outcome};
use crate::session;
use crate::theme::{self, Preference};

use agent_trust::Trust;
use choices::{Choice, Switch};
use configured::Configured;
use search::{Search, Target};
use tensorlake_key::{Place, TensorlakeAccount, TensorlakeKey};

/// What the session view's trust hint band says for an agent, kept with the rest of that agent's trust vocabulary.
pub(crate) use agent_trust::BandWords;
pub use agent_trust::TrustAgent;
pub use checkout_field::{CheckoutField, CheckoutUpdated, FolderScope};
/// The one way the GUI asks the helm for Podman checks, for the launcher too ([`containers::check_container_hosts`]).
pub(crate) use containers::check_container_hosts;
/// The line saying why a press sent nothing, shared with the host settings dialog, whose saves share a host's key the way
/// Settings' share the preference row's.
pub(crate) use refused::Refused;
use terminal::{LinkClick, TunnelConsent};

/// Hands the helm's stored appearance to the theme, for the main view's subscription to the current UI session. It
/// lives with the section that saves the choice, so the start hook's rule and the reading of the row's word are area
/// code, not shared wiring.
pub(crate) use appearance::follow_stored as follow_stored_appearance;

/// `.app-settings-dialog[aria-label="Settings"]`'s width.
const WIDTH: f32 = 760.;
/// Its height, capped by the window: what the old Checkouts section needed, kept for every section.
const HEIGHT: f32 = 780.;
/// `.app-settings-navigation`'s column: 168 pixels, its 12-pixel right padding and its edge included.
const NAV_WIDTH: f32 = 168.;
/// The search shortcut is scoped to the Settings frame, including its header and footer.
pub(crate) const SEARCH_CONTEXT: &str = "Settings";
/// `.app-settings-panel > h3`: the shown section's heading size.
const HEADING_TEXT: f32 = 18.;
/// The heading's line: `line-height: normal` at 18 pixels; GPUI's default line is taller.
const HEADING_LINE_HEIGHT: f32 = 22.;
/// `.host-settings-section`'s gap between a section's heading and its groups.
const SECTION_GAP: f32 = 14.;

/// The header's summary where App is listed: the old dialog's words, true there because App's switch is this device's
/// own setting while every other choice is the helm's.
///
/// NOTE: "and for this device" described the old app, which kept appearance and the status sounds per device. In this
/// app they are helm preferences (the maintainer's decision of 2026-11-13), so only the desktop app's automatic-update
/// choice (Settings → App) is this device's.
const SUMMARY_WITH_APP: &str = "For every host on this helm, and for this device";

/// The header's summary where App is not listed (no updater runs: Linux, and dev or agent builds), where nothing in the
/// dialog belongs to this device. The migration's orchestrator decided on 2026-11-15 that the words follow whether App
/// is listed, dropping "and for this device" without it. The old app at reference commit `24bc0580` had only the one
/// summary, so these are its words cut short rather than words of their own.
const SUMMARY: &str = "For every host on this helm";

/// The footer's note, the old desktop app's words. It names the dialog's explicit-save editors.
const FOOTER_NOTE: &str = "Most choices save immediately; credentials, the Tensorlake API key, ntfy alerts, checkout \
                           fields and file types use Save.";

/// Both controls use the recipe editor's leave question when it guards a close.
const CLOSE_HELP: &str =
    "Close: asks first about unsaved recipe edits and discards other unsaved fields";

const HOST_CHOICES_HELP: &str = "The host choices apply to every host on this helm. Changes from the command line or \
                                 an agent show here right away.";
const SOUNDS_ID: &str = "settings-sounds";
const SOUNDS_LABEL: &str = "Sounds";
const SOUNDS_HELP: &str = "These choices apply to this helm; changes from the command line or an agent show here \
                           right away. The session open in an active window stays quiet.";

/// Open Settings at Sessions with focus on its first switch, on top of everything else in `window`: the gear's entry
/// point (SPEC.md: "The gear always opens sessions and focuses its first switch").
///
/// `opener` is the gear's focus handle and `mark` the [`Mark`] taken in its click handler. Settings names the gear as
/// where closing returns focus ([`ReturnTo::Control`]), so it goes back there on every platform, also where a click
/// leaves a button unfocused (macOS): the old app returned focus to the gear explicitly (`RETURN_FOCUS`), and SPEC.md
/// has "Escape and either close control return focus to the gear". Nothing opens when the user has moved on since
/// `mark` or the window is inert.
///
/// Returns the dialog's layer, or `None` when nothing opened.
pub fn open(
    opener: &FocusHandle,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    open_at(Section::Sessions, opener, mark, window, cx)
}

/// [`open`], at `section` with focus on its first control (on the dialog itself at Appearance, Terminal and Agent
/// trust, [`Section::first`]). The screenshot goldens call it for each section they picture; [`open_at_agent_trust`]
/// opens through [`open_dialog`], the form that also hands back the dialog.
pub(crate) fn open_at(
    section: Section,
    opener: &FocusHandle,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    open_dialog(section, opener, mark, window, cx).map(|(layer, _)| layer)
}

/// [`open_at`], handing back the dialog too, for an entry point that tells it more.
fn open_dialog(
    section: Section,
    opener: &FocusHandle,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<(LayerId, Entity<SettingsDialog>)> {
    // An inert window has no store to draw the switches from, and nothing to save through.
    session::current_store(cx)?;
    if focus::has_moved_on(mark, cx) {
        return None;
    }
    let dialog = cx.new(|cx| SettingsDialog::new(section, window, cx));
    let container = cx.focus_handle();
    let start = dialog
        .read(cx)
        .start(section, cx)
        .unwrap_or_else(|| container.clone());
    let layer = layers::open_dialog_with_shortcut(
        Dialog {
            view: dialog.clone().into(),
            container: container.clone(),
            start,
            subject: None,
            // None on purpose: Enter on the dialog itself does nothing, as in the old app, where container Enter never
            // toggled a choice (`settings.spec.ts`, "settings gap keeps focus while text and container Enter keep their
            // own behavior").
            main_action: None,
            // SPEC.md lists the dialogs that share the Settings installer among those that keep focus on the control in
            // use, and the old Settings did (`DialogFocus::remembered`, whose gap rule is on).
            keeps_focus_on_empty_click: true,
            returns_to: ReturnTo::Control(opener.clone()),
            // Asked on Escape and the close controls ([`SettingsDialog::dismissal`]). Otherwise closing never waits: a
            // save in flight goes on in the operation store after the dialog is gone.
            on_dismiss: Some(Rc::new({
                let dialog = dialog.downgrade();
                move |window: &mut Window, cx: &mut App| {
                    dialog
                        .update(cx, |dialog, cx| dialog.dismissal(window, cx))
                        .unwrap_or(Dismissal::Close)
                }
            })),
            placement: layers::DialogPlacement::Centered,
            dismiss_on_outside_press: false,
        },
        SEARCH_CONTEXT,
        crate::actions::SearchSettings,
        Rc::new({
            let dialog = dialog.downgrade();
            move |window, cx| {
                let _ = dialog.update(cx, |dialog, cx| dialog.search.focus(window, cx));
            }
        }),
        mark,
        window,
        cx,
    )?;
    dialog.update(cx, |dialog, cx| {
        dialog.layer = Some(layer);
        dialog.show(section, window, cx);
    });
    #[cfg(test)]
    layers::record_opened(&dialog, cx);
    Some((layer, dialog))
}

/// Set each time a Settings dialog is released, however it closed (Escape, a close control, a reset's `close_all`), so
/// observing it is how a view learns that Settings closed: the trust hint band reads its agent's trust again then,
/// since the helm does not push trust and the user may have changed it in Settings, whichever entry point opened it
/// (`crate::session_pane::trust_band`). A dialog whose opening was refused is released too, which costs the band one
/// more read.
pub(crate) struct Closings;

impl gpui::Global for Closings {}

/// Open Settings at Agent trust with focus on `agent`'s switch: a trust hint band's Settings button (SPEC.md: "Trust
/// hints instead open agent trust at the relevant switch and scroll its block into view"). `opener` and `mark` are as
/// [`open`]'s: the hint's button and the mark taken in its click handler.
///
/// The switch is unavailable until the section's read of the agent's trust answers ([`agent_trust`]), and an
/// unavailable control cannot hold focus, so the dialog opens with focus on itself and moves focus to the switch once
/// the read has answered, scrolling the switch into view ([`SettingsDialog::trust_known`]). That matters because only
/// Claude's switch is always visible when the dialog opens: Muse's and Pi's lie below the fold at the dialog's full
/// height, and Codex's too when the window caps the dialog's height. Nothing is asked of the focus coordinator until
/// the read answers, so until then only the user moving on since `mark` (any key press or click, a click that leaves
/// focus on the dialog included) or focus leaving the dialog stops the move. Once the read has answered, the move waits
/// in the coordinator for the frame that draws the switch available, and any other focus move made before it lands
/// drops it. If the read fails, focus stays on the dialog, beside the failure line.
///
/// NOTE: The reveal shows the switch, not the agent's whole block that SPEC.md's "scroll its block into view" names:
/// for a switch that started below the fold, the help under it (and Muse's hint switch) can stay below the fold. This
/// is accepted as it stands: revealing the whole block would take a change in the focus coordinator, and none is
/// requested (`docs/gpui/parity/settings-and-hosts.md`, "Deviations").
///
/// NOTE: Trust is not pushed. A hint band that shows only while its agent's trust is off (SPEC.md) does not learn from
/// this function that the user turned trust on in Settings: it returns only the layer. As the old band did, the
/// session view's band reads trust again when Settings closes, by observing [`Closings`].
pub fn open_at_agent_trust(
    agent: TrustAgent,
    opener: &FocusHandle,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let (layer, dialog) = open_dialog(Section::AgentTrust, opener, mark, window, cx)?;
    // A fresh dialog has just sent its reads, so the switch is not available yet: focus waits for the answer.
    let handover = Handover {
        target: Target::Trust(agent),
        mark: *mark,
        window: window.window_handle(),
    };
    dialog.update(cx, |dialog, _| dialog.focus_on = Some(handover));
    Some(layer)
}

/// `PUT /api/preferences` turning `agent`'s "Show the hint when ... asks about a managed checkout" off, exactly as its
/// switch here saves it: for the trust hint band's Don't show again, which stores the same shared dismissal. `None` for
/// an agent without a hint (Pi).
pub(crate) fn hide_trust_hint_command(
    agent: TrustAgent,
) -> Option<Result<crate::operations::Command, farhelm_helm_client::PathError>> {
    agent.hint().map(|choice| choice.command(false))
}

/// Whether the pushed preference row lets `agent`'s trust hint show, by the rule its switch here reads (shown unless
/// the dismissal is stored `true`): for the trust hint band, so the two never disagree on an unset field. Never for an
/// agent without a hint (Pi).
pub(crate) fn trust_hint_shown(agent: TrustAgent, cx: &App) -> bool {
    let store = session::current_store(cx);
    let collections = store.as_ref().map(|store| store.read(cx).collections());
    let preferences = collections.and_then(|all| all.get::<Preferences>(PREFERENCES_KEY));
    agent
        .hint()
        .is_some_and(|choice| choice.stored(preferences))
}

/// `PUT /api/preferences` turning "Set up new hosts without asking" on, exactly as its switch here saves it: for the
/// host setup confirmation's permanent answer, which sets the same preference.
pub(crate) fn skip_host_setup_command()
-> Result<crate::operations::Command, farhelm_helm_client::PathError> {
    Choice::SkipHostSetupConfirmation.command(true)
}

/// `PUT /api/preferences` turning "Remove hosts without asking" on, exactly as its switch here saves it: for the host
/// removal confirmation's permanent answer, which sets the same preference.
pub(crate) fn skip_host_remove_command()
-> Result<crate::operations::Command, farhelm_helm_client::PathError> {
    Choice::SkipHostRemoveConfirmation.command(true)
}

/// `PUT /api/preferences` turning "Update remote hosts automatically" on, exactly as its switch here saves it: for the
/// attention list's Always update and its Try again ([`crate::hosts::status::always_update`]), which set the same
/// preference.
pub(crate) fn auto_update_command()
-> Result<crate::operations::Command, farhelm_helm_client::PathError> {
    Choice::AutoUpdateRemoteHosts.command(true)
}

/// Decide the open Settings dialog's tunnel switch for `platform` ([`SettingsDialog::decide_tunnels_for`]), for a
/// golden that pictures the Mac's Terminal section on any machine. Reaches the dialog through the handle the dialog's
/// opening records for tests. Panics when no dialog is open: called too early, it would otherwise leave the switch
/// decided for this machine's platform, and a test of that same platform would pass without testing anything.
#[cfg(test)]
pub(crate) fn decide_tunnels_for_opened(platform: Platform, cx: &mut App) {
    match layers::last_opened::<SettingsDialog>(cx) {
        Some(dialog) => dialog.update(cx, |dialog, cx| dialog.decide_tunnels_for(platform, cx)),
        None => panic!("no Settings dialog is open to decide its tunnel switch for"),
    }
}

/// Show a settled search in a golden without waiting for synthetic keyboard events before the first paint.
#[cfg(test)]
pub(crate) fn search_for_opened(query: &str, window: &mut Window, cx: &mut App) {
    let dialog =
        layers::last_opened::<SettingsDialog>(cx).expect("Settings is open for its golden");
    dialog.update(cx, |dialog, cx| {
        dialog.search.set_query(query, window, cx);
        dialog.search.focus(window, cx);
        cx.notify();
    });
}

/// Open a new recipe's editor in the open Settings dialog's Recipes section, as New recipe does, for the golden that
/// pictures the editor. Panics when no dialog is open or Recipes was never shown, which would leave the golden picturing
/// the list instead.
#[cfg(test)]
pub(crate) fn open_new_recipe_for_opened(window: &mut Window, cx: &mut App) {
    let recipes = layers::last_opened::<SettingsDialog>(cx)
        .and_then(|dialog| dialog.read(cx).recipes.clone());
    match recipes {
        Some(recipes) => recipes.update(cx, |recipes, cx| recipes.open_editor(None, window, cx)),
        None => panic!("no Settings dialog shows Recipes to open the recipe editor in"),
    }
}

/// The helm's preference row, if it pushed one.
fn preferences(collections: &farhelm_helm_client::Collections) -> Option<&Preferences> {
    collections.get::<Preferences>(PREFERENCES_KEY)
}

/// A section of the dialog.
///
/// Adding one is local: a variant here, its entry in [`Section::ALL`] at its place in SPEC.md's order, its key, label
/// and icon, its body in [`SettingsDialog::body`], and whatever state it keeps in [`SettingsDialog`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Section {
    Sessions,
    Appearance,
    Hosts,
    Recipes,
    /// The helm's Tensorlake account: its API key and whether sandboxes are created without asking ([`tensorlake`]).
    Tensorlake,
    /// Whether each container host can run containers, and the containers this helm left behind ([`containers`]).
    Containers,
    Credentials,
    Terminal,
    Checkouts,
    AgentTrust,
    Notifications,
    /// The desktop app's own automatic-updates switch, listed only while its updater runs ([`Section::listed`]).
    App,
}

impl Section {
    /// The sections, in SPEC.md's order ("Sessions, Appearance, Hosts, Recipes, Tensorlake, Containers, Credentials,
    /// Terminal, Checkouts, Agent trust and Notifications, followed by App").
    const ALL: &[Section] = &[
        Section::Sessions,
        Section::Appearance,
        Section::Hosts,
        Section::Recipes,
        Section::Tensorlake,
        Section::Containers,
        Section::Credentials,
        Section::Terminal,
        Section::Checkouts,
        Section::AgentTrust,
        Section::Notifications,
        Section::App,
    ];

    /// Whether the dialog lists this section in this app: every section in [`Section::ALL`], except App, which only a
    /// desktop app whose updater runs has (SPEC.md: "followed by **App** only in a desktop app whose updater is
    /// active"). The updater's own answer decides, so no platform or build is checked here.
    fn listed(self, cx: &App) -> bool {
        match self {
            Section::App => crate::desktop::updates::runs(cx),
            Section::Sessions
            | Section::Appearance
            | Section::Hosts
            | Section::Recipes
            | Section::Tensorlake
            | Section::Containers
            | Section::Credentials
            | Section::Terminal
            | Section::Checkouts
            | Section::AgentTrust
            | Section::Notifications => true,
        }
    }

    /// The section's stable key, which its button's element id is built from.
    fn key(self) -> &'static str {
        match self {
            Section::Sessions => "sessions",
            Section::Appearance => "appearance",
            Section::Hosts => "hosts",
            Section::Recipes => "recipes",
            Section::Tensorlake => "tensorlake",
            Section::Containers => "containers",
            Section::Credentials => "credentials",
            Section::Terminal => "terminal",
            Section::Checkouts => "checkouts",
            Section::AgentTrust => "agent-trust",
            Section::Notifications => "notifications",
            Section::App => "app",
        }
    }

    /// The section's name, on its button and as its heading.
    fn label(self) -> &'static str {
        match self {
            Section::Sessions => "Sessions",
            Section::Appearance => "Appearance",
            Section::Hosts => "Hosts",
            Section::Recipes => "Recipes",
            Section::Tensorlake => "Tensorlake",
            Section::Containers => "Containers",
            Section::Credentials => "Credentials",
            Section::Terminal => "Terminal",
            Section::Checkouts => "Checkouts",
            Section::AgentTrust => "Agent trust",
            Section::Notifications => "Notifications",
            Section::App => "App",
        }
    }

    /// The section's glyph on its button, the old app's (`settings_section_icon`).
    fn icon(self) -> Icon {
        match self {
            Section::Sessions => Icon::Sessions,
            Section::Appearance => Icon::Appearance,
            Section::Hosts => Icon::Hosts,
            Section::Recipes => Icon::Folder,
            Section::Tensorlake => Icon::Key,
            // The old app drew Containers with the Hosts glyph too (`settings_section_icon`).
            Section::Containers => Icon::Hosts,
            Section::Credentials => Icon::Lock,
            Section::Terminal => Icon::Terminal,
            Section::Checkouts => Icon::Folder,
            Section::AgentTrust => Icon::AgentTrust,
            Section::Notifications => Icon::Bell,
            Section::App => Icon::App,
        }
    }

    /// The control focus starts on when Settings opens at this section: its first switch. `None` for Appearance and
    /// Terminal, whose first control is a radio group: its options' focus handles live in the component's own element
    /// state, out of the dialog's reach, so opening there focuses the dialog itself (see [`open_at`]). Nothing in the
    /// app opens Settings at either; only their goldens do. `None` for Agent trust too, whose first switch is
    /// unavailable until the section's read answers ([`open_at_agent_trust`]), and for Checkouts, whose first control
    /// is a text field that is read-only until its read answers, and Credentials, Recipes and Tensorlake, whose
    /// controls wait for their reads too ([`credentials`], [`recipes`], [`tensorlake`]); only their goldens open at any
    /// of these. `None` for Containers too, whose first control, Refresh, belongs to a view made as the section first
    /// shows ([`containers`]); only its golden opens there. `None` for Notifications too, although its first control,
    /// the waiting sound's switch, could take focus: nothing in the app opens Settings there (only its golden and its
    /// tests do), and the ntfy form's tests walk Tab from the dialog itself. `None` for App as well, whose switch is
    /// not a preference [`Choice`]: [`SettingsDialog::start`] gives it its own handle.
    fn first(self) -> Option<Choice> {
        match self {
            Section::Sessions => Some(Choice::SkipSessionConfirmations),
            Section::Appearance => None,
            Section::Hosts => Some(Choice::AutoUpdateRemoteHosts),
            Section::Recipes => None,
            Section::Tensorlake => None,
            Section::Containers => None,
            Section::Credentials => None,
            Section::Terminal => None,
            Section::Checkouts => None,
            Section::AgentTrust => None,
            Section::Notifications => None,
            Section::App => None,
        }
    }
}

/// The open dialog: which section is shown, every section's state, and its layer.
pub(crate) struct SettingsDialog {
    /// The section drawn beside the list; the others keep their state undrawn.
    shown: Section,
    /// The field lives with the dialog. Searching temporarily replaces the right pane and keeps its section state.
    search: Search,
    /// One per [`Choice`], in [`Choice::ALL`]'s order, whichever section draws it.
    switches: Vec<Switch>,
    /// Appearance's radio group: like a switch's draft, it holds the value being saved and the last refusal, and its
    /// loaded value never reaches the screen ([`appearance::section`]).
    appearance: Draft<Preference>,
    /// Terminal's link choice: like a switch's draft, it holds the value being saved and the last refusal, and its
    /// loaded value never reaches the screen ([`terminal::links`]).
    link_click: Draft<LinkClick>,
    /// Terminal's tunnel switch: its off save, the last failure in either direction and its focus handle
    /// ([`terminal::tunnel_switch`]).
    tunnels: TunnelConsent,
    /// Agent trust's switches, in [`TrustAgent::ALL`]'s order. What they know comes from the read the section sends
    /// the first time it is shown in this opening; `trust_read` says whether it was sent.
    trust: Vec<Trust>,
    trust_read: bool,
    /// Checkouts' two explicitly saved fields, made the first time the section is shown in this opening, which is when
    /// they read their routes (the pattern Agent trust follows).
    checkout_fields: Option<CheckoutFields>,
    /// Checkouts' spare switch. Its read is sent with the fields', so `checkout_fields` also says whether it was sent.
    spare: Configured,
    spare_focus: FocusHandle,
    /// Notifications' ntfy alerts form, made the first time the section is shown in this opening, which is when it
    /// reads the helm's settings (the pattern Agent trust follows).
    ntfy: Option<Entity<ntfy::NtfyAlerts>>,
    /// Terminal's file-types field, made the first time the section is shown in this opening ([`file_types`]).
    file_types: Option<Entity<file_types::FileTypes>>,
    /// The Containers section, made the first time it is shown in this opening, which is when it reads the helm's
    /// container hosts and leftovers (the pattern Agent trust follows).
    containers: Option<Entity<containers::Containers>>,
    /// App's automatic-updates switch: its focus handle and the reason its last press was not saved ([`app`]).
    app: app::AppUpdates,
    /// Redraw App's check button as the shared updater publishes progress from any of its entry points.
    _updater: Option<Subscription>,
    /// The Credentials section, made the first time it is shown in this opening; it reads the catalog then and each
    /// time it is shown again.
    credentials: Option<Entity<credentials::Credentials>>,
    /// What this opening knows of the Tensorlake account, shared by every Tensorlake API key form in it (SPEC.md: a
    /// save or clear in either form updates both); made with the first section that draws one, which reads it then.
    tensorlake: Option<Entity<TensorlakeAccount>>,
    /// The Tensorlake section, made the first time it is shown in this opening, over the shared account above, which
    /// reads it then if no section has yet ([`tensorlake`]).
    tensorlake_section: Option<Entity<tensorlake::Tensorlake>>,
    /// The Recipes section, made the first time it is shown in this opening. Its rows are the pushed catalog; whether
    /// a key is saved comes from the shared account above, read then if no section has yet ([`recipes`]).
    recipes: Option<Entity<recipes::Recipes>>,
    /// The result or trust hint waiting for a control to become available.
    focus_on: Option<Handover>,
    /// Set right after the layer opens; what the close controls dismiss.
    layer: Option<LayerId>,
    /// Sets [`Closings`] when this dialog is released.
    _closing: Subscription,
    /// The control here whose press sent nothing last because another save held the preference row's key: the
    /// switches, Appearance, Terminal's link choice and tunnel switch, the spare switch and the trust switches
    /// ([`refused`]). Checkouts' fields and the ntfy form, views of their own, keep their own.
    refused: Refused,
    /// The platform the Terminal section decides its tunnel switch for ([`terminal::shown_on`]): this build's, which
    /// tests and goldens replace ([`SettingsDialog::decide_tunnels_for`]) so the Mac's section is covered on any machine.
    /// Held here and read the same way in every build, rather than chosen by a test-only branch while drawing.
    platform: Platform,
}

/// Checkouts' text fields, each a view of its own ([`CheckoutField`]) that the dialog draws as a child.
struct CheckoutFields {
    folder: Entity<CheckoutField>,
    post_clone: Entity<CheckoutField>,
}

/// A focus request that outlives the input event because its target still needs to load. The mark limits it to the
/// initiating input, and the window lets a read that answers outside the draw pass hand focus over.
struct Handover {
    target: Target,
    mark: Mark,
    window: AnyWindowHandle,
}

impl SettingsDialog {
    /// Decide the tunnel switch, and the file-types field's modifier, for `platform` instead of this build's: the
    /// test-only setter of [`Self::platform`], so tests and goldens draw the Mac's Terminal section on any machine.
    /// Takes effect at the next draw.
    #[cfg(test)]
    pub(crate) fn decide_tunnels_for(&mut self, platform: Platform, cx: &mut Context<Self>) {
        self.platform = platform;
        self.search.set_platform(platform, cx);
        if let Some(field) = &self.file_types {
            field.update(cx, |field, cx| field.set_platform(platform, cx));
        }
        cx.notify();
    }

    /// A dialog showing `shown`, with no save of its own yet.
    fn new(shown: Section, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let switches = Choice::ALL
            .iter()
            .map(|&choice| Switch {
                choice,
                // A draft's value is shown only while it is being saved, and is set by the click that saves it, so
                // what it is loaded with never reaches the screen (`Switch::shown`).
                draft: Draft::load(false),
                focus: cx.focus_handle().tab_stop(true),
            })
            .collect();
        SettingsDialog {
            shown,
            search: Search::new(crate::actions::THIS_PLATFORM, window, cx),
            switches,
            appearance: Draft::load(Preference::System),
            link_click: Draft::load(LinkClick::Intercept),
            tunnels: TunnelConsent::new(cx),
            trust: TrustAgent::ALL
                .iter()
                .map(|&agent| Trust {
                    agent,
                    state: Configured::new(),
                    focus: cx.focus_handle().tab_stop(true),
                })
                .collect(),
            trust_read: false,
            checkout_fields: None,
            spare: Configured::new(),
            spare_focus: cx.focus_handle().tab_stop(true),
            ntfy: None,
            file_types: None,
            containers: None,
            app: app::AppUpdates::new(cx),
            _updater: crate::desktop::updates::entity(cx)
                .map(|updater| cx.observe(&updater, |_, _, cx| cx.notify())),
            credentials: None,
            tensorlake: None,
            tensorlake_section: None,
            recipes: None,
            focus_on: None,
            layer: None,
            _closing: cx.on_release(|_: &mut SettingsDialog, cx| cx.set_global(Closings)),
            refused: Refused::default(),
            platform: crate::actions::THIS_PLATFORM,
        }
    }

    /// The index of `choice`'s switch in [`SettingsDialog::switches`].
    fn index(choice: Choice) -> usize {
        Choice::ALL
            .iter()
            .position(|&each| each == choice)
            .unwrap_or_default()
    }

    /// Where focus starts when the dialog opens at `section`, or `None` to start on the dialog itself
    /// ([`Section::first`]).
    ///
    /// App's switch is not a preference [`Choice`], so it has its own handle, given only while App is listed: where no
    /// updater runs, the section draws nothing, and a handle with no element behind it could not take focus.
    fn start(&self, section: Section, cx: &App) -> Option<FocusHandle> {
        if section == Section::App {
            return section.listed(cx).then(|| self.app.focus.clone());
        }
        let first = section.first()?;
        Some(self.switches[Self::index(first)].focus.clone())
    }

    /// Show `section`. Every section's state stays as it is: only which one is drawn changes. Showing Agent trust for
    /// the first time in this opening reads the agents' trust choices, and showing Checkouts for the first time makes
    /// its fields (which read their routes) and reads the spare switch, showing Notifications for the first time makes
    /// its form, which reads the helm's ntfy settings, and showing Containers for the first time makes its section,
    /// which reads the leftovers and asks the helm to check every container host. Showing Credentials makes it the
    /// first time, which reads the Tensorlake account too if no section has yet, and reads its catalog every time;
    /// showing Tensorlake makes it the first time, over the same account, read then if no section has yet and not
    /// again, as the old account was read once per opening, except once after a refused Create SSH key
    /// ([`tensorlake_ssh_key`]); showing Recipes makes it the first time, over the same account, read then if no
    /// section has yet.
    fn show(&mut self, section: Section, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_on = None;
        self.shown = section;
        if section == Section::Containers && self.containers.is_none() {
            self.containers = Some(cx.new(containers::Containers::new));
        }
        if section == Section::Terminal && self.file_types.is_none() {
            let platform = self.platform;
            self.file_types = Some(cx.new(|cx| file_types::FileTypes::new(platform, window, cx)));
        }
        if section == Section::Notifications && self.ntfy.is_none() {
            let form = cx.new(|cx| ntfy::NtfyAlerts::new(window, cx));
            cx.observe(&form, |dialog, _, cx| dialog.finish_handover(cx))
                .detach();
            self.ntfy = Some(form);
        }
        if section == Section::Credentials {
            match &self.credentials {
                // Shown again: the catalog is not pushed, so it is read again (the section's module docs).
                Some(section) => section.update(cx, |section, cx| section.read(cx)),
                None => {
                    let account = self
                        .tensorlake
                        .get_or_insert_with(|| cx.new(TensorlakeAccount::new))
                        .clone();
                    let key =
                        cx.new(|cx| TensorlakeKey::new(account, Place::Credentials, window, cx));
                    self.credentials =
                        Some(cx.new(|cx| credentials::Credentials::new(key, window, cx)));
                }
            }
        }
        if section == Section::Tensorlake {
            match &self.tensorlake_section {
                // Shown again: the SSH key group reads the account again if a refused Create SSH key asked it to.
                Some(shown) => shown.update(cx, |shown, cx| shown.shown_again(cx)),
                None => {
                    let account = self
                        .tensorlake
                        .get_or_insert_with(|| cx.new(TensorlakeAccount::new))
                        .clone();
                    let key = cx.new(|cx| {
                        TensorlakeKey::new(account.clone(), Place::Tensorlake, window, cx)
                    });
                    let view = cx.new(|cx| tensorlake::Tensorlake::new(account, key, window, cx));
                    cx.observe(&view, |dialog, _, cx| dialog.finish_handover(cx))
                        .detach();
                    self.tensorlake_section = Some(view);
                }
            }
        }
        if section == Section::Recipes {
            if self.recipes.is_none() {
                // Whether a key is saved comes from the account Credentials and Tensorlake share ([`recipes`]).
                let account = self
                    .tensorlake
                    .get_or_insert_with(|| cx.new(TensorlakeAccount::new))
                    .clone();
                let layer = self.layer;
                self.recipes = Some(cx.new(|cx| recipes::Recipes::new(layer, account, window, cx)));
            }
        } else if let Some(recipes) = &self.recipes {
            // Undrawn from now on: it must not keep holding the running pulse ([`recipes::Recipes::hidden`]).
            recipes.update(cx, |recipes, _| recipes.hidden());
        }
        if section == Section::Checkouts && self.checkout_fields.is_none() {
            self.checkout_fields = Some(CheckoutFields {
                folder: cx.new(|cx| {
                    CheckoutField::folder(FolderScope::Default, checkouts::FOLDER_NOTE, window, cx)
                }),
                post_clone: cx.new(|cx| CheckoutField::post_clone(window, cx)),
            });
            let dialog = cx.weak_entity();
            checkouts::read_spare(cx, move |answer, cx| {
                let _ = dialog.update(cx, |dialog, cx| {
                    dialog.spare.read_answered(answer);
                    dialog.finish_handover(cx);
                    cx.notify();
                });
            });
        }
        if section == Section::AgentTrust && !self.trust_read {
            self.trust_read = true;
            for agent in TrustAgent::ALL {
                let dialog = cx.weak_entity();
                agent.read(cx, move |answer, cx| {
                    let _ = dialog.update(cx, |dialog, cx| dialog.trust_known(agent, answer, cx));
                });
            }
        }
        cx.notify();
    }

    /// What closing the dialog by Escape or a close control does: close, unless an open recipe editor asks first
    /// ([`recipes::Editor::dismissal`]), in which case Recipes is shown, if another section was, so the question
    /// shows where it is asked.
    fn dismissal(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Dismissal {
        let Some(editor) = self
            .recipes
            .as_ref()
            .and_then(|recipes| recipes.read(cx).editor())
        else {
            return Dismissal::Close;
        };
        if editor.read(cx).asks_before_leaving(cx) {
            if self.shown != Section::Recipes {
                self.show(Section::Recipes, window, cx);
            }
            if self.search.active() {
                self.search.clear(window, cx);
                cx.notify();
            }
        }
        editor.update(cx, |editor, cx| editor.dismissal(window, cx))
    }

    /// The read of `agent`'s trust answered: the switch shows it, and takes focus, scrolled into view, if
    /// [`open_at_agent_trust`] asked for it and the user has not moved on since.
    fn trust_known(
        &mut self,
        agent: TrustAgent,
        answer: Result<bool, crate::host_text::HostText>,
        cx: &mut Context<Self>,
    ) {
        self.trust[agent.index()].state.read_answered(answer);
        self.finish_handover(cx);
        cx.notify();
    }

    /// Unavailable switches are untracked. Let their ordinary read unlock them before asking the focus coordinator
    /// to wait for the frame that draws the available switch. A later key or click ends the handover.
    fn finish_handover(&mut self, cx: &mut Context<Self>) {
        let Some(handover) = &self.focus_on else {
            return;
        };
        if focus::has_moved_on(&handover.mark, cx) {
            self.focus_on = None;
        } else if let Some(handle) = self.search_target(handover.target, cx)
            && let Some(handover) = self.focus_on.take()
        {
            let _ = handover.window.update(cx, |_, window, cx| {
                focus::focus_in_layer(&handle, &handover.mark, window, cx);
            });
        }
    }

    /// Resolve a handle from the section's current state. Read-only fields are still drawn as focusable. Disabled
    /// switches and controls that this build or a section's current editor hides have no position.
    fn search_target(&self, target: Target, cx: &App) -> Option<FocusHandle> {
        match target {
            Target::Top => None,
            Target::Choice(choice) => Some(self.switches[Self::index(choice)].focus.clone()),
            Target::Trust(agent) => {
                let trust = &self.trust[agent.index()];
                trust.state.available().then(|| trust.focus.clone())
            }
            Target::AppUpdates => self.start(Section::App, cx),
            Target::AppCheck => Section::App
                .listed(cx)
                .then(|| self.app.check_focus.clone()),
            Target::Tunnels => {
                terminal::shown_on(self.platform).then(|| self.tunnels.focus.clone())
            }
            Target::FileTypes => Some(self.file_types.as_ref()?.read(cx).search_focus(cx)),
            Target::CheckoutFolder => Some(
                self.checkout_fields
                    .as_ref()?
                    .folder
                    .read(cx)
                    .search_focus(cx),
            ),
            Target::PostClone => Some(
                self.checkout_fields
                    .as_ref()?
                    .post_clone
                    .read(cx)
                    .search_focus(cx),
            ),
            Target::SpareCheckout => self.spare.available().then(|| self.spare_focus.clone()),
            Target::NtfyField(field) => {
                Some(field.input(self.ntfy.as_ref()?.read(cx)).focus_handle(cx))
            }
            Target::NtfySwitch(switch) => self.ntfy.as_ref()?.read(cx).search_focus(switch),
            Target::TensorlakeKey(Place::Tensorlake) => {
                let section = self.tensorlake_section.as_ref()?.read(cx);
                Some(section.key().read(cx).search_focus(cx))
            }
            Target::TensorlakeKey(Place::Credentials) => {
                let section = self.credentials.as_ref()?.read(cx);
                section.key_focus(cx)
            }
            Target::TensorlakeAsk => self.tensorlake_section.as_ref()?.read(cx).search_focus(cx),
        }
    }

    /// A search temporarily undraws Recipes, just like changing sections does. Keep its draft and release its clocks.
    fn search_edited(&mut self, cx: &mut Context<Self>) {
        self.search.edited(cx);
        if self.search.active()
            && self.shown == Section::Recipes
            && let Some(recipes) = &self.recipes
        {
            recipes.update(cx, |recipes, _| recipes.hidden());
        }
        cx.notify();
    }

    fn open_search_result(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((section, target)) = self.search.chosen() {
            // The next pane may focus a multiline field. The Enter that picked this result must not type into it.
            focus::note_closing(true, window, cx);
            self.open_search_target(section, target, window, cx);
        }
    }

    /// Clear the query before showing the result's section. While an unavailable switch loads or an entry has no
    /// target, reveal the top of the selected section by moving to the field alongside it.
    fn open_search_target(
        &mut self,
        section: Section,
        target: Target,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.search.clear(window, cx);
        let mark = Mark::take(cx);
        self.show(section, window, cx);
        if let Some(handle) = self.search_target(target, cx) {
            focus::focus_in_layer(&handle, &mark, window, cx);
        } else {
            self.search.focus(window, cx);
            if matches!(
                target,
                Target::Trust(_)
                    | Target::SpareCheckout
                    | Target::NtfySwitch(_)
                    | Target::TensorlakeAsk
            ) {
                self.focus_on = Some(Handover {
                    target,
                    mark,
                    window: window.window_handle(),
                });
            }
        }
    }

    /// The user turned a switch over helm configuration to `on` (an agent's trust, Checkouts' spare switch): save it
    /// with `command`, through the switch's own route, under the preference row's key like every Settings choice
    /// ([`configured`]; [`agent_trust`] says why the key is shared). `state` reaches the switch in the dialog, and
    /// `control` is its element id, where a refused press is said ([`refused`]) and which names it in the log. Once the
    /// helm has done it, that is the choice the section holds; a refusal leaves the choice read before.
    fn toggle_configured(
        &mut self,
        control: &'static str,
        state: impl Fn(&mut Self) -> &mut Configured + Copy + 'static,
        on: bool,
        command: impl FnOnce(bool) -> Result<Command, PathError>,
        cx: &mut Context<Self>,
    ) {
        if !state(self).available() {
            return;
        }
        if state(self).draft.edit(on) {
            let started = drafts::save(
                self,
                move |dialog: &mut SettingsDialog| &mut state(dialog).draft,
                &OpKey::preferences(),
                |on| command(*on),
                cx,
                move |dialog, settled, _| {
                    if matches!(settled.outcome, Outcome::Done(_)) {
                        state(dialog).saved(on);
                    }
                },
            );
            self.refused.record(control, &started);
            if let Err(not_started) = started {
                tracing::debug!(control, ?not_started, "configuration switch not saved");
            }
        } else {
            // The switch's own save is in flight: work on the same key.
            self.refused.refuse(control);
        }
        cx.notify();
    }

    /// Edit the draft `field` reaches to `value`, the value the user chose, and save it as one field of the preference
    /// row with `command`: what every switch and radio choice here does. A press whose draft refuses the edit (its own
    /// save is in flight) or that the operation store refuses (another preference save is; see the module docs) sends
    /// nothing, and `control`, its element id, says why ([`refused`]).
    fn save_choice<T: Clone + PartialEq + std::fmt::Debug + 'static>(
        &mut self,
        control: &'static str,
        field: impl Fn(&mut Self) -> &mut Draft<T> + Copy + 'static,
        value: T,
        command: impl FnOnce(&T) -> Result<Command, PathError>,
        cx: &mut Context<Self>,
    ) {
        if field(self).edit(value) {
            let started = drafts::save(
                self,
                field,
                &OpKey::preferences(),
                command,
                cx,
                |_, _, _| {},
            );
            self.refused.record(control, &started);
            if let Err(not_started) = started {
                tracing::debug!(control, value = ?field(self).value(), ?not_started, "preference not saved");
            }
        } else {
            self.refused.refuse(control);
        }
        cx.notify();
    }

    /// The user turned `choice`'s switch to `on`: save it as the preference row's field ([`SettingsDialog::save_choice`]).
    /// A press of "Update remote hosts automatically" is a newer choice than any earlier Always update, so that one's
    /// failed save stops showing in the attention list ([`crate::hosts::status::auto_update_chosen`]), whether or not
    /// this press's own save starts.
    fn toggle(&mut self, choice: Choice, on: bool, cx: &mut Context<Self>) {
        if choice == Choice::AutoUpdateRemoteHosts {
            crate::hosts::status::auto_update_chosen(cx);
        }
        let index = Self::index(choice);
        self.save_choice(
            choice.id(),
            move |dialog: &mut SettingsDialog| &mut dialog.switches[index].draft,
            on,
            |on| choice.command(*on),
            cx,
        );
    }

    /// The user chose `preference` in Appearance: save it as the preference row's `appearance` field, as a switch saves
    /// ([`SettingsDialog::save_choice`]). The palette changes only once the helm stores the choice and pushes it, like
    /// any change to the preference row.
    fn choose_appearance(&mut self, preference: Preference, cx: &mut Context<Self>) {
        self.save_choice(
            appearance::ID,
            |dialog: &mut SettingsDialog| &mut dialog.appearance,
            preference,
            |preference| appearance::command(*preference),
            cx,
        );
    }

    /// The user chose `choice` for terminal web links: save it as the preference row's `terminal_link_click` field, as a
    /// switch saves ([`SettingsDialog::save_choice`]).
    fn choose_link_click(&mut self, choice: LinkClick, cx: &mut Context<Self>) {
        self.save_choice(
            terminal::ID,
            |dialog: &mut SettingsDialog| &mut dialog.link_click,
            choice,
            |choice| terminal::command(*choice),
            cx,
        );
    }

    /// The user turned the tunnel switch on: whether to ask the warning. Not when another save holds the preference
    /// row's key (the off save among them), since Allow's save could not start: the press asks nothing and the busy
    /// line says why, as for any switch. Otherwise the press goes through, so the last failure and any busy line under
    /// the switch go, and the caller asks.
    fn may_ask_tunnel_consent(&mut self, cx: &mut Context<Self>) -> bool {
        let busy = session::current_operations(cx).is_some_and(|operations| {
            operations
                .read(cx)
                .in_flight(&OpKey::preferences())
                .is_some()
        });
        if busy {
            self.refused.refuse(terminal::TUNNEL_ID);
        } else {
            self.tunnels.failure = None;
            self.refused.clear(terminal::TUNNEL_ID);
        }
        cx.notify();
        !busy
    }

    /// The user turned the tunnel switch off: save `tunnel_consent: false` at once, as a switch saves
    /// ([`SettingsDialog::save_choice`]), with the helm's words under the switch if it refuses. No warning: withdrawing
    /// consent narrows what a remote page can do, and the helm closes the open tunnels when it stores it.
    fn turn_tunnels_off(&mut self, cx: &mut Context<Self>) {
        if !self.tunnels.off.edit(false) {
            self.refused.refuse(terminal::TUNNEL_ID);
            cx.notify();
            return;
        }
        let started = drafts::save(
            self,
            |dialog: &mut SettingsDialog| &mut dialog.tunnels.off,
            &OpKey::preferences(),
            |_| terminal::tunnels_off_command(),
            cx,
            |dialog, settled, _| {
                if let Outcome::Refused(refusal) = &settled.outcome {
                    dialog.tunnels.failure = Some(refusal.words.clone());
                }
            },
        );
        self.refused.record(terminal::TUNNEL_ID, &started);
        match started {
            Ok(()) => self.tunnels.failure = None,
            Err(not_started) => tracing::debug!(?not_started, "tunnel consent not withdrawn"),
        }
        cx.notify();
    }

    /// The tunnel switch, wired to this dialog: on asks the terminal's warning with a mark taken at the press, off
    /// saves at once (see [`terminal`]'s module docs).
    fn tunnel_switch(&self, preferences: Option<&Preferences>, cx: &Context<Self>) -> AnyElement {
        let weak = cx.weak_entity();
        terminal::tunnel_switch(
            &self.tunnels,
            preferences,
            &self.refused,
            move |on, window, cx| {
                if !on {
                    let _ = weak.update(cx, |dialog, cx| dialog.turn_tunnels_off(cx));
                    return;
                }
                let mark = Mark::take(cx);
                if !weak
                    .update(cx, |dialog, cx| dialog.may_ask_tunnel_consent(cx))
                    .unwrap_or(false)
                {
                    return;
                }
                // Asked from the press, not from inside an update of the dialog, since the layer manager may ask the
                // dialog things as the warning opens over it. Allow's success needs nothing here: the helm pushes the
                // stored consent, and the switch follows it.
                let dialog = weak.clone();
                crate::terminal::tunnels::ask_consent(&mark, window, cx, move |saved, cx| {
                    if let Err(reason) = saved {
                        let _ = dialog.update(cx, |dialog, cx| {
                            dialog.tunnels.failure = Some(reason);
                            cx.notify();
                        });
                    }
                });
            },
            cx,
        )
        .into_any_element()
    }

    /// `choice`'s switch with its help and failure line, toggling through this dialog.
    fn switch(
        &self,
        choice: Choice,
        preferences: Option<&Preferences>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let weak = cx.weak_entity();
        choices::group(
            &self.switches[Self::index(choice)],
            preferences,
            &self.refused,
            move |on, _, cx| {
                let _ = weak.update(cx, |dialog, cx| dialog.toggle(choice, on, cx));
            },
            cx,
        )
        .into_any_element()
    }

    /// Notifications' Sounds sub-heading: its heading, the paragraph the three switches share, and the switches for the
    /// status sounds (SPEC.md, "Status"), in the old app's order (`SoundSettings` in `sounds.rs` at the reference
    /// commit). They save like every other switch here; the sounds' owner reads the same fields
    /// ([`crate::sounds::StatusSounds`]).
    ///
    /// The paragraph is the old one reworded: the old app kept these switches on the device ("These choices apply only
    /// to this device"), and here they are shared helm preferences (the maintainer's decision of 2026-11-13), so it says
    /// what the other shared switches' help says.
    fn sounds(&self, preferences: Option<&Preferences>, cx: &Context<Self>) -> AnyElement {
        let palette = theme::palette(cx);
        div()
            .id(SOUNDS_ID)
            .role(Role::Region)
            .aria_label(SOUNDS_LABEL)
            .flex()
            .flex_col()
            .gap(px(SECTION_GAP))
            .min_w_0()
            .child(
                div()
                    .id("settings-sounds-heading")
                    .role(Role::Heading)
                    .aria_label(SOUNDS_LABEL)
                    .text_size(px(choices::LABEL_TEXT))
                    .line_height(px(choices::LABEL_LINE_HEIGHT))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::color(palette.fg_0))
                    .child(SOUNDS_LABEL),
            )
            .child(choices::help(SOUNDS_HELP, cx))
            .child(self.switch(Choice::SoundWaiting, preferences, cx))
            .child(self.switch(Choice::SoundApproval, preferences, cx))
            .child(self.switch(Choice::SoundTurnFinished, preferences, cx))
            .into_any_element()
    }

    /// The shown section's content below its heading.
    fn body(
        &self,
        section: Section,
        preferences: Option<&Preferences>,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        match section {
            Section::Sessions => vec![
                self.switch(Choice::SkipSessionConfirmations, preferences, cx),
                self.switch(Choice::NameSessionsOnQuickLaunch, preferences, cx),
                self.switch(Choice::ColorTerminalFrames, preferences, cx),
            ],
            Section::Appearance => {
                let weak = cx.weak_entity();
                vec![
                    appearance::section(
                        &self.appearance,
                        preferences,
                        &self.refused,
                        move |&preference, _, cx| {
                            let _ = weak.update(cx, |dialog, cx| {
                                dialog.choose_appearance(preference, cx);
                            });
                        },
                        cx,
                    )
                    .into_any_element(),
                ]
            }
            Section::Hosts => vec![
                self.switch(Choice::AutoUpdateRemoteHosts, preferences, cx),
                choices::help(HOST_CHOICES_HELP, cx).into_any_element(),
                self.switch(Choice::SkipHostSetupConfirmation, preferences, cx),
                self.switch(Choice::SkipHostRemoveConfirmation, preferences, cx),
            ],
            Section::Containers => self
                .containers
                .iter()
                .map(|section| section.clone().into_any_element())
                .collect(),
            Section::Terminal => {
                let weak = cx.weak_entity();
                let mut body: Vec<AnyElement> = terminal::links(
                    &self.link_click,
                    preferences,
                    &self.refused,
                    move |&choice, _, cx| {
                        let _ = weak.update(cx, |dialog, cx| dialog.choose_link_click(choice, cx));
                    },
                    cx,
                )
                .into_iter()
                .map(IntoElement::into_any_element)
                .collect();
                // Below the link choices, as in the old desktop app, and only where tunnels open.
                if terminal::shown_on(self.platform) {
                    body.push(self.tunnel_switch(preferences, cx));
                }
                // Last, as in the old desktop app, and in every desktop app.
                body.extend(self.file_types.clone().map(IntoElement::into_any_element));
                body
            }
            Section::Checkouts => {
                let weak = cx.weak_entity();
                let spare = checkouts::spare(
                    &self.spare,
                    &self.spare_focus,
                    &self.refused,
                    move |on, _, cx| {
                        let _ = weak.update(cx, |dialog, cx| {
                            dialog.toggle_configured(
                                checkouts::SPARE_ID,
                                |dialog| &mut dialog.spare,
                                on,
                                checkouts::spare_command,
                                cx,
                            )
                        });
                    },
                    cx,
                );
                let mut body = Vec::new();
                if let Some(fields) = &self.checkout_fields {
                    body.push(fields.folder.clone().into_any_element());
                    body.push(fields.post_clone.clone().into_any_element());
                }
                body.push(spare.into_any_element());
                body
            }
            Section::AgentTrust => self
                .trust
                .iter()
                .enumerate()
                .map(|(index, trust)| {
                    let agent = trust.agent;
                    let weak = cx.weak_entity();
                    agent_trust::block(
                        trust,
                        index == 0,
                        &self.refused,
                        move |on, _, cx| {
                            let _ = weak.update(cx, |dialog, cx| {
                                let index = agent.index();
                                dialog.toggle_configured(
                                    agent.id(),
                                    move |dialog| &mut dialog.trust[index].state,
                                    on,
                                    |on| agent.command(on),
                                    cx,
                                )
                            });
                        },
                        |choice| self.switch(choice, preferences, cx),
                        cx,
                    )
                    .into_any_element()
                })
                .collect(),
            // Sounds above ntfy alerts, with the old sections' rule between them (`.host-settings-section +
            // .host-settings-section`: a top border and 20 pixels above the next heading).
            Section::Notifications => {
                let palette = theme::palette(cx);
                std::iter::once(self.sounds(preferences, cx))
                    .chain(self.ntfy.iter().map(|form| {
                        div()
                            .border_t_1()
                            .border_color(theme::color(palette.border_dim))
                            .pt(px(20.))
                            .child(form.clone())
                            .into_any_element()
                    }))
                    .collect()
            }
            Section::App => {
                if !section.listed(cx) {
                    return vec![
                        choices::help("App updates are unavailable in this build.", cx)
                            .into_any_element(),
                    ];
                }
                let weak = cx.weak_entity();
                app::section(
                    &self.app,
                    move |on, _, cx| {
                        let _ = weak.update(cx, |dialog, cx| {
                            dialog.app.set(on, cx);
                            cx.notify();
                        });
                    },
                    cx,
                )
                .into_iter()
                .flatten()
                .map(IntoElement::into_any_element)
                .collect()
            }
            Section::Credentials => self
                .credentials
                .iter()
                .map(|section| section.clone().into_any_element())
                .collect(),
            Section::Tensorlake => self
                .tensorlake_section
                .iter()
                .map(|section| section.clone().into_any_element())
                .collect(),
            Section::Recipes => self
                .recipes
                .iter()
                .map(|section| section.clone().into_any_element())
                .collect(),
        }
    }
}

/// A click handler that dismisses the dialog through the layer manager, as Escape does: what both close controls do.
/// Called from the click, not from inside an update of the dialog, since the layer manager may ask the dialog things.
fn dismiss(
    dialog: WeakEntity<SettingsDialog>,
) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static {
    move |_, window, cx| {
        let layer = dialog.upgrade().and_then(|dialog| dialog.read(cx).layer);
        if let Some(layer) = layer {
            layers::dismiss(layer, window, cx);
        }
    }
}

impl Render for SettingsDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let store = session::current_store(cx);
        let preferences = store
            .as_ref()
            .and_then(|store| preferences(store.read(cx).collections()))
            .cloned();
        let weak = cx.weak_entity();

        let help_edge =
            layers::beside_edge("settings-navigation-help-edge", window, cx).into_any_element();
        let searching = self.search.active();
        let listed = Section::ALL
            .iter()
            .copied()
            .filter(|section| section.listed(cx));
        // `.app-settings-navigation`: the section buttons, 6 pixels apart, with a rule between them and the section.
        let navigation = div()
            .id("settings-sections")
            .relative()
            .role(Role::Navigation)
            .aria_label("Settings sections")
            .flex_none()
            .w(px(NAV_WIDTH))
            .flex()
            .flex_col()
            .gap(px(6.))
            .pr(px(12.))
            .border_r_1()
            .border_color(theme::color(palette.border_dialog))
            .child(help_edge)
            .child(self.search.field(cx))
            .children(listed.map(|section| {
                let weak = weak.clone();
                Button::new(
                    format!("settings-section-{}", section.key()),
                    section.label(),
                    HoverHelp::tooltip(format!("Show {} settings", section.label()))
                        .beside_list(false),
                    Role::Button,
                )
                .tier(Tier::Nav)
                .icon(section.icon())
                .pressed(section == self.shown)
                .on_click(move |_, window, cx| {
                    let _ = weak.update(cx, |dialog, cx| {
                        dialog.search.clear(window, cx);
                        dialog.show(section, window, cx);
                    });
                })
            }));
        // `.app-settings-panel`: the shown section under its heading.
        let shown = if searching {
            self.search.pane(window, cx)
        } else {
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(SECTION_GAP))
                .child(
                    div()
                        .id("settings-heading")
                        .debug_selector(|| "settings-heading".into())
                        .role(Role::Heading)
                        .aria_label(self.shown.label())
                        .text_size(px(HEADING_TEXT))
                        .line_height(px(HEADING_LINE_HEIGHT))
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::color(palette.fg_0))
                        .child(self.shown.label()),
                )
                .children(self.body(self.shown, preferences.as_ref(), cx))
                .into_any_element()
        };
        let body = div()
            .flex()
            .items_start()
            .gap(px(20.))
            .text_color(theme::color(palette.fg_0))
            .when(searching, |body| body.h_full())
            .child(navigation)
            .child(shown);

        let close = Button::new(
            "settings-footer-close",
            "Close",
            HoverHelp::tooltip(CLOSE_HELP),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .on_click(dismiss(weak.clone()));
        // The recipe editor's answers, pinned with the footer only while Recipes shows the editor (SPEC.md: "Switching
        // Settings sections hides those actions with the editor"), ruled off from Close, which they do not belong to.
        let recipe_actions = self
            .recipes
            .as_ref()
            .filter(|_| self.shown == Section::Recipes && !searching)
            .and_then(|recipes| recipes.read(cx).editor())
            .map(|editor| recipes::Editor::actions(&editor, cx));
        let frame = DialogFrame::new("settings", "Settings", body, vec![close]);
        let frame = match recipe_actions {
            Some(actions) => frame.notices(actions).notices_rule(),
            None => frame,
        };
        frame
            .summary(if Section::App.listed(cx) {
                SUMMARY_WITH_APP
            } else {
                SUMMARY
            })
            .tile(
                svg()
                    .path(Icon::Settings.path())
                    .size(px(28.))
                    .text_color(theme::color(palette.fg_0)),
            )
            .note(FOOTER_NOTE)
            .width(px(WIDTH))
            .fixed_height(px(HEIGHT))
            .close(
                "Close settings",
                HoverHelp::tooltip(CLOSE_HELP),
                false,
                dismiss(weak),
            )
    }
}
