//! The launcher's "Create a sandbox for this session?": the inline question it asks before sending a create that would
//! make a billed environment, and the consent its "Create and launch" gives that create.
//!
//! SPEC.md, "Sandbox sessions" (the question, its words, the setting that skips it) and "Keyboard focus" (a question
//! with a risky answer starts on its harmless one). Ported from the old launcher's sandbox question in
//! `list/create_form.rs` at reference commit `24bc0580`, drawn by the prompt foundation's inline question in the
//! launcher's notices region, where the YOLO question goes (`yolo_question` is the pattern). Here's the TLDR:
//!
//! - **When.** A press of Launch (or anything that submits the draft) whose request is a new environment the provider
//!   asks about ([`crate::environment_ui::ProviderUi::creation_question`]) asks first and sends nothing. The provider
//!   decides: a Tensorlake sandbox asks unless the account as the launcher read it has "Create Tensorlake sandboxes
//!   without asking" on, and then the create goes unconfirmed and the helm checks the setting again; a container never
//!   asks.
//! - **Out of sight.** A quick launch prepared out of sight (`prepare_unseen`) reaches the same press without the
//!   launcher on screen, and its launcher comes back to show the question rather than create anything, as the old app
//!   revealed its hidden quick launch to ask. The question is asked when the launcher is next shown, under the same
//!   moved-on rule as a refusal (behind the notice if the user moved on).
//! - **The answers.** Cancel ("create no sandbox and keep the launcher open") sends nothing and leaves the launcher and
//!   its draft as they were. "Create and launch" submits the same draft once more, with consent, which sends exactly
//!   one create with `confirmed` set. Focus starts on Cancel, the harmless answer (SPEC.md, "Opening and closing": "A
//!   risky action that cannot be undone starts on Cancel"; the old question started on Create and launch). A
//!   "Tensorlake pricing" link sits between the words and the answers ([`crate::prompt::Question::link`]).
//! - **Consent is for one create.** Create and launch's consent is an argument of the one submit it makes; any later
//!   submit asks again, a press after the helm refused that create included, as the old launcher's did.
//! - **The skip is spent by one create.** The setting is read once, as the launcher opens, so it may be stale: turned
//!   off since, the helm refuses the unconfirmed create, and a launcher that trusted its copy again would send the same
//!   create into the same refusal at every press. Once it has sent a create on the setting, this launcher asks instead,
//!   without reading the account again (decided by the GPUI migration's coordinator on 2026-11-16, after the gate
//!   review found the loop).
//! - **Retired by an edit.** Any edit of the draft takes the question away (`Launcher::changed`), as it does the YOLO
//!   question's: it asked about the draft as it was.

use farhelm_proto::session_requests::EnvironmentCreateBody;
use gpui::{Context, Window};

use super::Launcher;
use crate::components::{HoverHelp, Tier};
use crate::environment_ui::{CreationQuestion, ProviderUi};
use crate::focus::Mark;
use crate::host_text::{HostSentence, HostText};
use crate::launcher::request::Recipes;
use crate::prompt::{self, Answer, Inline, Question};

/// What [`crate::environment_ui::ProviderUi::creation_question`] is given in place of the account once this launcher
/// has spent its skip setting: an account it does not trust, which asks. Never shown.
const SKIP_SPENT: &str = "the skip setting was used by an earlier create";

/// What the question is about, frozen as it is worded: the recipe's name and the provider's words for it, its size
/// included, as the catalog held them at the press. The answer launches the launcher's draft, which cannot have
/// changed meanwhile, since an edit retires the question.
#[derive(Clone, Debug)]
pub(in crate::launcher) struct SandboxAsk {
    recipe: HostText,
    words: CreationQuestion,
}

/// The question's state in the launcher: what it asks about, and the question once asked (`None` while the launcher is
/// out of sight, until it is next shown).
pub(in crate::launcher) struct SandboxState {
    ask: SandboxAsk,
    inline: Option<Inline<SandboxAsk>>,
}

impl Launcher {
    /// At a submit that would send `body`, a new environment's create, decide whether the creation question comes
    /// first. Returns `true` when it does: the question is set up (and asked now if the launcher is shown, for the
    /// user's action at `mark`), and the caller sends nothing. Otherwise `body` goes out, `confirmed` only when
    /// `consent` says this submit is Create and launch's. A create sent unconfirmed only because of the account's skip
    /// setting spends it for this launcher ([`Launcher::sandbox_skip_spent`]), so a later submit asks.
    ///
    /// `recipes` is the catalog the submit checked the draft against; the draft's refusal check has already passed, so
    /// it lists the recipe. Were it not to, the create goes unconfirmed, and the helm refuses one it would bill for.
    pub(in crate::launcher) fn hold_for_creation_question(
        &mut self,
        body: &mut EnvironmentCreateBody,
        recipes: &Recipes,
        consent: bool,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if consent {
            body.confirmed = true;
            return false;
        }
        let Recipes::Listed(listed) = recipes else {
            return false;
        };
        let Some(recipe) = listed.iter().find(|recipe| recipe.name == body.recipe) else {
            return false;
        };
        let provider = body.place.provider();
        let account = if self.sandbox_skip_spent {
            Err(SKIP_SPENT)
        } else {
            self.account.settings()
        };
        let Some(words) = ProviderUi(provider).creation_question(recipe, account) else {
            // A provider that asks, not asking: the account's skip let this create go unconfirmed.
            if provider.asks_before_creating() {
                self.sandbox_skip_spent = true;
            }
            return false;
        };
        self.sandbox = Some(SandboxState {
            ask: SandboxAsk {
                recipe: HostText::from_host(recipe.name.as_str()),
                words,
            },
            inline: None,
        });
        if self.layer.is_some() {
            self.ask_pending_sandbox(mark, window, cx);
        }
        cx.notify();
        true
    }

    /// Ask the pending creation question, now that the launcher is shown for the user's action at `mark`: right after
    /// the press that needs it, or when a launcher prepared out of sight is brought back. Asked once: a question
    /// already asked stays as it is.
    pub(in crate::launcher) fn ask_pending_sandbox(
        &mut self,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.sandbox.as_mut() else {
            return;
        };
        if state.inline.is_some() {
            return;
        }
        let launcher = cx.weak_entity();
        state.inline = Some(prompt::ask_inline(
            question(&state.ask),
            mark,
            move |_: SandboxAsk, window, cx| {
                // The answer is the press the launch's moved-on check counts from.
                let mark = Mark::take(cx);
                let _ = launcher.update(cx, |launcher, cx| {
                    launcher.submit(false, true, mark, window, cx)
                });
            },
            window,
            cx,
        ));
        cx.notify();
    }

    /// The question while it is up, for drawing where the launcher's notices go.
    pub(in crate::launcher) fn sandbox_inline(&self) -> Option<&Inline<SandboxAsk>> {
        self.sandbox
            .as_ref()
            .and_then(|state| state.inline.as_ref())
    }
}

/// The question for `ask`: the title, the recipe, its size and what creating it means, the pricing link, then Cancel
/// and "Create and launch", starting on Cancel. Plain, not loud: the old question was an ordinary confirmation beside
/// the launcher's actions, unlike the YOLO one.
fn question(ask: &SandboxAsk) -> Question<SandboxAsk> {
    let words = &ask.words;
    Question::new(
        words.title,
        Answer::new(
            words.confirm_label,
            HoverHelp::tooltip(words.confirm_help),
            Tier::Primary,
        ),
        ask.clone(),
    )
    .line(
        HostSentence::new("sandbox-question-recipe")
            .text("Recipe ")
            .host(&ask.recipe),
    )
    .line(HostSentence::new("sandbox-question-size").text(words.size.clone()))
    .line(HostSentence::new("sandbox-question-consequence").text(words.consequence))
    .link(
        words.link_label,
        words.link_url,
        HoverHelp::tooltip(words.link_help),
    )
    .cancel_answer(Answer::new(
        "Cancel",
        HoverHelp::tooltip(words.cancel_help),
        Tier::Neutral,
    ))
    .starts_on_cancel()
}
