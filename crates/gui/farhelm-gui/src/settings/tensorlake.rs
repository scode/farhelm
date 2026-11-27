//! Settings → Tensorlake: the helm's Tensorlake account, for hosts and sessions that are Tensorlake sandboxes.
//!
//! SPEC.md, "Session list" (the paragraph that starts "The **Tensorlake** section holds the helm's Tensorlake
//! account"); SPEC_impl.md, "Tensorlake account". Ported from the old app's `TensorlakeSettings`
//! (`tensorlake_settings.rs`) and its look from the `.tensorlake-experimental`, `.host-settings-section` and
//! `.host-settings-section-title` rules in `app.css`, all at reference commit `24bc0580`, with the old words. Here's
//! the TLDR of its rules:
//!
//! - **The warning comes first, always.** The maintainer's EXPERIMENTAL warning heads the section, above the API key
//!   every Tensorlake user has to set up, so nobody starts without seeing it, and it is drawn whether or not the
//!   account read answers (SPEC.md: "it shows whether or not the section's settings load"). Its words are the
//!   maintainer's, reproduced exactly (2026-11-04); change them only on their say.
//! - **The key form is the shared one.** The section draws its own copy of the Tensorlake API key form over the
//!   opening's account ([`super::tensorlake_key`]), so a Save or Clear here shows in Credentials at once and each keeps
//!   its own typed key.
//! - **The create-without-asking switch shows only what the helm stored.** It can authorize billed sandboxes without a
//!   question, so, as in the old section, it never shows a choice the helm has not acknowledged (SPEC.md: it "saves
//!   only when the helm acknowledges it and shows only the stored choice"): during its save it keeps showing the
//!   stored choice, a done save's answer becomes the account's, and a refusal leaves the stored choice with the helm's
//!   words under the switch. It is unavailable until the account read answers, and stays so after a failed read until
//!   Retry loading answers, since an off switch drawn then would be a guess. Its save names only its own field, under
//!   the preference row's key like every other Settings save ([`super::agent_trust`] has the reasoning).
//!
//! - **The SSH key group and Sandboxes not on this helm are views of their own** ([`super::tensorlake_ssh_key`],
//!   [`super::tensorlake_sandboxes`]), in the old order between the key form and "Creating sandboxes", over the same
//!   account.

use farhelm_proto::tensorlake::SaveSettings;
use gpui::{
    App, AppContext as _, Context, Div, Entity, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Role, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, div, px,
};

use super::choices::{self, GROUP_GAP};
use super::tensorlake_key::{self, TensorlakeAccount, TensorlakeKey};
use super::tensorlake_sandboxes::Sandboxes;
use super::tensorlake_ssh_key::SshKey;
use super::{Refused, card};
use crate::components::{Checkbox, HoverHelp};
use crate::drafts::{self, Draft};
use crate::host_text::HostSentence;
use crate::operations::{OpKey, Outcome};
use crate::theme::{self, metrics};

/// The maintainer's warning at the top of the section, word for word (see the module docs).
pub(super) const EXPERIMENTAL_WARNING: &str = "EXPERIMENTAL: Tensorlake support is currently early and experimental. \
                                               User experience is not yet reasonable. Expect rough edges. Things will \
                                               change randomly between releases. Try at your own risk. You are \
                                               responsible for making sure sandboxes are not running and billing you \
                                               when you don't want them to.";

/// The warning's accessible name, the old note's `aria-label`.
pub(super) const EXPERIMENTAL_NAME: &str = "Tensorlake support is experimental";

/// The help under the warning, the old words.
pub(super) const INTRO: &str = "Tensorlake sandboxes are cloud machines Farhelm can create and use as hosts. Farhelm creates them \
                     in the Tensorlake project your API key belongs to.";

/// The switch's element id; its failure line is `-error` after it.
pub(super) const ASK_ID: &str = "tensorlake-create-without-asking";
const ASK_ERROR_ID: &str = "tensorlake-create-without-asking-error";
pub(super) const ASK_LABEL: &str = "Create Tensorlake sandboxes without asking";
pub(super) const ASK_TOOLTIP: &str = "Skip the confirmation when adding a Tensorlake host, which creates a sandbox that bills \
                           while it runs";
pub(super) const ASK_HELP: &str = "Adding a Tensorlake host or launching a recipe session creates a sandbox, which bills while \
                        running, so Farhelm asks first. This skips that question. \"Set up new hosts without asking\" \
                        under Hosts never does.";

/// The note under the switch about Tensorlake's own idle suspend and the sessions' fixed timeout (SPEC.md: "on
/// Tensorlake's Usage Credits plan Tensorlake suspends an ordinary host sandbox after 24 hours without a connection
/// from Farhelm"), the old words.
pub(super) const USAGE_CREDITS_NOTE: &str = "On Tensorlake's Usage Credits plan, Tensorlake suspends a sandbox after \
                                             24 hours without a connection from Farhelm, for example while this \
                                             helm's machine is asleep. Sandbox sessions instead have a fixed \
                                             30-minute idle timeout. Their processes and files remain until you \
                                             explicitly delete the sandbox; Resume continues them.";

/// `.tensorlake-experimental`: its padding (top and bottom, then the sides), its text size and its line (1.45 of
/// 15 pixels), and the room under it.
const WARNING_PAD: (f32, f32) = (14., 16.);
const WARNING_TEXT: f32 = 15.;
const WARNING_LINE_HEIGHT: f32 = 21.75;
const WARNING_MARGIN: f32 = 14.;

/// The space between the warning, introduction and groups.
const PART_GAP: f32 = 14.;

/// The switch's request: its own field and nothing else, so it cannot undo a key another form saved.
fn ask_save(create_without_asking: bool) -> SaveSettings {
    SaveSettings {
        create_without_asking: Some(create_without_asking),
        ..SaveSettings::default()
    }
}

/// The Tensorlake section, a view of its own that Settings makes the first time Tensorlake shows in an opening and
/// keeps until the dialog closes, so its key form's typed key and its switch's failure survive section switches.
pub(super) struct Tensorlake {
    ask_focus: gpui::FocusHandle,
    account: Entity<TensorlakeAccount>,
    /// The section's copy of the API key form.
    key: Entity<TensorlakeKey>,
    /// The SSH key group, under the key form.
    ssh_key: Entity<SshKey>,
    /// Sandboxes not on this helm, under the SSH key group.
    sandboxes: Entity<Sandboxes>,
    /// The switch's save: whether it is in flight, and the helm's words for the last refusal. Its value is the choice
    /// being saved, which the switch never shows (see the module docs).
    ask: Draft<bool>,
    /// Whether the switch's last press sent nothing because another Settings save held the key ([`Refused`]).
    refused: Refused,
    /// Only its close continuation is used: a close waiting on the switch's save resumes from its completion.
    pub(super) save_receipt: super::editing::SaveReceipt,
    /// Redraws the section when the account changes: the read, a retry, or a save from either key form.
    _account: Subscription,
}

impl Tensorlake {
    /// The section over `account`, the opening's, with `key` as its key form, drawn in `window`.
    pub(super) fn new(
        account: Entity<TensorlakeAccount>,
        key: Entity<TensorlakeKey>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let observing = cx.observe(&account, |_, _, cx| cx.notify());
        let ssh_key = cx.new(|cx| SshKey::new(account.clone(), window, cx));
        let sandboxes = cx.new(|cx| Sandboxes::new(account.clone(), cx));
        Tensorlake {
            ask_focus: cx.focus_handle().tab_stop(true),
            account,
            key,
            ssh_key,
            sandboxes,
            ask: Draft::load(false),
            refused: Refused::default(),
            save_receipt: super::editing::SaveReceipt::default(),
            _account: observing,
        }
    }

    /// Settings shows the section again in this opening: the SSH key group may read the account again
    /// ([`SshKey::shown_again`]).
    pub(super) fn shown_again(&mut self, cx: &mut Context<Self>) {
        self.ssh_key
            .update(cx, |ssh_key, cx| ssh_key.shown_again(cx));
    }

    /// The section's key form, whose masked field can take focus from a search result.
    pub(super) fn key(&self) -> Entity<TensorlakeKey> {
        self.key.clone()
    }

    /// The stored create-without-asking choice, as the helm last said; `None` until the account read answers.
    fn stored(&self, cx: &App) -> Option<bool> {
        self.account
            .read(cx)
            .settings()
            .map(|settings| settings.create_without_asking)
    }

    /// The switch has a position only after the account read makes it available.
    pub(super) fn search_focus(&self, cx: &App) -> Option<gpui::FocusHandle> {
        self.stored(cx).map(|_| self.ask_focus.clone())
    }

    /// Whether the switch's save is out, which holds a close ([`super::SettingsDialog::close_busy`]).
    pub(super) fn holds_close(&self) -> bool {
        self.ask.is_saving()
    }

    /// The user turned the switch to `on`: save it (the module docs). Does nothing until the account is known, when
    /// the switch is drawn unavailable. A press during the switch's own save, or while another Settings save holds the
    /// key, sends nothing and says so under the switch.
    fn toggle(&mut self, on: bool, cx: &mut Context<Self>) {
        if self.stored(cx).is_none() {
            return;
        }
        if !self.ask.edit(on) {
            self.refused.refuse(ASK_ID);
            cx.notify();
            return;
        }
        let account = self.account.clone();
        let started = drafts::save(
            self,
            |section: &mut Self| &mut section.ask,
            &OpKey::preferences(),
            |on| tensorlake_key::save_command(&ask_save(*on)),
            cx,
            move |section, settled, cx| {
                if let Outcome::Done(body) = &settled.outcome {
                    let body = body.clone();
                    account.update(cx, |account, cx| account.acknowledged(body, cx));
                }
                let refused = matches!(settled.outcome, Outcome::Refused(_));
                section.save_receipt.settled_other(refused, cx);
            },
        );
        self.refused.record(ASK_ID, &started);
        if let Err(not_started) = started {
            tracing::debug!(
                on,
                ?not_started,
                "Tensorlake create-without-asking not saved"
            );
        }
        cx.notify();
    }

    /// The maintainer's warning: the danger fill with its stronger edge and the on-fill ink, bold
    /// (`.tensorlake-experimental`).
    fn warning(cx: &App) -> gpui::Stateful<Div> {
        let palette = theme::palette(cx);
        div()
            .id("tensorlake-experimental")
            .debug_selector(|| "tensorlake-experimental".to_owned())
            .role(Role::Note)
            .aria_label(EXPERIMENTAL_NAME)
            .mb(px(WARNING_MARGIN))
            .py(px(WARNING_PAD.0))
            .px(px(WARNING_PAD.1))
            .rounded(metrics::RADIUS_LG)
            .border_2()
            .border_color(theme::color(palette.danger_strong))
            .bg(theme::color(palette.danger_bg))
            .text_color(theme::color(palette.on_fill))
            .text_size(px(WARNING_TEXT))
            .line_height(px(WARNING_LINE_HEIGHT))
            .font_weight(FontWeight::BOLD)
            .child(EXPERIMENTAL_WARNING)
    }

    /// Let the creation switch own its help and outcomes, with the Usage Credits paragraph below the card.
    fn creating(&self, cx: &Context<Self>) -> gpui::Stateful<Div> {
        let stored = self.stored(cx);
        let weak = cx.weak_entity();
        let switch = Checkbox::new(
            ASK_ID,
            ASK_LABEL,
            HoverHelp::tooltip(ASK_TOOLTIP),
            Role::Switch,
        )
        .switch()
        .content(card::switch_copy(ASK_LABEL, ASK_HELP, cx))
        .focus(&self.ask_focus)
        .checked(stored == Some(true))
        .disabled(stored.is_none())
        .on_toggle(move |on, _, cx| {
            let _ = weak.update(cx, |section, cx| section.toggle(on, cx));
        });
        // The old line was the helm's words alone, with none of the app's before them.
        let failure = self.ask.refusal().map(|reason| {
            choices::host_failure_line(
                ASK_ERROR_ID,
                HostSentence::new(ASK_ERROR_ID).host(reason),
                cx,
            )
        });
        let group = div()
            .flex()
            .flex_col()
            .gap(px(GROUP_GAP))
            .min_w_0()
            .child(switch)
            .children(failure)
            .children(choices::busy_line(&self.refused, ASK_ID, cx));
        div()
            .id("tensorlake-creating")
            .role(Role::Region)
            .aria_label("Creating sandboxes")
            .flex()
            .flex_col()
            .gap(px(8.))
            .min_w_0()
            .child(
                card::heading("Creating sandboxes", None, None, cx)
                    .id("tensorlake-creating-heading")
                    .role(Role::Heading)
                    .aria_label("Creating sandboxes"),
            )
            .child(card::rows([group], cx))
            .child(choices::help(USAGE_CREDITS_NOTE, cx))
    }
}

impl Render for Tensorlake {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("tensorlake")
            .role(Role::Region)
            .aria_label(SharedString::from("Tensorlake"))
            .flex()
            .flex_col()
            .gap(px(PART_GAP))
            .w_full()
            .min_w_0()
            .child(Self::warning(cx))
            .child(choices::help(INTRO, cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(card::heading("API key", None, None, cx))
                    .child(card::rows([self.key.clone()], cx)),
            )
            .child(self.ssh_key.clone())
            .child(self.sandboxes.clone())
            .child(self.creating(cx))
    }
}
