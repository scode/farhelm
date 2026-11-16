//! The Tensorlake API key form: the helm's one write-only Tensorlake key, with Save, Test and Clear, and what Settings
//! knows of the Tensorlake account in one opening ([`TensorlakeAccount`]).
//!
//! SPEC.md, "Session list" (the Credentials paragraph that starts "Credentials includes the same Tensorlake API key
//! form as Tensorlake") and SPEC_impl.md, "Named credential catalog" (its last paragraph). Ported from the old app's
//! `TensorlakeKey` and `AccountState` (`tensorlake_settings.rs` at reference commit `24bc0580`), with their words.
//! Here's the TLDR of its rules:
//!
//! - **One account per opening, one draft per form.** SPEC.md puts the same form in Credentials and in Tensorlake, and
//!   a save or clear in either updates both forms' saved-key metadata at once while each keeps its own unsaved key. So
//!   what the helm acknowledged lives in a [`TensorlakeAccount`] the Settings dialog makes once per opening and hands
//!   to each form, and the typed key lives in the form. The account keeps the helm's whole answer, not only whether a
//!   key is saved, because Settings → Tensorlake draws its other parts (the create-without-asking switch) from it too.
//! - **Two places, one form.** Each form is made for its [`Place`]. In Credentials it is labelled "Tensorlake API key"
//!   and shows no storage warning, since the overview's names this key; in Tensorlake it is labelled "API key" and
//!   shows its own warning, worded for that one key (SPEC.md: "in Tensorlake the key form shows its own warning"). Each
//!   place's second help line points at the other.
//! - **Read once.** `/api/tensorlake` is helm configuration the helm does not push, so the account reads it when it is
//!   made, and a reply reaches only that opening. Until it answers, every control is locked and the field has no
//!   placeholder; a failed read shows the helm's reason with "Retry loading", and nothing is editable until a retry
//!   answers (the old "a failed load seeds nothing editable"). Whatever else reads the account in that opening (the
//!   Tensorlake section's switch) is locked the same way.
//! - **Write-only.** The helm never returns the key. Save sends only the typed key, Clear only the clear flag, so
//!   neither can undo a change made meanwhile to the account's other fields. A done Save or Clear takes the helm's
//!   answer as the account's new metadata and empties the field; a refusal keeps the typed key with the helm's words.
//! - **Saves are the operation store's work** under [`OpKey::preferences`], as every other piece of helm configuration
//!   in Settings saves (`super::agent_trust` has the reasoning): the account has no key of its own, and the embedded
//!   helm answers a key save in milliseconds. A press the key refuses says so under the form ([`Refused`]).
//! - **Test is not a save.** It asks the helm to list sandboxes with the saved key (never the field's text) and reports
//!   how many it sees; it changes nothing, so it is an ordinary command ([`crate::operations::send`]), as ntfy's Send
//!   test is. While Test or Clear is out the form is locked, as the old one was.

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::tensorlake::{SaveSettings, Settings, TestResult};
use gpui::{
    AnyWindowHandle, App, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Role, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, div, px,
};

use super::Refused;
use super::choices;
use crate::components::{Button, HoverHelp, TextInput, TextInputState, Tier};
use crate::drafts::DraftInput;
use crate::host_text::HostText;
use crate::operations::{Command, NotStarted, OpKey, Outcome, Work};
use crate::session;
use crate::theme;

/// The route the account reads and the form saves, as a refusal without words names it.
const ROUTE: &str = "/api/tensorlake";
/// The form's name for [`Refused`], and the prefix of its element ids: the field is `tensorlake-key`, its buttons
/// `tensorlake-key-save`, `-test` and `-clear`, its lines `tensorlake-key-outcome`, `-working` and `-busy`.
const ID: &str = "tensorlake-key";

/// `/api/tensorlake`, or with `tail` a route under it (Test's `test`, Create SSH key's `ssh-key`).
pub(super) fn path(tail: &[&str]) -> Result<ApiPath, PathError> {
    let segments: Vec<&str> = std::iter::once("tensorlake")
        .chain(tail.iter().copied())
        .collect();
    ApiPath::api(&segments)
}

/// `PUT /api/tensorlake` with `save`, which names only the field it changes. Turning it into JSON cannot fail (an
/// optional string and two flags); if it ever did, the request would go without a body and the helm would refuse it.
pub(super) fn save_command(save: &SaveSettings) -> Result<Command, PathError> {
    Ok(Command::new(
        Work::Save,
        Method::Put,
        path(&[])?,
        serde_json::to_value(save).ok(),
        format!("PUT {ROUTE}"),
    ))
}

/// Save's request: the typed key and nothing else.
fn key_save(key: &str) -> SaveSettings {
    SaveSettings {
        api_key: Some(key.to_owned()),
        ..SaveSettings::default()
    }
}

/// Clear's request: remove the key, touch nothing else.
fn key_clear() -> SaveSettings {
    SaveSettings {
        clear_api_key: true,
        ..SaveSettings::default()
    }
}

/// What a successful Test says, counting every sandbox the key can see.
fn test_message(result: TestResult) -> String {
    match result.sandboxes {
        1 => "The key works. It can see 1 sandbox.".into(),
        count => format!("The key works. It can see {count} sandboxes."),
    }
}

// ---- The account --------------------------------------------------------------------------------------------------

/// What one opening of Settings knows of the helm's Tensorlake account, as the helm last said it, shared by every key
/// form of that opening and by the Tensorlake section (see the module docs). Made by the dialog when a section that
/// needs it first shows, which is when it reads the route.
///
/// Every done save through `PUT /api/tensorlake` answers with the whole settings, and the account takes that answer
/// whole. The UI channel admits one command at a time and answers them in order (SPEC_impl.md, "The UI channel"), so
/// the read's answer and every save's reach the account in the order they were sent, and none carries an older stored
/// value than the answer before it. That is why taking an answer whole is safe here, where the old form, whose requests
/// could overtake each other, copied only the field its control owned.
pub(super) struct TensorlakeAccount {
    /// The helm's settings: the read's answer, then each done save's. `None` until the read answers.
    settings: Option<Settings>,
    /// Why the last read failed, until a read answers.
    unreadable: Option<HostText>,
    /// How many Saves and Clears of the key either form has had done in this opening. A replaced key can make a list
    /// read with the old one wrong even while a key stays saved, so the list of Sandboxes not on this helm reads again
    /// when this moves ([`super::tensorlake_sandboxes`]).
    key_saves: u64,
}

impl TensorlakeAccount {
    /// The account, reading the helm's Tensorlake settings now.
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        Self::read(cx);
        TensorlakeAccount {
            settings: None,
            unreadable: None,
            key_saves: 0,
        }
    }

    /// The helm's settings as last answered; `None` until the read answers, and while it keeps failing.
    pub(super) fn settings(&self) -> Option<&Settings> {
        self.settings.as_ref()
    }

    /// How many Saves and Clears of the key have been done in this opening ([`TensorlakeAccount::key_saves`]'s field).
    pub(super) fn key_saves(&self) -> u64 {
        self.key_saves
    }

    /// Why the account could not be read, until a read answers.
    fn unreadable(&self) -> Option<&HostText> {
        self.unreadable.as_ref()
    }

    /// Read the account again: Retry loading after a failed read, and the SSH key group after a Create SSH key the helm
    /// did not answer as done, whose key may still be on its way ([`super::tensorlake_ssh_key`]). Until the read answers,
    /// what the account showed stays shown.
    pub(super) fn read_again(&mut self, cx: &mut Context<Self>) {
        self.unreadable = None;
        Self::read(cx);
        cx.notify();
    }

    /// Read `/api/tensorlake`, the answer going to this account if its opening is still open, under
    /// [`choices::read_route`]'s contract.
    fn read(cx: &mut Context<Self>) {
        let account = cx.weak_entity();
        choices::read_route(path(&[]), format!("GET {ROUTE}"), cx, move |answer, cx| {
            let _ = account.update(cx, |account, cx| account.answered(answer, cx));
        });
    }

    /// The helm said what it stores: after the read, or as a done Save's or Clear's answer.
    fn answered(&mut self, answer: Result<Settings, HostText>, cx: &mut Context<Self>) {
        match answer {
            Ok(settings) => {
                self.settings = Some(settings);
                self.unreadable = None;
            }
            Err(reason) => self.unreadable = Some(reason),
        }
        cx.notify();
    }

    /// A done save of the account answered with `body`, the settings the helm now stores. A body that does not decode
    /// (the helm always answers with its settings, so this is a helm the GUI does not understand) reads the account
    /// again: the save is done, but what it stored is unknown, and the account is otherwise read only once per opening,
    /// so the controls would go on showing the choice from before the save until Settings is opened again.
    pub(super) fn acknowledged(&mut self, body: Option<serde_json::Value>, cx: &mut Context<Self>) {
        self.take_answer(body, cx);
    }

    /// A done Save or Clear of the key answered with `body`: [`TensorlakeAccount::acknowledged`], and one more key save
    /// counted.
    fn key_saved(&mut self, body: Option<serde_json::Value>, cx: &mut Context<Self>) {
        self.key_saves += 1;
        self.take_answer(body, cx);
    }

    /// [`TensorlakeAccount::acknowledged`]'s work.
    fn take_answer(&mut self, body: Option<serde_json::Value>, cx: &mut Context<Self>) {
        match body.map(serde_json::from_value::<Settings>) {
            Some(Ok(settings)) => self.answered(Ok(settings), cx),
            Some(Err(_)) | None => Self::read(cx),
        }
    }
}

// ---- The form -----------------------------------------------------------------------------------------------------

/// Where a key form is drawn, which decides its words (the module docs). Both places' forms share their element ids,
/// which is fine because Settings draws one section at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Place {
    /// Settings → Credentials, under the overview's storage warning, which names this key.
    Credentials,
    /// Settings → Tensorlake, with a storage warning of its own.
    Tensorlake,
}

impl Place {
    /// The field's label, which is also its accessible name: Credentials names the product to tell this key from the
    /// named secrets around it, while the Tensorlake section's heading already names it.
    fn label(self) -> &'static str {
        match self {
            Place::Credentials => "Tensorlake API key",
            Place::Tensorlake => "API key",
        }
    }

    /// The second help line, which points at the other place, in the old form's words.
    fn elsewhere(self) -> &'static str {
        match self {
            Place::Credentials => {
                "The same key as in Settings → Tensorlake. Changing it here changes it there."
            }
            Place::Tensorlake => "You can also change it in Settings → Credentials.",
        }
    }

    /// Whether the form draws its own "Not encrypted on disk" warning: only where no other warning names the key.
    fn warns(self) -> bool {
        match self {
            Place::Credentials => false,
            Place::Tensorlake => true,
        }
    }
}

/// The Tensorlake section's storage warning, the old singular `StorageWarning` body.
const STORAGE_WARNING: &str = "Farhelm stores this key in plain text in this helm's state, on the machine the helm \
                               runs on. Anyone who can read that folder can read it. Farhelm never shows a saved \
                               value again.";

/// The key form, a view of its own over the opening's [`TensorlakeAccount`]. Settings makes one with Credentials and
/// one with Tensorlake, and keeps each until the dialog closes, so its typed key survives section switches and the
/// credential editor.
pub(super) struct TensorlakeKey {
    account: Entity<TensorlakeAccount>,
    /// Where it is drawn ([`Place`]).
    place: Place,
    key: DraftInput,
    /// Whether Clear or Test is out; Save's own flight is the draft's.
    working: bool,
    /// How the last Save, Clear or Test went, with the typed key it applies to: the one sent, kept or tested, or the
    /// empty field a done Save or Clear leaves. Shown while the draft still holds that key
    /// ([`TensorlakeKey::outcome`]), so an edit makes it stale, as the old form's `oninput` cleared it; replaced by the
    /// next Save, Clear or Test.
    outcome: Option<(Result<SharedString, HostText>, String)>,
    /// Whether the last Save or Clear sent nothing because another Settings save held the key.
    refused: Refused,
    /// The window the form is drawn in, where a done Save or Clear empties the field.
    window: AnyWindowHandle,
    /// The account's changes, which every form of the opening follows.
    _account: Subscription,
}

impl TensorlakeKey {
    /// The form for `place` over `account`, with an empty field.
    pub(super) fn new(
        account: Entity<TensorlakeAccount>,
        place: Place,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // A Save or Clear in any form of this opening, or the read, changes what every form shows.
        let account_changes = cx.observe_in(&account, window, |form, _, window, cx| {
            form.placeholder(window, cx);
            form.hold(cx);
        });
        let key = Self::key_input(window, cx);
        let form = TensorlakeKey {
            account,
            place,
            key,
            working: false,
            outcome: None,
            refused: Refused::default(),
            window: window.window_handle(),
            _account: account_changes,
        };
        form.placeholder(window, cx);
        form.hold(cx);
        form
    }

    /// An empty key field. Enter in it saves nothing: the old field sat in no form, so Enter did nothing there. Its
    /// state's one subscription is its draft's (a text field's state takes one), so the form watches no edits itself.
    fn key_input(window: &mut Window, cx: &mut Context<Self>) -> DraftInput {
        let state = TextInputState::new_masked(window, cx);
        DraftInput::new(
            "",
            state,
            |form: &mut Self| &mut form.key,
            |_, _, _| {},
            window,
            cx,
        )
    }

    /// The outcome to show: the last one, while the draft still holds the key it applies to.
    fn outcome(&self) -> Option<&Result<SharedString, HostText>> {
        self.outcome
            .as_ref()
            .filter(|(_, typed)| typed == self.key.draft().value())
            .map(|(outcome, _)| outcome)
    }

    /// Empty the key field after a done Save or Clear, as the old form did, so the key is neither shown nor sent again.
    /// A fresh field, because a draft the user typed into never follows an empty value; the field needs the window,
    /// which a completion does not have, so this waits until the completion has returned.
    fn empty_later(&self, cx: &mut Context<Self>) {
        let (handle, form) = (self.window, cx.weak_entity());
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                let _ = form.update(cx, |form, cx| {
                    form.key = Self::key_input(window, cx);
                    form.placeholder(window, cx);
                    form.hold(cx);
                });
            });
        });
    }

    /// The typed key the form's draft holds, for tests: the field is masked, so what it paints says nothing.
    #[cfg(test)]
    pub(super) fn typed(&self) -> String {
        self.key.draft().value().clone()
    }

    /// Whether the helm has a key saved; `None` until the account's read answers.
    fn key_set(&self, cx: &App) -> Option<bool> {
        self.account
            .read(cx)
            .settings()
            .map(|settings| settings.api_key_set)
    }

    /// Whether the controls take no input: before the account's read has answered, and while Save, Clear or Test is
    /// out.
    fn locked(&self, cx: &App) -> bool {
        self.key_set(cx).is_none() || self.working || self.key.draft().is_saving()
    }

    /// Hold the field read-only while the form is locked, and redraw. Called whenever that may have changed.
    fn hold(&self, cx: &mut Context<Self>) {
        self.key.input().set_read_only(self.locked(cx), cx);
        cx.notify();
    }

    /// Give the field the placeholder that says whether a key is saved, the form's only sign of it, and none before
    /// the read has answered, as the old field did.
    fn placeholder(&self, window: &mut Window, cx: &mut Context<Self>) {
        let placeholder = match self.key_set(cx) {
            None => "",
            Some(true) => "Key saved; paste a new one to replace it",
            Some(false) => "No key saved",
        };
        self.key.input().set_placeholder(placeholder, window, cx);
    }

    /// Save the typed key. Does nothing while the form is locked or the field is blank, when Save is drawn busy or
    /// unavailable.
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.locked(cx) || self.key.draft().value().trim().is_empty() {
            return;
        }
        let started = DraftInput::save(
            self,
            |form| &mut form.key,
            &OpKey::preferences(),
            |key| save_command(&key_save(key)),
            window,
            cx,
            move |form, settled, cx| {
                match &settled.outcome {
                    Outcome::Done(body) => {
                        form.outcome = Some((Ok("API key saved".into()), String::new()));
                        let body = body.clone();
                        form.account
                            .update(cx, |account, cx| account.key_saved(body, cx));
                        form.empty_later(cx);
                    }
                    Outcome::Refused(refusal) => {
                        let typed = form.key.draft().value().clone();
                        form.outcome = Some((Err(refusal.words.clone()), typed));
                    }
                    Outcome::Accepted(_) => {}
                }
                form.hold(cx);
            },
        );
        self.started(started, cx);
    }

    /// Clear the saved key. Does nothing while the form is locked or no key is saved, when Clear is drawn busy or
    /// unavailable. A done Clear empties the typed key too, as the old form did.
    fn clear(&mut self, cx: &mut Context<Self>) {
        if self.locked(cx) || self.key_set(cx) != Some(true) {
            return;
        }
        let Some(operations) = session::current_operations(cx) else {
            return;
        };
        let command = match save_command(&key_clear()) {
            Ok(command) => command,
            Err(error) => {
                tracing::debug!(?error, "Tensorlake key clear not built");
                return;
            }
        };
        let form = cx.weak_entity();
        let started = operations.update(cx, |operations, cx| {
            operations.start(&OpKey::preferences(), command, cx, move |settled, cx| {
                let outcome = settled.outcome.clone();
                let _ = form.update(cx, |form, cx| {
                    form.working = false;
                    match outcome {
                        Outcome::Done(body) => {
                            form.outcome = Some((Ok("API key cleared".into()), String::new()));
                            form.account
                                .update(cx, |account, cx| account.key_saved(body, cx));
                            form.empty_later(cx);
                        }
                        Outcome::Refused(refusal) => {
                            let typed = form.key.draft().value().clone();
                            form.outcome = Some((Err(refusal.words), typed));
                        }
                        Outcome::Accepted(_) => {}
                    }
                    form.hold(cx);
                });
            })
        });
        if started.is_ok() {
            self.working = true;
        }
        self.started(started, cx);
    }

    /// After a Save or Clear was asked: a started one drops the last outcome and the busy line, a busy key says so
    /// under the form ([`Refused`]).
    fn started(&mut self, started: Result<(), NotStarted>, cx: &mut Context<Self>) {
        self.refused.record(ID, &started);
        match started {
            Ok(()) => self.outcome = None,
            Err(not_started) => tracing::debug!(?not_started, "Tensorlake key not saved"),
        }
        self.hold(cx);
    }

    /// Test: list sandboxes with the saved key (the module docs). Does nothing while the form is locked, no key is
    /// saved, or there is no UI session to send through.
    fn test(&mut self, cx: &mut Context<Self>) {
        if self.locked(cx) || self.key_set(cx) != Some(true) {
            return;
        }
        let path = match path(&["test"]) {
            Ok(path) => path,
            Err(error) => {
                tracing::debug!(?error, "Tensorlake test path not built");
                return;
            }
        };
        let shown = format!("POST {}", path.as_str());
        let named = shown.clone();
        let form = cx.weak_entity();
        let sent =
            crate::operations::send(Method::Post, path, None, shown, cx, move |answer, cx| {
                let result = crate::operations::decode::<TestResult>(answer, &named);
                let _ = form.update(cx, |form, cx| {
                    form.working = false;
                    let typed = form.key.draft().value().clone();
                    form.outcome = Some((result.map(|result| test_message(result).into()), typed));
                    form.hold(cx);
                });
            });
        if sent {
            self.working = true;
            self.outcome = None;
            self.hold(cx);
        }
    }
}

impl Render for TensorlakeKey {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let weak = cx.weak_entity();
        let key_set = self.key_set(cx);
        let busy = self.working || self.key.draft().is_saving();

        let field = choices::labelled(
            self.place.label(),
            ID,
            TextInput::new(
                ID,
                self.key.input(),
                self.place.label(),
                HoverHelp::tooltip(if key_set == Some(true) {
                    "Paste a new Tensorlake API key to replace the saved one"
                } else {
                    "Paste a Tensorlake API key"
                }),
            )
            .height(px(choices::FIELD_HEIGHT)),
        );

        let unloaded = self.account.read(cx).unreadable().cloned();
        let retry = unloaded.is_some().then(|| {
            let account = self.account.clone();
            Button::new(
                format!("{ID}-retry"),
                "Retry loading",
                HoverHelp::tooltip("Load the saved Tensorlake settings again"),
                Role::Button,
            )
            .tier(Tier::Neutral)
            .on_click(move |_, _, cx| {
                account.update(cx, |account, cx| account.read_again(cx));
            })
        });

        let no_key = key_set != Some(true);
        let test = Button::new(
            format!("{ID}-test"),
            "Test",
            HoverHelp::tooltip(
                "List your Tensorlake sandboxes with the saved key, to check that it works",
            ),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .disabled(key_set.is_none() || no_key)
        // Busy, not disabled, while working, so a clicked button keeps focus (SPEC_impl.md, "Drafts").
        .busy(busy)
        .on_click({
            let weak = weak.clone();
            move |_, _, cx| {
                let _ = weak.update(cx, |form, cx| form.test(cx));
            }
        });
        let clear = Button::new(
            format!("{ID}-clear"),
            "Clear",
            HoverHelp::tooltip("Remove the saved API key from this helm"),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .disabled(key_set.is_none() || no_key)
        .busy(busy)
        .on_click({
            let weak = weak.clone();
            move |_, _, cx| {
                let _ = weak.update(cx, |form, cx| form.clear(cx));
            }
        });
        let save = Button::new(
            format!("{ID}-save"),
            "Save",
            HoverHelp::tooltip("Save this API key for every window using this helm"),
            Role::Button,
        )
        .tier(Tier::Primary)
        .disabled(key_set.is_none() || self.key.draft().value().trim().is_empty())
        .busy(busy)
        .on_click(move |_, window, cx| {
            let _ = weak.update(cx, |form, cx| form.save(window, cx));
        });

        let outcome = self.outcome().map(|outcome| match outcome {
            Ok(words) => choices::outcome_line(
                format!("{ID}-outcome"),
                Role::Status,
                theme::color(palette.fg_1),
            )
            .aria_label(words.clone())
            .child(words.clone()),
            Err(reason) => choices::failure_outcome(format!("{ID}-outcome"), reason, cx),
        });

        div()
            .id("tensorlake-key-form")
            .role(Role::Region)
            .aria_label("Tensorlake API key")
            .flex()
            .flex_col()
            .gap(px(14.))
            .w_full()
            .min_w_0()
            .children(unloaded.as_ref().map(|reason| choices::failure_outcome(format!("{ID}-load-error"), reason, cx)))
            .children(retry)
            // Under the load failure and above the field, as the old form drew its `StorageWarning`, so the user meets
            // it before pasting a key.
            .children(self.place.warns().then(|| {
                super::credentials::storage_warning(
                    "tensorlake-key-storage-warning",
                    STORAGE_WARNING,
                    cx,
                )
            }))
            .child(field)
            .child(choices::help(
                "Create a key in the Tensorlake dashboard, in the project you want sandboxes in. Test lists your \
                 sandboxes to check that the saved key works.",
                cx,
            ))
            .child(choices::help(self.place.elsewhere(), cx))
            // `.dialog-actions`: Test, Clear and Save at the form's end, in the old order.
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .w_full()
                    .child(test)
                    .child(clear)
                    .child(save),
            )
            .children(busy.then(|| {
                choices::outcome_line(format!("{ID}-working"), Role::Status, theme::color(palette.fg_0))
                    .aria_label("Working…")
                    .child("Working…")
            }))
            .children(outcome)
            .children(choices::busy_line(&self.refused, ID, cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The key is write-only and the account's switch is a separate save, so each control must send only the field it
    /// owns; anything more would let a form that never saw a newer value overwrite it. The old form's
    /// `each_control_sends_only_its_own_field`, for the two controls this form has. Specified: Save sends only the key,
    /// Clear only the clear flag.
    #[test]
    fn each_control_sends_only_its_own_field() {
        let wire = |save: SaveSettings| serde_json::to_value(save).ok();
        assert_eq!(
            wire(key_save("k")),
            Some(serde_json::json!({ "api_key": "k" }))
        );
        assert_eq!(
            wire(key_clear()),
            Some(serde_json::json!({ "clear_api_key": true }))
        );
    }

    /// The count is the visible proof that the key reached the account. Specified: the message counts sandboxes with
    /// the right plural, zero included.
    #[test]
    fn the_test_message_counts_sandboxes() {
        assert_eq!(
            test_message(TestResult { sandboxes: 1 }),
            "The key works. It can see 1 sandbox."
        );
        assert_eq!(
            test_message(TestResult { sandboxes: 0 }),
            "The key works. It can see 0 sandboxes."
        );
    }
}
