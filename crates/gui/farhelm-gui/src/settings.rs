//! The Settings dialog: the helm's shared choices, grouped in sections, opened from the gear at the foot of the host
//! rail.
//!
//! SPEC.md, "Session list" (the paragraph that starts "The gear at the foot of the host rail opens a settings dialog").
//! Ported from the old app's `settings.rs` (`SettingsDialog`) with its header from `hosts/settings_dialog.rs`
//! (`HostDialogHeader`) and its layout from the `.app-settings-*` and `.host-dialog-framed` rules in `app.css`, all at
//! reference commit `24bc0580`. Here's the TLDR of its rules:
//!
//! - **The frame.** The host dialogs' framed header (a gear tile, the title "Settings", the summary, and its own header
//!   close control, "Close settings") over independently scrolling navigation and page panes, with a pinned page
//!   heading and footer. The footer holds Close and any actions for the current page or editor. It is
//!   760 pixels wide and 780 tall, capped by the window, whatever section is shown, so switching sections never moves
//!   the frame or its close controls (`.app-settings-dialog[aria-label="Settings"]`).
//! - **The sections.** A list of section buttons on the left, each with its icon and full label and the solid selection
//!   fill on the one shown, and the shown section beside it under a heading of the same name. Only the sections this
//!   build has are listed, in SPEC.md's order ([`Section::all`]), and App only while the desktop app's updater runs
//!   ([`Section::listed`]). Each section's state lives in the dialog for as long as it is open, so a hidden section
//!   keeps its pending saves, failure lines and unsaved text (Checkouts' fields); only the shown one is drawn, so Tab
//!   never reaches a hidden control.
//! - **Opening and closing.** [`open`] is the gear's entry point. It opens at Sessions with focus on its first switch
//!   (SPEC.md: "The gear always opens sessions and focuses its first switch"). [`open_at_agent_trust`] is a trust
//!   hint's. It opens the agent's own page and puts focus on its trust switch once the switch can take it. Escape, the
//!   header's close control and the footer's Close check the same drafts. The close question names changes even on
//!   hidden pages. Saving from it waits for acknowledged writes before closing. Closing returns focus to the gear
//!   and sets [`Closings`], so the trust hint band can read the current trust setting.
//! - **Saving a choice.** Every switch here, Appearance's choice and Terminal's link choice save one field of the
//!   helm's preference row at once, with [`crate::drafts::save`] under [`OpKey::preferences`]. The workspace trust
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
//!   shows an open editor, the dialog pins the editor's actions in the same row as Close, below the editor's status.
//!   Terminal's tunnel switch saves like a switch only when turned off; turned on, it asks the terminal's tunnel
//!   warning, whose Allow saves the consent, and shows the stored choice until the helm pushes it ([`terminal`]). App's
//!   switch is not the helm's at all: it writes the desktop app's own setting through its updater, at once and with a
//!   final answer, so it has no draft and never holds the preference row's key ([`app`]). Containers saves no choice at
//!   all: it shows what the helm reads from its container hosts, and its Refresh and confirmed Deletes are plain
//!   commands, not operations on a key ([`containers`]).
//!
//! **Shared writes.** The operation store admits one request on the preference key at a time. Pressing another
//! control while that key is held leaves the control as it was and gives the reason beside it ([`refused`]).
//! Radios still move keyboard focus when the key refuses a choice. The footer's explicit multi-form Save sequences
//! the form saves, each through its usual key, and stops at the first refusal.
//!
//! NOTE: Not ported yet, each recorded in `docs/gpui/parity/settings-and-hosts.md`. The one-pane layout at 620 pixels
//! wide and below ("‹ Settings" back to the list) is not built, since the window's minimum width is above it. A
//! section switch does not move focus to the shown section's heading, as the old app did so a keyboard user went on
//! from there: the focus coordinator moves focus to a control only while no layer is open, and the heading is not a
//! control. Checkouts' docs link has its own line and can require scrolling even at full dialog height.

mod agent_trust;
mod app;
mod appearance;
mod card;
mod checkout_field;
mod checkouts;
mod choices;
pub(crate) use choices::preference_command;
mod configured;
mod containers;
#[cfg(test)]
mod containers_tests;
#[cfg(test)]
mod credential_tests;
mod credentials;
mod custom_agents;
#[cfg(test)]
pub(crate) use custom_agents::{stage_custom_agent_draft, stage_custom_agent_save_refusal};
mod editing;
mod file_types;
#[cfg(test)]
mod file_types_tests;
mod frame;
mod hook_settings;
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
#[cfg(test)]
pub(crate) use tensorlake_sandboxes::cleanup_key_refusal_for_golden;
mod tensorlake_ssh_key;
#[cfg(test)]
mod tensorlake_tests;
mod terminal;
#[cfg(test)]
mod tests;

use std::rc::Rc;

use farhelm_helm_client::PathError;
use farhelm_input_mapping::Platform;
use farhelm_proto::LaunchHarness;
use farhelm_proto::ui_channel::{PREFERENCES_KEY, Preferences};
use gpui::{
    AnyElement, AnyWindowHandle, App, AppContext as _, Context, Entity, FocusHandle,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Role, ScrollHandle,
    StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity, Window, div, px, svg,
};

use crate::assets::Icon;
use crate::components::{Button, DialogFrame, HoverHelp, Tier};
use crate::drafts::{self, Draft};
use crate::focus::{self, Mark, ReturnTo};
use crate::layers::{self, Dialog, Dismissal, LayerId};
use crate::operations::{Command, OpKey, Outcome};
use crate::session;
use crate::theme::{self, Preference};

use crate::hook_setup::HookAgent;
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
/// The search shortcut is scoped to the Settings frame, including its header and footer.
pub(crate) const SEARCH_CONTEXT: &str = "Settings";
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

/// The footer explains why forms and ordinary switches behave differently.
const FOOTER_NOTE: &str = "Switches save at once; forms save with Save below.";

/// Both close buttons and Escape go through the same draft check.
const CLOSE_HELP: &str = "Close Settings. Unsaved changes ask first.";

const SOUNDS_ID: &str = "settings-sounds";
const SOUNDS_LABEL: &str = "Sounds";
const SOUNDS_HELP: &str = "The session open in an active window stays quiet.";

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

/// [`open`], at `section` with focus on its first control (on the dialog itself for pages whose first control needs
/// its own focus handling, [`Section::first`]). The screenshot goldens call it for each section. [`open_at_agent_trust`]
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
    let store = session::current_store(cx)?;
    if focus::has_moved_on(mark, cx) {
        return None;
    }
    let dialog = cx.new(|cx| {
        cx.observe(&store, |_: &mut SettingsDialog, _, cx| cx.notify())
            .detach();
        SettingsDialog::new(section.clone(), window, cx)
    });
    let container = cx.focus_handle();
    let start = dialog
        .read(cx)
        .start(&section, cx)
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
            // Escape and either close button check every loaded form, including drafts on hidden pages.
            on_dismiss: Some(Rc::new({
                let dialog = dialog.downgrade();
                move |window: &mut Window, cx: &mut App| {
                    dialog
                        .update(cx, |dialog, cx| dialog.ask_to_close(window, cx))
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

/// The trust hint band's Settings button opens the agent's own page at its trust switch. The hint's button and its
/// click mark are the opener and mark, as in [`open`].
///
/// The switch is unavailable until the section's read of the agent's trust answers ([`agent_trust`]), and an
/// unavailable control cannot hold focus. The dialog holds focus until the read succeeds and the coordinator sees
/// an available switch. A key or click after `mark`, including a click that leaves focus in place, ends the handover.
/// When the read fails, the dialog keeps focus and shows the failure beside the switch.
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
    let (layer, dialog) = open_dialog(Section::Agent(agent.harness()), opener, mark, window, cx)?;
    // The switch becomes available in the first frame after the trust read answers.
    let handover = Handover {
        target: Target::Trust(agent),
        mark: *mark,
        window: window.window_handle(),
    };
    dialog.update(cx, |dialog, _| dialog.focus_on = Some(handover));
    Some(layer)
}

/// Open the agent's settings and focus the setup switch once the helm confirms its current value.
pub(crate) fn open_at_hook_setup(
    agent: HookAgent,
    opener: &FocusHandle,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let (layer, dialog) = open_dialog(Section::Agent(agent.harness()), opener, mark, window, cx)?;
    let handover = Handover {
        target: Target::HookSetup(agent),
        mark: *mark,
        window: window.window_handle(),
    };
    dialog.update(cx, |dialog, _| dialog.focus_on = Some(handover));
    Some(layer)
}

/// Read explicit consent from its configuration route. Failed reads never stand for an off switch.
pub(crate) fn read_hook_setup(
    agent: HookAgent,
    cx: &mut App,
    on_read: impl FnOnce(Result<bool, crate::host_text::HostText>, &mut App) + 'static,
) {
    configured::read_enabled(agent.route(), cx, on_read);
}

/// Persist the same all-hosts switch from Settings and the launch prompt.
pub(crate) fn hook_setup_command(agent: HookAgent, enabled: bool) -> Result<Command, PathError> {
    configured::enabled_command(agent.route(), enabled)
}

/// Store the dismissal both surfaces observe, using the Settings preference field and operation key.
pub(crate) fn dismiss_hook_setup_command(agent: HookAgent) -> Result<Command, PathError> {
    Choice::ShowHookSetupHint(agent).command(false)
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

/// The field each draft golden edits. A pending read leaves the input unavailable.
#[cfg(test)]
pub(crate) fn draft_input_for_opened(
    section: Section,
    cx: &App,
) -> Option<crate::components::TextInputState> {
    let dialog = layers::last_opened::<SettingsDialog>(cx)?;
    let dialog = dialog.read(cx);
    match section {
        Section::Terminal => Some(dialog.file_types.as_ref()?.read(cx).input_for_golden()),
        Section::Checkouts => dialog
            .checkout_fields
            .as_ref()?
            .folder
            .read(cx)
            .input_for_golden(),
        Section::Notifications => {
            let alerts = dialog.ntfy.as_ref()?.read(cx);
            alerts.search_focus(ntfy::Switch::Enabled)?;
            Some(alerts.topic_input())
        }
        Section::Recipes => dialog
            .recipes
            .as_ref()?
            .read(cx)
            .editor()?
            .read(cx)
            .field("recipe-name")
            .cloned(),
        Section::Credentials => Some(
            dialog
                .credentials
                .as_ref()?
                .read(cx)
                .visible_editor()?
                .read(cx)
                .secret_for_golden(),
        ),
        _ => None,
    }
}

/// The edited golden verifies that the real form accepted the staged input before the scene can settle.
#[cfg(test)]
pub(crate) fn draft_changed_for_opened(section: Section, cx: &App) -> bool {
    layers::last_opened::<SettingsDialog>(cx)
        .is_some_and(|dialog| dialog.read(cx).section_edited(&section, cx))
}

/// Move an open golden to a second form, using the same page transition as its navigation.
#[cfg(test)]
pub(crate) fn show_for_opened(section: Section, window: &mut Window, cx: &mut App) {
    let dialog =
        layers::last_opened::<SettingsDialog>(cx).expect("Settings is open for its golden");
    dialog.update(cx, |dialog, cx| dialog.show(section, window, cx));
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

/// The saved-recipe scenes stage either the first Delete or the follow-up
/// that needs explicit consent to forget an absent image capture.
#[cfg(test)]
#[derive(Clone, Copy)]
pub(crate) enum SavedRecipeQuestion {
    Delete,
    DeleteAnyway,
}

/// Stage the real saved-recipe editor and optionally open its inline delete question.
#[cfg(test)]
pub(crate) fn open_saved_recipe_for_opened(
    question: Option<SavedRecipeQuestion>,
    window: &mut Window,
    cx: &mut App,
) {
    let saved = recipe_tests::entry("rust-dev", recipe_tests::idle(), serde_json::Value::Null);
    let saved = serde_json::from_value(saved).expect("the saved recipe fixture decodes");
    let recipes = layers::last_opened::<SettingsDialog>(cx)
        .and_then(|dialog| dialog.read(cx).recipes.clone())
        .expect("Settings is open at Recipes for its golden");
    let editor = recipes.update(cx, |recipes, cx| {
        recipes.open_editor(Some(&saved), window, cx);
        recipes.editor().expect("the saved recipe editor is open")
    });
    if let Some(question) = question {
        editor.update(cx, |editor, cx| {
            editor.ask_delete_for_golden(question, window, cx)
        });
    }
}

/// Open the in-place agent login editor on a staged Credentials page.
#[cfg(test)]
pub(crate) fn open_agent_credential_for_opened(window: &mut Window, cx: &mut App) {
    let credentials = layers::last_opened::<SettingsDialog>(cx)
        .and_then(|dialog| dialog.read(cx).credentials.clone())
        .expect("Settings is open at Credentials for its golden");
    credentials.update(cx, |credentials, cx| {
        credentials.open_agent_editor_for_golden(window, cx)
    });
}

/// A staged inline question has to wait for the fake helm's leftovers response.
#[cfg(test)]
pub(crate) fn ask_first_container_for_opened(window: &mut Window, cx: &mut App) -> bool {
    let Some(containers) = layers::last_opened::<SettingsDialog>(cx)
        .and_then(|dialog| dialog.read(cx).containers.clone())
    else {
        return false;
    };
    let Some(row) = containers.read(cx).first_leftover_for_golden() else {
        return false;
    };
    containers.update(cx, |containers, cx| {
        containers.ask_for_golden(row, window, cx)
    });
    true
}

/// Wait for the recipe editor's initial focus before moving the viewport in its end scene.
#[cfg(test)]
pub(crate) fn page_scroll_for_opened(
    wait_for_recipe_back: bool,
    window: &Window,
    cx: &App,
) -> Option<ScrollHandle> {
    let dialog = layers::last_opened::<SettingsDialog>(cx)?;
    let dialog = dialog.read(cx);
    if wait_for_recipe_back {
        let editor = dialog.recipes.as_ref()?.read(cx).editor()?;
        if !editor.read(cx).start(cx).is_focused(window) {
            return None;
        }
    }
    Some(dialog.page_scroll.clone())
}

/// The helm's preference row, if it pushed one.
fn preferences(collections: &farhelm_helm_client::Collections) -> Option<&Preferences> {
    collections.get::<Preferences>(PREFERENCES_KEY)
}

const NO_AGENT_SETTINGS: &str = "There are no settings to configure for this agent.";
const NO_AGENT_SETTINGS_ID: &str = "settings-agent-empty";

/// A section of the dialog.
///
/// Ordinary pages have an explicit order. Agent pages are derived from the launch harnesses so an agent appears even
/// when Farhelm has no settings for it.
#[derive(Clone, Debug, PartialEq, Eq)]
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
    Agent(LaunchHarness),
    /// The ID stays stable when the helm pushes a rename.
    CustomAgent(String),
    AddCustomAgent,
    Notifications,
    /// The desktop app's own automatic-updates switch, listed only while its updater runs ([`Section::listed`]).
    App,
}

impl Section {
    /// Settings navigation and search share this order so results follow the same sequence as the section list.
    fn all() -> impl Iterator<Item = Section> {
        [
            Section::Sessions,
            Section::Appearance,
            Section::Terminal,
            Section::Notifications,
            Section::Credentials,
            Section::App,
            Section::Hosts,
            Section::Checkouts,
            Section::Containers,
            Section::Recipes,
            Section::Tensorlake,
        ]
        .into_iter()
        .chain(
            LaunchHarness::BUILT_INS
                .iter()
                .copied()
                .filter(|harness| !harness.is_shell())
                .map(Section::Agent),
        )
        .chain([Section::AddCustomAgent])
    }

    /// Whether the dialog lists this section in this app: every section in [`Section::all`], except App, which only a
    /// desktop app whose updater runs has (SPEC.md: "followed by **App** only in a desktop app whose updater is
    /// active"). The updater's own answer decides, so no platform or build is checked here.
    fn listed(&self, cx: &App) -> bool {
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
            | Section::Agent(_)
            | Section::CustomAgent(_)
            | Section::AddCustomAgent
            | Section::Notifications => true,
        }
    }

    /// Search uses the same stable suffix as the navigation button.
    fn key(&self) -> &'static str {
        self.button_id().trim_start_matches("settings-section-")
    }

    /// Element IDs stay the same as settings move within a page or groups change position.
    fn button_id(&self) -> &'static str {
        match self {
            Section::Sessions => "settings-section-sessions",
            Section::Appearance => "settings-section-appearance",
            Section::Hosts => "settings-section-hosts",
            Section::Recipes => "settings-section-recipes",
            Section::Tensorlake => "settings-section-tensorlake",
            Section::Containers => "settings-section-containers",
            Section::Credentials => "settings-section-credentials",
            Section::Terminal => "settings-section-terminal",
            Section::Checkouts => "settings-section-checkouts",
            Section::Agent(harness) => crate::harness::settings_button_id(*harness),
            Section::CustomAgent(_) => "settings-section-custom-agent",
            Section::AddCustomAgent => "settings-section-add-custom-agent",
            Section::Notifications => "settings-section-notifications",
            Section::App => "settings-section-app",
        }
    }

    /// The section's name, on its button and as its heading.
    fn label(&self) -> &'static str {
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
            Section::Agent(harness) => crate::harness::name(*harness),
            Section::CustomAgent(_) => "Custom agent",
            Section::AddCustomAgent => "+ Add custom agent",
            Section::Notifications => "Notifications",
            Section::App => "App",
        }
    }

    /// The section's glyph on its button, the old app's (`settings_section_icon`).
    fn icon(&self) -> Icon {
        match self {
            Section::Sessions => Icon::Sessions,
            Section::Appearance => Icon::Appearance,
            Section::Hosts => Icon::Hosts,
            Section::Recipes => Icon::Folder,
            Section::Tensorlake => Icon::Key,
            Section::Containers => Icon::EnvironmentContainer,
            Section::Credentials => Icon::Lock,
            Section::Terminal => Icon::Terminal,
            Section::Checkouts => Icon::Branch,
            Section::Agent(harness) => crate::harness::icon(crate::harness::glyph(*harness)),
            Section::CustomAgent(_) | Section::AddCustomAgent => Icon::HarnessCustom,
            Section::Notifications => Icon::Bell,
            Section::App => Icon::App,
        }
    }

    /// The control focus starts on when Settings opens at this section: its first switch. `None` for Appearance and
    /// Terminal, whose first control is a radio group: its options' focus handles live in the component's own element
    /// state, out of the dialog's reach, so opening there focuses the dialog itself (see [`open_at`]). Nothing in the
    /// app opens Settings at either; only their goldens do. `None` for agent pages too, whose trust switch is
    /// unavailable until the section's read answers ([`open_at_agent_trust`]), and for Checkouts, whose first control
    /// is a text field that is read-only until its read answers, and Credentials, Recipes and Tensorlake, whose
    /// controls wait for their reads too ([`credentials`], [`recipes`], [`tensorlake`]); only their goldens open at any
    /// of these. `None` for Containers too, whose first control, Refresh, belongs to a view made as the section first
    /// shows ([`containers`]); only its golden opens there. `None` for Notifications too, although its first control,
    /// the waiting sound's switch, could take focus: nothing in the app opens Settings there (only its golden and its
    /// tests do), and the ntfy form's tests walk Tab from the dialog itself. `None` for App as well, whose switch is
    /// not a preference [`Choice`]: [`SettingsDialog::start`] gives it its own handle.
    fn first(&self) -> Option<Choice> {
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
            Section::Agent(_) => None,
            Section::CustomAgent(_) | Section::AddCustomAgent => None,
            Section::Notifications => None,
            Section::App => None,
        }
    }
}

/// The open dialog: which section is shown, every section's state, and its layer.
pub(crate) struct SettingsDialog {
    /// The section drawn beside the list; the others keep their state undrawn.
    shown: Section,
    /// Search temporarily replaces the page. Keep its scroll offset in the dialog so clearing the query restores it.
    page_scroll: ScrollHandle,
    /// The explicit saves still owned by the last footer or close action.
    batch: Option<editing::Batch>,
    save_generation: u64,
    /// A close Save cannot pick one of two different replacement keys for the same account.
    key_conflict: bool,
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
    /// Trust switches, in [`TrustAgent::ALL`]'s order. Each page reads its own switch the first time it is shown.
    trust: Vec<Trust>,
    /// Configuration switches are read on demand by the two agent pages.
    hooks: [hook_settings::HookSwitch; 2],
    /// Checkouts' two explicitly saved fields, made the first time the section is shown in this opening, which is when
    /// they read their routes.
    checkout_fields: Option<CheckoutFields>,
    /// Checkouts' spare switch. Its read is sent with the fields', so `checkout_fields` also says whether it was sent.
    spare: Configured,
    spare_focus: FocusHandle,
    /// Notifications' ntfy alerts form, made the first time the section is shown in this opening, which is when it
    /// reads the helm's settings once for this opening.
    ntfy: Option<Entity<ntfy::NtfyAlerts>>,
    /// Terminal's file-types field, made the first time the section is shown in this opening ([`file_types`]).
    file_types: Option<Entity<file_types::FileTypes>>,
    /// The Containers section, made the first time it is shown in this opening, which is when it reads the helm's
    /// container hosts and leftovers.
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
    /// Drafts belong to this opening, including those a user leaves on another custom-agent page.
    custom_agents: std::collections::BTreeMap<Option<String>, Entity<custom_agents::Editor>>,
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
            page_scroll: ScrollHandle::new(),
            batch: None,
            save_generation: 0,
            key_conflict: false,
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
                    read_started: false,
                })
                .collect(),
            hooks: HookAgent::ALL.map(|_| hook_settings::HookSwitch {
                state: Configured::new(),
                focus: cx.focus_handle().tab_stop(true),
                read_started: false,
            }),
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
            custom_agents: std::collections::BTreeMap::new(),
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
    fn start(&self, section: &Section, cx: &App) -> Option<FocusHandle> {
        if *section == Section::App {
            return section.listed(cx).then(|| self.app.focus.clone());
        }
        let first = section.first()?;
        Some(self.switches[Self::index(first)].focus.clone())
    }

    /// Show `section`, keeping each page's state while it is hidden. Each agent's section reads its trust choice once
    /// per opening. Showing Checkouts for the first time makes
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
        if self.shown != section {
            self.page_scroll.set_offset(gpui::Point::default());
        }
        self.shown = section.clone();
        if let Some(key) = section.custom_agent_key() {
            self.open_custom_editor(key, window, cx);
        }
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
                        .with_pinned_save()
                }),
                post_clone: cx.new(|cx| CheckoutField::post_clone(window, cx).with_pinned_save()),
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
        if let Section::Agent(harness) = section
            && let Some(agent) = TrustAgent::for_harness(harness)
            && !self.trust[agent.index()].read_started
        {
            self.trust[agent.index()].read_started = true;
            let dialog = cx.weak_entity();
            agent.read(cx, move |answer, cx| {
                let _ = dialog.update(cx, |dialog, cx| dialog.trust_known(agent, answer, cx));
            });
        }
        if let Section::Agent(harness) = section
            && let Some(agent) = HookAgent::for_harness(harness)
            && !self.hooks[agent.index()].read_started
        {
            self.hooks[agent.index()].read_started = true;
            let dialog = cx.weak_entity();
            read_hook_setup(agent, cx, move |answer, cx| {
                let _ = dialog.update(cx, |dialog, cx| {
                    dialog.hooks[agent.index()].state.read_answered(answer);
                    dialog.finish_handover(cx);
                    cx.notify();
                });
            });
        }
        cx.notify();
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
            Target::HookSetup(agent) => {
                let switch = &self.hooks[agent.index()];
                switch.state.available().then(|| switch.focus.clone())
            }
            Target::AppUpdates => self.start(&Section::App, cx),
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
            self.page_scroll.set_offset(gpui::Point::default());
            self.search.focus(window, cx);
            if matches!(
                target,
                Target::Trust(_)
                    | Target::HookSetup(_)
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
        let running = if choice == Choice::ShowWhatsNew {
            crate::desktop::updates::upgrade_notes(cx).map(|host| host.running.clone())
        } else {
            None
        };
        let index = Self::index(choice);
        self.save_choice(
            choice.id(),
            move |dialog: &mut SettingsDialog| &mut dialog.switches[index].draft,
            on,
            |on| match running {
                Some(running) => choices::whats_new_command(&running, *on),
                None => choice.command(*on),
            },
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

    /// The four status sound preferences share one card. Each row carries its own description and preview.
    fn sounds(&self, preferences: Option<&Preferences>, cx: &Context<Self>) -> AnyElement {
        div()
            .id(SOUNDS_ID)
            .role(Role::Region)
            .aria_label(SOUNDS_LABEL)
            .flex()
            .flex_col()
            .gap(px(6.))
            .min_w_0()
            .child(
                div()
                    .id("settings-sounds-heading")
                    .role(Role::Heading)
                    .aria_label(SOUNDS_LABEL)
                    .child(card::heading(SOUNDS_LABEL, Some(SOUNDS_HELP), None, cx)),
            )
            .child(card::rows(
                [
                    self.switch(Choice::SoundWaiting, preferences, cx),
                    self.switch(Choice::SoundApproval, preferences, cx),
                    self.switch(Choice::SoundTurnFinished, preferences, cx),
                    self.switch(Choice::SoundSshSignIn, preferences, cx),
                ],
                cx,
            ))
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
                card::rows(
                    [
                        self.switch(Choice::SkipSessionConfirmations, preferences, cx),
                        self.switch(Choice::NameSessionsOnQuickLaunch, preferences, cx),
                        self.switch(Choice::ColorTerminalFrames, preferences, cx),
                    ],
                    cx,
                )
                .into_any_element(),
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
                card::rows(
                    [
                        self.switch(Choice::AutoUpdateRemoteHosts, preferences, cx),
                        self.switch(Choice::SkipHostSetupConfirmation, preferences, cx),
                        self.switch(Choice::SkipHostRemoveConfirmation, preferences, cx),
                    ],
                    cx,
                )
                .into_any_element(),
            ],
            Section::Containers => self
                .containers
                .iter()
                .map(|section| section.clone().into_any_element())
                .collect(),
            Section::Terminal => {
                let weak = cx.weak_entity();
                let mut body = vec![
                    terminal::links(
                        &self.link_click,
                        preferences,
                        &self.refused,
                        move |&choice, _, cx| {
                            let _ =
                                weak.update(cx, |dialog, cx| dialog.choose_link_click(choice, cx));
                        },
                        cx,
                    )
                    .into_any_element(),
                ];
                // Below the link choices, as in the old desktop app, and only where tunnels open.
                if terminal::shown_on(self.platform) {
                    body.push(self.tunnel_switch(preferences, cx));
                }
                // Last, as in the old desktop app, and in every desktop app.
                body.extend(self.file_types.iter().map(|form| {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(card::heading("Downloaded files", None, None, cx))
                        .child(card::rows([form.clone()], cx))
                        .into_any_element()
                }));
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
                self.checkout_fields
                    .iter()
                    .map(|fields| {
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .child(card::heading("New checkouts", None, None, cx))
                            .child(card::rows(
                                [fields.folder.clone(), fields.post_clone.clone()],
                                cx,
                            ))
                            .into_any_element()
                    })
                    .chain(std::iter::once(card::rows([spare], cx).into_any_element()))
                    .collect()
            }
            Section::Agent(harness) => {
                let mut panels = Vec::new();
                if let Some(agent) = TrustAgent::for_harness(harness) {
                    let trust = &self.trust[agent.index()];
                    let weak = cx.weak_entity();
                    panels.push(
                        agent_trust::block(
                            trust,
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
                        .into_any_element(),
                    );
                }
                if let Some(agent) = HookAgent::for_harness(harness) {
                    let index = agent.index();
                    let weak = cx.weak_entity();
                    let opt_in = hook_settings::row(
                        agent,
                        &self.hooks[index],
                        &self.refused,
                        move |on, _, cx| {
                            let _ = weak.update(cx, |dialog, cx| {
                                dialog.toggle_configured(
                                    agent.id(),
                                    move |dialog| &mut dialog.hooks[index].state,
                                    on,
                                    |on| hook_setup_command(agent, on),
                                    cx,
                                );
                            });
                        },
                        cx,
                    );
                    panels.push(
                        card::rows(
                            [
                                opt_in.into_any_element(),
                                self.switch(Choice::ShowHookSetupHint(agent), preferences, cx),
                            ],
                            cx,
                        )
                        .into_any_element(),
                    );
                }
                if panels.is_empty() {
                    panels.push(
                        card::help(NO_AGENT_SETTINGS_ID, NO_AGENT_SETTINGS, cx).into_any_element(),
                    );
                }
                panels
            }
            Section::CustomAgent(_) | Section::AddCustomAgent => self
                .custom_editor()
                .map(|editor| editor.clone().into_any_element())
                .into_iter()
                .collect(),
            Section::Notifications => [
                self.sounds(preferences, cx),
                card::rows(
                    [self.switch(Choice::CheckAnnouncements, preferences, cx)],
                    cx,
                )
                .into_any_element(),
            ]
            .into_iter()
            .chain(self.ntfy.iter().cloned().map(IntoElement::into_any_element))
            .collect(),
            Section::App => {
                let weak = cx.weak_entity();
                let rows = app::section(
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
                .chain(
                    crate::desktop::updates::upgrade_notes(cx)
                        .map(|_| self.switch(Choice::ShowWhatsNew, preferences, cx)),
                );
                vec![card::rows(rows, cx).into_any_element()]
            }
            Section::Credentials => {
                let conflict = self.key_conflict.then(|| {
                    let message =
                        "Two different Tensorlake keys are typed. Save the one you want first.";
                    choices::outcome_line(
                        "settings-key-conflict",
                        Role::Alert,
                        theme::color(theme::palette(cx).danger),
                    )
                    .aria_label(message)
                    .child(message)
                    .into_any_element()
                });
                conflict
                    .into_iter()
                    .chain(
                        self.credentials
                            .iter()
                            .cloned()
                            .map(IntoElement::into_any_element),
                    )
                    .collect()
            }
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
        self.sync_custom_agents(store.as_ref(), window, cx);
        let preferences = store
            .as_ref()
            .and_then(|store| preferences(store.read(cx).collections()))
            .cloned();
        let weak = cx.weak_entity();

        let searching = self.search.active();
        let page = if searching {
            div()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .bg(theme::color(palette.settings_page))
                .child(self.search.pane(window, cx))
                .into_any_element()
        } else {
            self.page(preferences.as_ref(), window, cx)
        };
        let body = div()
            .flex()
            .flex_1()
            .min_h_0()
            .text_color(theme::color(palette.fg_0))
            .child(self.navigation(window, cx))
            .child(page);

        let close = Button::new(
            "settings-footer-close",
            "Close",
            HoverHelp::tooltip(CLOSE_HELP),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .on_click(dismiss(weak.clone()));
        // An open editor owns the row even if another form on this page is dirty. Delete confirmation replaces
        // the recipe editor's controls until the question is answered.
        let editor_footer = match (searching, &self.shown) {
            (false, Section::Recipes) => self
                .recipes
                .as_ref()
                .and_then(|recipes| recipes.read(cx).editor())
                .map(|editor| recipes::Editor::footer(&editor, true, cx)),
            (false, Section::Credentials) => self
                .credentials
                .as_ref()
                .and_then(|credentials| credentials.read(cx).visible_editor())
                .map(|editor| credentials::Editor::footer(&editor, cx)),
            (false, Section::CustomAgent(_) | Section::AddCustomAgent) => self
                .custom_editor()
                .map(|editor| custom_agents::Editor::footer(editor, cx)),
            _ => None,
        };
        let pending = (!searching && editor_footer.is_none())
            .then(|| self.page_answers(cx))
            .flatten();
        let confirming = editor_footer
            .as_ref()
            .is_some_and(|(_, _, confirming)| *confirming);
        let divided = !confirming && (editor_footer.is_some() || pending.is_some());
        let (note, mut answers) =
            pending.map_or((None, Vec::new()), |(note, answers)| (Some(note), answers));
        let note = note.or_else(|| {
            if editor_footer.is_some() && !confirming {
                self.editor_note(cx)
            } else {
                None
            }
        });
        if !confirming {
            answers.push(close);
        }
        let frame = DialogFrame::new("settings", "Settings", body, answers);
        let frame = if let Some(name) = note {
            frame.footer_lead(
                div()
                    .id("settings-unsaved")
                    .role(Role::Status)
                    .aria_label(format!("{name} has unsaved changes"))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_size(px(11.))
                    .text_color(theme::color(palette.fg_1))
                    .child(editing::dot(false, palette))
                    .child(
                        div()
                            .flex()
                            .gap(px(3.))
                            .child(
                                div()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme::color(palette.fg_0))
                                    .child(name),
                            )
                            .child("has unsaved changes"),
                    ),
            )
        } else if !confirming && self.shown.custom_agent_key().is_none() {
            // A custom registration stores its capability checkboxes together with the other draft fields.
            frame.note(FOOTER_NOTE)
        } else {
            frame
        };
        let frame = match editor_footer {
            Some((actions, Some(notices), _)) => frame.footer_actions(actions).notices(notices),
            Some((actions, None, _)) => frame.footer_actions(actions),
            None => frame,
        };
        let frame = if divided {
            frame.separate_close()
        } else {
            frame
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
            .width(px(WIDTH))
            .fixed_height(px(HEIGHT))
            .split_body()
            .close(
                "Close settings",
                HoverHelp::tooltip(CLOSE_HELP),
                false,
                dismiss(weak),
            )
    }
}
