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
use crate::session::{self, ObservingCurrent, PerSession};
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

/// The local hide, in-flight save and refusal belong to the UI session that received the click.
/// The new snapshot supplies the shared dismissal after a reset.
#[derive(Default)]
struct Known {
    /// The × is local to an occurrence. Evidence that the run captured or ended clears it, including off screen.
    hidden: HashSet<(String, HookAgent)>,
    saving: Option<HookAgent>,
    error: Option<(String, SharedString)>,
}

pub(super) struct HookBand {
    known: PerSession<Known>,
    /// The last selected hint and its shared dismissal. Keep the observation across resets so an inert
    /// window can retire the previously drawn hint.
    observed: Option<(String, HookAgent, bool)>,
    settings: FocusHandle,
    _store: ObservingCurrent,
}

impl HookBand {
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        Self {
            known: PerSession::default(),
            observed: None,
            settings: cx.focus_handle().tab_stop(true),
            _store: session::observe_current(cx, Self::follow),
        }
    }

    /// A missing row may only be outside the capped list. Clear the local dismissal when the pushed facts change.
    fn follow(&mut self, cx: &mut Context<Self>) {
        let known = self.known.current(cx);
        let observed = session::current_store(cx).and_then(|store| {
            let collections = store.read(cx).collections();
            known.hidden.retain(|(key, kind)| {
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
        let known = self.known.current(cx);
        known.error = None;
        let begun = operations.update(cx, |operations, cx| {
            operations.start(&OpKey::preferences(), command, cx, move |settled, cx| {
                let _ = band.update(cx, |band, cx| {
                    let known = band.known.current(cx);
                    known.saving = None;
                    if !matches!(settled.outcome, Outcome::Done(_)) {
                        known.error = Some((
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
            Ok(()) => known.saving = Some(agent),
            Err(NotStarted::Busy(_)) => known.error = Some((key, trust_band::busy())),
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
    let known = state.known.read(cx);
    if dismissed(agent, cx)
        || known.is_some_and(|known| {
            known.saving == Some(agent) || known.hidden.contains(&(key.to_owned(), agent))
        })
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
            band.known
                .current(cx)
                .hidden
                .insert((hidden_key.clone(), agent));
            cx.notify();
        });
    });
    let palette = theme::palette(cx);
    let failure = known
        .and_then(|known| known.error.as_ref())
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
    use farhelm_helm_client::Method;
    use farhelm_proto::PersistentHookSetup;
    use gpui::{TestAppContext, VisualTestContext};

    use super::ID;
    use crate::session;
    use crate::session_list::menu_tests::{push, snapshot};
    use crate::session_list::tests::session;
    use crate::test_support::{click, frame, open_main, settle};

    /// Inspect the hint above the open session's terminal after the view has drawn its changes.
    fn shown(cx: &mut VisualTestContext) -> bool {
        frame(cx)
            .iter()
            .any(|node| node.element_id.as_deref() == Some(ID))
    }

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

    /// A reconnect shows the selected run's hint again after a local hide or a lost dismissal reply.
    /// Both cases reconnect to the same eligible row, with no shared dismissal stored.
    #[gpui::test]
    fn hook_hint_dismissals_start_over_after_a_ui_reset(cx: &mut TestAppContext) {
        let row = session("a")
            .agent_kind("grok")
            .waiting_for_hooks(PersistentHookSetup::NotRequested);
        let snapshot = || snapshot(&[(row.clone(), 0)], false);
        let mut main = open_main(cx, snapshot());
        main.terminal_live();
        assert!(shown(main.cx), "the premise: the hook hint is shown");

        for button in ["hook-setup-band-hide", "hook-setup-band-dismiss"] {
            let old = main
                .cx
                .update(|_, cx| session::current_ui_session(cx))
                .unwrap();
            click(main.cx, button);
            assert!(!shown(main.cx), "the press hid the hint");
            if button == "hook-setup-band-dismiss" {
                assert!(
                    main.helm.commands().iter().any(|command| {
                        command.method == Method::Put
                            && command.path == "/api/preferences"
                            && command.body
                                == Some(serde_json::json!({"grok_hook_hint_dismissed": true}))
                    }),
                    "the dismissal save is outstanding"
                );
            }
            main.helm.break_channel();
            settle(main.cx);
            assert!(
                main.cx
                    .update(|_, cx| session::current_ui_session(cx))
                    .is_none()
            );
            main.helm.connect(snapshot());
            settle(main.cx);
            let new = main
                .cx
                .update(|_, cx| session::current_ui_session(cx))
                .unwrap();
            assert_ne!(new, old, "the snapshot replaced the UI session");
            assert!(shown(main.cx), "the new UI session shows the eligible hint");
        }
    }
}
