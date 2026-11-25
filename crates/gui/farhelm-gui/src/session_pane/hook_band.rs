//! The setup hint belongs to the open run's reported facts. A previously installed callback can still capture a
//! conversation, so the hint disappears as soon as Restart stops waiting for a capture.

use std::collections::HashSet;

use farhelm_proto::RestartOffer;
use farhelm_proto::ui_channel::{PREFERENCES_KEY, Preferences, SessionRow};
use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement as _,
    ParentElement as _, Role, SharedString, StatefulInteractiveElement as _, Styled as _, div,
};

use super::trust_band;
use crate::components::{Button, HoverHelp, Tier};
use crate::focus::Mark;
use crate::hook_setup::HookAgent;
use crate::operations::{NotStarted, OpKey, Outcome};
use crate::session::{self, ObservingCurrent};
use crate::{settings, theme};

pub(super) const ID: &str = "hook-setup-band";

/// The hint is for missing setup on a run that could resume if it reported a conversation. Failed installs have
/// their own notification and a reason at Restart.
fn agent(row: &SessionRow) -> Option<HookAgent> {
    if row.info.restart_offer == RestartOffer::NotCaptured
        && row.info.persistent_hook_setup.is_not_requested()
        && !row.info.status.has_ended()
    {
        HookAgent::for_kind(row.info.agent_kind)
    } else {
        None
    }
}

pub(super) struct HookBand {
    /// The × is local to an occurrence. Evidence that the run captured or ended clears it, including off screen.
    hidden: HashSet<(String, HookAgent)>,
    /// The selected session's eligible hint and its shared dismissal. Changes here wake the pane on pushes.
    observed: Option<(String, HookAgent, bool)>,
    saving: Option<HookAgent>,
    error: Option<(String, SharedString)>,
    settings: FocusHandle,
    _store: ObservingCurrent,
}

impl HookBand {
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        Self {
            hidden: HashSet::new(),
            observed: None,
            saving: None,
            error: None,
            settings: cx.focus_handle().tab_stop(true),
            _store: session::observe_current(cx, Self::follow),
        }
    }

    /// A missing row may only be outside the capped list. Clear the local dismissal when the pushed facts change.
    fn follow(&mut self, cx: &mut Context<Self>) {
        let observed = session::current_store(cx).and_then(|store| {
            let collections = store.read(cx).collections();
            self.hidden.retain(|(key, kind)| {
                collections
                    .get::<SessionRow>(key)
                    .is_none_or(|row| agent(row) == Some(*kind))
            });
            let key = crate::local_state::selected(cx)?;
            let kind = agent(collections.get::<SessionRow>(&key)?)?;
            Some((key, kind, dismissed(kind, cx)))
        });
        if self.observed != observed {
            self.observed = observed;
            cx.notify();
        }
    }

    /// Store the same preference as the launcher prompt. A failed save restores the band and explains the refusal.
    fn dismiss(&mut self, key: String, agent: HookAgent, cx: &mut Context<Self>) {
        let Some(operations) = session::current_operations(cx) else {
            return;
        };
        let Ok(command) = settings::dismiss_hook_setup_command(agent) else {
            return;
        };
        let band = cx.weak_entity();
        let settling = key.clone();
        self.error = None;
        let begun = operations.update(cx, |operations, cx| {
            operations.start(&OpKey::preferences(), command, cx, move |settled, cx| {
                let _ = band.update(cx, |band, cx| {
                    band.saving = None;
                    if !matches!(settled.outcome, Outcome::Done(_)) {
                        band.error = Some((
                            settling,
                            "Could not save hiding the hook setup prompt and hint. Try again."
                                .into(),
                        ));
                    }
                    cx.notify();
                });
            })
        });
        match begun {
            Ok(()) => self.saving = Some(agent),
            Err(NotStarted::Busy(_)) => self.error = Some((key, trust_band::busy())),
            Err(error) => tracing::debug!(?error, "hook setup dismissal was not sent"),
        }
        cx.notify();
    }
}

/// The launcher and the band share one pushed preference for each agent.
fn dismissed(agent: HookAgent, cx: &App) -> bool {
    session::current_store(cx).and_then(|store| {
        store
            .read(cx)
            .collections()
            .get::<Preferences>(PREFERENCES_KEY)
            .and_then(|prefs| agent.dismissal(prefs))
    }) == Some(true)
}

/// Offer setup in the visible agent terminal. The switch itself is read only by the settings page and launcher.
pub(super) fn draw(
    band: &Entity<HookBand>,
    key: &str,
    row: &SessionRow,
    cx: &App,
) -> Option<AnyElement> {
    let agent = agent(row)?;
    let state = band.read(cx);
    if dismissed(agent, cx)
        || state.saving == Some(agent)
        || state.hidden.contains(&(key.to_owned(), agent))
    {
        return None;
    }
    let opener = state.settings.clone();
    let settings = Button::new(
        "hook-setup-band-settings",
        "Settings",
        HoverHelp::tooltip("Open this agent's hook setup setting").below(),
        Role::Button,
    )
    .tier(Tier::Primary)
    .focus(&state.settings)
    .on_click(move |_, window, cx| {
        settings::open_at_hook_setup(agent, &opener, &Mark::take(cx), window, cx);
    });
    let saving = band.downgrade();
    let saved_key = key.to_owned();
    let dismiss = Button::new(
        "hook-setup-band-dismiss",
        "Don't show again",
        HoverHelp::tooltip("Hide the hook setup prompt and hint for this agent").below(),
        Role::Button,
    )
    .tier(Tier::Neutral)
    .on_click(move |_, _, cx| {
        let _ = saving.update(cx, |band, cx| band.dismiss(saved_key.clone(), agent, cx));
    });
    let hiding = band.downgrade();
    let hidden_key = key.to_owned();
    let hide = Button::new(
        "hook-setup-band-hide",
        "×",
        HoverHelp::tooltip("Hide this hint for this session").below(),
        Role::Button,
    )
    .tier(Tier::Neutral)
    .accessible_name("Hide this hook setup hint for this session")
    .on_click(move |_, _, cx| {
        let _ = hiding.update(cx, |band, cx| {
            band.hidden.insert((hidden_key.clone(), agent));
            cx.notify();
        });
    });
    let palette = theme::palette(cx);
    let failure = state
        .error
        .as_ref()
        .filter(|(failed, _)| failed == key)
        .map(|(_, words)| {
            div()
                .id("hook-setup-band-error")
                .role(Role::Alert)
                .aria_label(words.clone())
                .text_color(theme::color(palette.danger))
                .child(words.clone())
        });
    Some(
        trust_band::strip(ID, palette)
            .child(trust_band::copy(
                "hook-setup-band-copy",
                "Farhelm didn’t set up conversation hooks for this session. ",
                "Restart is unavailable until Farhelm learns which conversation to resume.",
                palette,
            ))
            .child(
                trust_band::action_row()
                    .child(settings)
                    .child(dismiss)
                    .child(hide),
            )
            .children(failure)
            .into_any_element(),
    )
}

/// Draw the production band for a known session result in a component screenshot.
#[cfg(test)]
pub(crate) fn golden(cx: &mut App) -> AnyElement {
    use gpui::AppContext as _;

    let band = cx.new(HookBand::new);
    let fixture = crate::session_list::tests::session("a")
        .agent_kind("grok")
        .waiting_for_hooks(farhelm_proto::PersistentHookSetup::NotRequested);
    let row = serde_json::from_value(fixture.entry().value).expect("the hook hint fixture decodes");
    draw(&band, "a", &row, cx).expect("the missing setup result draws a hint")
}

#[cfg(test)]
mod tests {
    use farhelm_proto::PersistentHookSetup;
    use gpui::TestAppContext;

    use super::ID;
    use crate::session_list::menu_tests::{push, snapshot};
    use crate::session_list::tests::session;
    use crate::test_support::{click, frame, open_main};

    /// The hint is bound to a run's missing setup while it waits for a capture. A pushed capture closes the
    /// occurrence, even after a local dismissal. A failed install leaves the session usable and has no hint.
    /// The visible explanation is also the value provided to screen readers for announcements.
    #[gpui::test]
    fn hook_setup_hint_follows_the_reported_run_without_a_settings_read(cx: &mut TestAppContext) {
        let grok = session("a").agent_kind("grok");
        let missing = grok
            .clone()
            .waiting_for_hooks(PersistentHookSetup::NotRequested);
        let mut main = open_main(cx, snapshot(&[(missing.clone(), 0)], false));
        main.terminal_live();
        let shown = |cx: &mut gpui::VisualTestContext| {
            frame(cx)
                .iter()
                .any(|node| node.element_id.as_deref() == Some(ID))
        };
        assert!(shown(main.cx), "a live uncaptured Grok run has no setup");
        let copy = frame(main.cx)
            .into_iter()
            .find(|node| node.element_id.as_deref() == Some("hook-setup-band-copy"))
            .expect("the setup hint explains why Restart is unavailable");
        assert_eq!(
            copy.value.as_deref(),
            Some(
                "Farhelm didn’t set up conversation hooks for this session. \
                  Restart is unavailable until Farhelm learns which conversation to resume."
            )
        );
        assert!(
            !main
                .helm
                .commands()
                .iter()
                .any(|cmd| cmd.path.starts_with("/api/agent-hook-setup/"))
        );

        click(main.cx, "hook-setup-band-hide");
        assert!(!shown(main.cx), "the occurrence was dismissed locally");
        push(&mut main.helm, &["a"], &[(grok.clone(), 0)], main.cx);
        assert!(!shown(main.cx), "the callback captured a conversation");
        let failed = grok.waiting_for_hooks(PersistentHookSetup::Failed {
            reason: "read-only".into(),
        });
        push(&mut main.helm, &["a"], &[(failed, 0)], main.cx);
        assert!(
            !shown(main.cx),
            "an installation failure uses the notification and Restart tooltip"
        );
        push(&mut main.helm, &["a"], &[(missing, 0)], main.cx);
        assert!(
            shown(main.cx),
            "a later occurrence of missing setup is visible"
        );
    }
}
