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
//!   build has are listed, in SPEC.md's order ([`Section::ALL`]). Each section's state lives in the dialog for as long
//!   as it is open, so a hidden section keeps its pending saves and failure lines (and, once sections with text fields
//!   exist, their unsaved text); only the shown one is drawn, so Tab never reaches a hidden control.
//! - **Opening and closing.** [`open`] is the gear's entry point. It opens at Sessions with focus on its first switch
//!   (SPEC.md: "The gear always opens sessions and focuses its first switch"). [`open_at_agent_trust`] is a trust
//!   hint's: it opens at Agent trust and puts focus on that agent's switch once the switch can take it. Escape, the
//!   header's close control and the footer's Close all close it, and focus goes back to the gear. Closing discards what
//!   the dialog holds; a save already sent goes on, since it is the operation store's work, not the dialog's (the
//!   helm's reply then settles nothing on screen, and the store already shows the result).
//! - **Saving a choice.** Every switch here, Appearance's choice and Terminal's link choice save one field of the
//!   helm's preference row at once, with [`crate::drafts::save`] under [`OpKey::preferences`]; Agent trust's trust
//!   switches save the same way through their own routes ([`agent_trust`]). While the save is in flight the control
//!   shows the value being saved (its draft); otherwise it shows what the helm stores, so a refused save shows the
//!   stored value again, with the failure line under the control until its next save starts (SPEC.md: "A failed save
//!   restores the last known value and shows an adjacent error"). Nothing is optimistic, queued, retried or read back.
//!
//! NOTE: Quick flips do not follow the old app's "newest-choice" rule. Its preference queue coalesced a field's clicks,
//! sent only the newest, and reported a failure only for the newest click. Here the operation store admits one request
//! on the preference row's key at a time, and every Settings save holds that key, so a click on any switch or radio
//! option while any Settings save is in flight is refused and changes nothing: the control goes on showing the value in
//! flight, and the next click after the reply acts. On the radios (Appearance, Terminal's link choice) an arrow press
//! moves focus before it reports the choice, so a dropped arrow press still moves focus while the choice stays where it
//! was. The helm is embedded and answers a preference write in milliseconds, so the window in which a click is dropped
//! this way is that short. The migration's orchestrator accepted this on 2026-11-14, to be revisited only if a user
//! meets it; closing it would take a key per field or a queue, both changes to the operation store, not here.
//!
//! NOTE: Not ported yet, each recorded in `docs/gpui/parity/settings-and-hosts.md`. The one-pane layout at 620 pixels
//! wide and below ("‹ Settings" back to the list) is not built, since the window's minimum width is above it. A
//! section switch does not move focus to the shown section's heading, as the old app did so a keyboard user went on
//! from there: the focus coordinator moves focus to a control only while no layer is open, and the heading is not a
//! control. The section list is not pinned while a long section scrolls (the old `position: sticky`); no section here
//! is long enough to scroll at the minimum window size.

mod agent_trust;
mod appearance;
mod choices;
mod terminal;
#[cfg(test)]
mod tests;

use farhelm_proto::ui_channel::{PREFERENCES_KEY, Preferences};
use gpui::{
    AnyElement, AnyWindowHandle, App, AppContext as _, Context, Entity, FocusHandle, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Role,
    StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, div, px, svg,
};

use crate::assets::Icon;
use crate::components::{Button, DialogFrame, HoverHelp, Tier};
use crate::drafts::{self, Draft};
use crate::focus::{self, Mark, ReturnTo};
use crate::layers::{self, Dialog, LayerId};
use crate::operations::{OpKey, Outcome};
use crate::session;
use crate::theme::{self, Preference};

use agent_trust::{Known, Trust};
use choices::{Choice, Switch};

pub use agent_trust::TrustAgent;
use terminal::LinkClick;

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
/// `.app-settings-panel > h3`: the shown section's heading size.
const HEADING_TEXT: f32 = 18.;
/// The heading's line: `line-height: normal` at 18 pixels; GPUI's default line is taller.
const HEADING_LINE_HEIGHT: f32 = 22.;
/// `.host-settings-section`'s gap between a section's heading and its groups.
const SECTION_GAP: f32 = 14.;

/// The header's summary, the old dialog's words.
///
/// NOTE: "and for this device" described the old app, which kept appearance and the status sounds per device. In this
/// app they are helm preferences (the maintainer's decision of 2026-11-13), so only the desktop app's automatic-update
/// choice (Settings → App, not built yet) is this device's. The migration's orchestrator decided on 2026-11-14 to keep
/// the words as they are, since they are true again once App is built.
const SUMMARY: &str = "For every host on this helm, and for this device";

/// The footer's note, the old desktop app's words.
///
/// NOTE: It names editors that later sections bring (credentials, the Tensorlake API key, ntfy alerts, checkout fields
/// and file types); none of them is built yet, and every choice built so far saves immediately.
const FOOTER_NOTE: &str = "Most choices save immediately; credentials, the Tensorlake API key, ntfy alerts, checkout \
                           fields and file types use Save.";

/// The close controls' shared hover help, the old dialog's.
const CLOSE_HELP: &str = "Close settings; unsaved fields are discarded";

/// Open Settings at Sessions with focus on its first switch, on top of everything else in `window`: the gear's entry
/// point (SPEC.md: "The gear always opens sessions and focuses its first switch").
///
/// `opener` is the gear's focus handle and `mark` the [`Mark`] taken in its click handler. The gear takes focus first,
/// so that closing Settings returns focus to it on every platform, also where a click leaves a button unfocused (macOS):
/// the old app returned focus to the gear explicitly (`RETURN_FOCUS`), and SPEC.md has "Escape and either close control
/// return focus to the gear". Nothing opens, and the gear takes no focus, when the user has moved on since `mark` or the
/// window is inert.
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
    open_dialog(section, opener, mark, window, cx).map(|(layer, _, _)| layer)
}

/// [`open_at`], handing back the dialog and its own focus handle too, for an entry point that tells it more.
fn open_dialog(
    section: Section,
    opener: &FocusHandle,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<(LayerId, Entity<SettingsDialog>, FocusHandle)> {
    // An inert window has no store to draw the switches from, and nothing to save through.
    session::current_store(cx)?;
    if focus::has_moved_on(mark, cx) {
        return None;
    }
    focus::focus_control(opener, mark, window, cx);
    let dialog = cx.new(|cx| SettingsDialog::new(section, cx));
    let container = cx.focus_handle();
    let start = dialog
        .read(cx)
        .start(section)
        .unwrap_or_else(|| container.clone());
    let layer = layers::open_dialog(
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
            returns_to: ReturnTo::Opener,
            // None: closing never waits. A save in flight goes on in the operation store after the dialog is gone.
            on_dismiss: None,
            placement: layers::DialogPlacement::Centered,
            dismiss_on_outside_press: false,
        },
        mark,
        window,
        cx,
    )?;
    dialog.update(cx, |dialog, cx| {
        dialog.layer = Some(layer);
        dialog.show(section, cx);
    });
    Some((layer, dialog, container))
}

/// Open Settings at Agent trust with focus on `agent`'s switch: a trust hint's "Settings and caveats" (SPEC.md: "Trust
/// hints instead open agent trust at the relevant switch and scroll its block into view"). `opener` and `mark` are as
/// [`open`]'s: the hint's button and the mark taken in its click handler.
///
/// The switch is unavailable until the section's read of the agent's trust answers ([`agent_trust`]), and an
/// unavailable control cannot hold focus, so the dialog opens with focus on itself and hands focus to the switch once
/// the read has answered. The handover is skipped when the user has moved on since `mark` (any key press or click, a
/// click that leaves focus on the dialog included), or moved focus off the dialog in the meantime. If the read fails,
/// focus stays on the dialog, beside the failure line.
///
/// NOTE: The switch is not scrolled into view, so Muse's and Pi's lie below the fold at the dialog's full height, and
/// Codex's too when the window caps the dialog's height (in a window shorter than about 730 pixels; the window's
/// minimum height is 560). Only Claude's switch is always visible. The handover borrows [`focus::move_within_control`],
/// which moves without revealing its target and whose documented callers are a radio group and an inline question; see
/// [`SettingsDialog::trust_known`] for why that borrowed use is safe here. It waits for `focus::focus_in_layer` to
/// accept a target that is not drawn yet, which reveals the switch (not the agent's whole block, which SPEC.md's
/// "scroll its block into view" asks for).
///
/// NOTE: Until then, opening this for any agent but Claude can focus a trust switch the user cannot see (Muse's and
/// Pi's always, Codex's in a short window), and Space toggles it: a user who presses Space to scroll to the switch
/// turns that agent's trust on unseen. A hint band that calls this before the reveal lands should either not call it
/// for an agent whose switch lies below the fold, or accept that risk knowingly.
///
/// NOTE: Trust is not pushed. A hint band that shows only while its agent's trust is off (SPEC.md) does not learn from
/// this function that the user turned trust on in Settings: it returns only the layer, and the dialog has no dismissal
/// hook. The old band read trust again when Settings closed; the session view's band needs its own re-read when the
/// dialog it opened closes (for example, a dismissal hook offered here when the band is built).
pub fn open_at_agent_trust(
    agent: TrustAgent,
    opener: &FocusHandle,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let (layer, dialog, container) = open_dialog(Section::AgentTrust, opener, mark, window, cx)?;
    // A fresh dialog has just sent its reads, so the switch is not available yet: focus waits for the answer.
    let handover = Handover {
        agent,
        mark: *mark,
        from: container,
        window: window.window_handle(),
    };
    dialog.update(cx, |dialog, _| dialog.focus_on = Some(handover));
    Some(layer)
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
    Terminal,
    AgentTrust,
}

impl Section {
    /// The sections this build has, in SPEC.md's order ("Sessions, Appearance, Hosts, Recipes, Tensorlake,
    /// Containers, Credentials, Terminal, Checkouts, Agent trust and Notifications, followed by App"). The others join
    /// with their own changes.
    const ALL: &[Section] = &[
        Section::Sessions,
        Section::Appearance,
        Section::Hosts,
        Section::Terminal,
        Section::AgentTrust,
    ];

    /// The section's stable key, which its button's element id is built from.
    fn key(self) -> &'static str {
        match self {
            Section::Sessions => "sessions",
            Section::Appearance => "appearance",
            Section::Hosts => "hosts",
            Section::Terminal => "terminal",
            Section::AgentTrust => "agent-trust",
        }
    }

    /// The section's name, on its button and as its heading.
    fn label(self) -> &'static str {
        match self {
            Section::Sessions => "Sessions",
            Section::Appearance => "Appearance",
            Section::Hosts => "Hosts",
            Section::Terminal => "Terminal",
            Section::AgentTrust => "Agent trust",
        }
    }

    /// The section's glyph on its button, the old app's (`settings_section_icon`).
    fn icon(self) -> Icon {
        match self {
            Section::Sessions => Icon::Sessions,
            Section::Appearance => Icon::Appearance,
            Section::Hosts => Icon::Hosts,
            Section::Terminal => Icon::Terminal,
            Section::AgentTrust => Icon::AgentTrust,
        }
    }

    /// The control focus starts on when Settings opens at this section: its first switch. `None` for Appearance and
    /// Terminal, whose first control is a radio group: its options' focus handles live in the component's own element
    /// state, out of the dialog's reach, so opening there focuses the dialog itself (see [`open_at`]). Nothing in the
    /// app opens Settings at either; only their goldens do. `None` for Agent trust too, whose first switch is
    /// unavailable until the section's read answers ([`open_at_agent_trust`]).
    fn first(self) -> Option<Choice> {
        match self {
            Section::Sessions => Some(Choice::SkipSessionConfirmations),
            Section::Appearance => None,
            Section::Hosts => Some(Choice::AutoUpdateRemoteHosts),
            Section::Terminal => None,
            Section::AgentTrust => None,
        }
    }
}

/// The open dialog: which section is shown, every section's state, and its layer.
pub(crate) struct SettingsDialog {
    /// The section drawn beside the list; the others keep their state undrawn.
    shown: Section,
    /// One per [`Choice`], in [`Choice::ALL`]'s order, whichever section draws it.
    switches: Vec<Switch>,
    /// Appearance's radio group: like a switch's draft, it holds the value being saved and the last refusal, and its
    /// loaded value never reaches the screen ([`appearance::section`]).
    appearance: Draft<Preference>,
    /// Terminal's link choice: like a switch's draft, it holds the value being saved and the last refusal, and its
    /// loaded value never reaches the screen ([`terminal::links`]).
    link_click: Draft<LinkClick>,
    /// Agent trust's switches, in [`TrustAgent::ALL`]'s order. What they know comes from the read the section sends
    /// the first time it is shown in this opening; `trust_read` says whether it was sent.
    trust: Vec<Trust>,
    trust_read: bool,
    /// The handover a trust hint asked for ([`open_at_agent_trust`]), until its agent's read answers. Left set if that
    /// read fails, which is harmless: the read is sent once per opening, so nothing can match it again in this opening.
    focus_on: Option<Handover>,
    /// Set right after the layer opens; what the close controls dismiss.
    layer: Option<LayerId>,
}

/// What a trust hint's entry point asks of the dialog ([`open_at_agent_trust`]): once `agent`'s read answers, focus
/// moves from the dialog itself (`from`, in `window`) to that agent's switch, unless the user has moved on since
/// `mark`.
struct Handover {
    agent: TrustAgent,
    mark: Mark,
    from: FocusHandle,
    window: AnyWindowHandle,
}

impl SettingsDialog {
    /// A dialog showing `shown`, with no save of its own yet.
    fn new(shown: Section, cx: &mut Context<Self>) -> Self {
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
            switches,
            appearance: Draft::load(Preference::System),
            link_click: Draft::load(LinkClick::Intercept),
            trust: TrustAgent::ALL
                .iter()
                .map(|&agent| Trust {
                    agent,
                    known: Known::Reading,
                    draft: Draft::load(false),
                    focus: cx.focus_handle().tab_stop(true),
                })
                .collect(),
            trust_read: false,
            focus_on: None,
            layer: None,
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
    fn start(&self, section: Section) -> Option<FocusHandle> {
        let first = section.first()?;
        Some(self.switches[Self::index(first)].focus.clone())
    }

    /// Show `section`. Every section's state stays as it is: only which one is drawn changes. Showing Agent trust for
    /// the first time in this opening reads the agents' trust choices.
    fn show(&mut self, section: Section, cx: &mut Context<Self>) {
        self.shown = section;
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

    /// The read of `agent`'s trust answered: the switch shows it, and takes focus if [`open_at_agent_trust`] asked for
    /// it and the user has not moved on since.
    fn trust_known(
        &mut self,
        agent: TrustAgent,
        answer: Result<bool, crate::host_text::HostText>,
        cx: &mut Context<Self>,
    ) {
        let trust = &mut self.trust[agent.index()];
        trust.known = match answer {
            Ok(enabled) => Known::Read(enabled),
            Err(reason) => Known::Unreadable(reason),
        };
        // The switch takes focus from the dialog itself ([`open_at_agent_trust`]). This borrows
        // `focus::move_within_control` outside its documented callers (a radio group's arrows and an inline question).
        // It is safe here for the same two reasons those are: it moves focus only from the dialog itself, which still
        // holds it only if the user has not moved focus anywhere since the dialog opened, and the target is drawn in
        // the same layer by the next frame, which this update's notify asks for (the function's contract). The mark
        // check covers what focus alone cannot: a click that leaves focus on the dialog still counts as moving on.
        //
        // TODO: switch to `focus::focus_in_layer`, under the same mark, once it accepts a target that is not drawn yet;
        // it also scrolls the switch into view.
        if trust.available()
            && let Some(handover) = self.focus_on.take_if(|handover| handover.agent == agent)
            && !focus::has_moved_on(&handover.mark, cx)
        {
            let to = trust.focus.clone();
            // A window that has closed meanwhile has nothing to focus, so its error is ignored.
            let _ = handover.window.update(cx, |_, window, cx| {
                focus::move_within_control(&handover.from, &to, window, cx)
            });
        }
        cx.notify();
    }

    /// The user turned `agent`'s trust switch to `on`: save it through the agent's trust route, under the preference
    /// row's key like every Settings choice ([`agent_trust`]). Once the helm has done it, that is the choice the
    /// section holds; a refusal leaves the choice read before.
    fn toggle_trust(&mut self, agent: TrustAgent, on: bool, cx: &mut Context<Self>) {
        let index = agent.index();
        if !self.trust[index].available() || !self.trust[index].draft.edit(on) {
            return;
        }
        let started = drafts::save(
            self,
            move |dialog: &mut SettingsDialog| &mut dialog.trust[index].draft,
            &OpKey::preferences(),
            |on| agent.command(*on),
            cx,
            move |dialog, settled, _| {
                if matches!(settled.outcome, Outcome::Done(_)) {
                    dialog.trust[index].known = Known::Read(on);
                }
            },
        );
        if let Err(not_started) = started {
            tracing::debug!(?agent, ?not_started, "agent trust not saved");
        }
    }

    /// The user turned `choice`'s switch to `on`: save it as the preference row's field. Does nothing while that
    /// switch's own save is in flight, or while any other preference save is (the operation store refuses the key; see
    /// the module docs).
    fn toggle(&mut self, choice: Choice, on: bool, cx: &mut Context<Self>) {
        let index = Self::index(choice);
        if !self.switches[index].draft.edit(on) {
            return;
        }
        let started = drafts::save(
            self,
            move |dialog: &mut SettingsDialog| &mut dialog.switches[index].draft,
            &OpKey::preferences(),
            |on| choice.command(*on),
            cx,
            |_, _, _| {},
        );
        if let Err(not_started) = started {
            tracing::debug!(?choice, ?not_started, "preference switch not saved");
        }
    }

    /// The user chose `preference` in Appearance: save it as the preference row's `appearance` field, as
    /// [`SettingsDialog::toggle`] saves a switch, with the same refusal while any preference save is in flight. The
    /// palette changes only once the helm stores the choice and pushes it, like every other client's.
    fn choose_appearance(&mut self, preference: Preference, cx: &mut Context<Self>) {
        if !self.appearance.edit(preference) {
            return;
        }
        let started = drafts::save(
            self,
            |dialog: &mut SettingsDialog| &mut dialog.appearance,
            &OpKey::preferences(),
            |preference| appearance::command(*preference),
            cx,
            |_, _, _| {},
        );
        if let Err(not_started) = started {
            tracing::debug!(?preference, ?not_started, "appearance not saved");
        }
    }

    /// The user chose `choice` for terminal web links: save it as the preference row's `terminal_link_click` field, as
    /// [`SettingsDialog::toggle`] saves a switch, with the same refusal while any preference save is in flight.
    fn choose_link_click(&mut self, choice: LinkClick, cx: &mut Context<Self>) {
        if !self.link_click.edit(choice) {
            return;
        }
        let started = drafts::save(
            self,
            |dialog: &mut SettingsDialog| &mut dialog.link_click,
            &OpKey::preferences(),
            |choice| terminal::command(*choice),
            cx,
            |_, _, _| {},
        );
        if let Err(not_started) = started {
            tracing::debug!(?choice, ?not_started, "terminal link choice not saved");
        }
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
            move |on, _, cx| {
                let _ = weak.update(cx, |dialog, cx| dialog.toggle(choice, on, cx));
            },
            cx,
        )
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
                choices::help(
                    "The host choices apply to every host on this helm. Other open clients pick them up right away.",
                    cx,
                )
                .into_any_element(),
                self.switch(Choice::SkipHostSetupConfirmation, preferences, cx),
                self.switch(Choice::SkipHostRemoveConfirmation, preferences, cx),
            ],
            Section::Terminal => {
                let weak = cx.weak_entity();
                terminal::links(
                    &self.link_click,
                    preferences,
                    move |&choice, _, cx| {
                        let _ = weak.update(cx, |dialog, cx| dialog.choose_link_click(choice, cx));
                    },
                    cx,
                )
                .into_iter()
                .map(IntoElement::into_any_element)
                .collect()
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
                        move |on, _, cx| {
                            let _ = weak.update(cx, |dialog, cx| dialog.toggle_trust(agent, on, cx));
                        },
                        |choice| self.switch(choice, preferences, cx),
                        cx,
                    )
                    .into_any_element()
                })
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let store = session::current_store(cx);
        let preferences = store
            .as_ref()
            .and_then(|store| preferences(store.read(cx).collections()))
            .cloned();
        let weak = cx.weak_entity();

        // `.app-settings-navigation`: the section buttons, 6 pixels apart, with a rule between them and the section.
        let navigation = div()
            .id("settings-sections")
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
            .children(Section::ALL.iter().map(|&section| {
                let weak = weak.clone();
                Button::new(
                    format!("settings-section-{}", section.key()),
                    section.label(),
                    HoverHelp::tooltip(format!("Show {} settings", section.label())),
                    Role::Button,
                )
                .tier(Tier::Nav)
                .icon(section.icon())
                .pressed(section == self.shown)
                .on_click(move |_, _, cx| {
                    let _ = weak.update(cx, |dialog, cx| dialog.show(section, cx));
                })
            }));
        // `.app-settings-panel`: the shown section under its heading.
        let shown = div()
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
            .children(self.body(self.shown, preferences.as_ref(), cx));
        let body = div()
            .flex()
            .items_start()
            .gap(px(20.))
            .text_color(theme::color(palette.fg_0))
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
        DialogFrame::new("settings", "Settings", body, vec![close])
            .summary(SUMMARY)
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
