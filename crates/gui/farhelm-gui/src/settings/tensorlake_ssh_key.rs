//! Settings → Tensorlake's SSH key group: whether the helm has Farhelm's own Tensorlake SSH key, Create SSH key until
//! it does, and then the one-time command that registers the key with the user's Tensorlake account, with Copy.
//!
//! SPEC.md, "Session list" (the Tensorlake paragraph: "Until Farhelm's own Tensorlake SSH key ... exists, the section
//! offers **Create SSH key**; opening Settings never creates it. Once it exists, the section shows the one-time command
//! ... with a **Copy** button") and "Security"; SPEC_impl.md, "Tensorlake account". Ported from the SSH key group of the
//! old app's `TensorlakeSettings` (`tensorlake_settings.rs`) and its look from the `.tensorlake-command` and
//! `.tensorlake-settings pre` rules in `app.css`, all at reference commit `24bc0580`, with the old words. Here's the TLDR
//! of its rules:
//!
//! - **The key's state is the account's.** What the group shows comes from the opening's [`TensorlakeAccount`], read
//!   once per opening; nothing here reads the route on its own, and before the read answers the group shows only its
//!   help. Reading never creates the key: only the button does (SPEC.md: "opening Settings never creates it").
//! - **Create SSH key is a plain command.** It changes nothing a Settings control shows as a draft, so it is not work on
//!   a key: it goes out with [`crate::operations::send`], the button is busy while it is out, and a done answer, the
//!   helm's whole settings, becomes the account's, as a done save's does. A refusal shows the helm's words under the
//!   group.
//! - **A refusal reads the account again, once.** The helm answers Create SSH key within its host reply bound, and a
//!   generation still running then carries on after a 504 that says so and tells the user to select Create SSH key
//!   again, which waits for that generation and answers its key. The account is not pushed, and nothing that is pushed
//!   changes when the key is written, so the group reads the account once when a refusal comes, showing whatever the
//!   helm stores by then, and does not poll for it: SPEC.md, "Settings", treats the helm as a presumed low-latency
//!   remote and adds no machinery for someone to move on before a round trip settles. Any refusal does this, not only
//!   the 504: telling them apart would take the reply's status, which a plain command does not carry
//!   ([`crate::operations::answer`]). A refusal's line shows only while the key is still not created; once a read finds
//!   the key, the line goes. A read that fails shows the helm's reason with Retry loading in the API key forms, as any
//!   failed account read does.
//! - **When that read finds nothing, say so.** Once the read after a refusal has answered and the account still shows
//!   no key, a line under the refusal says to try again ([`TRY_AGAIN`]), until the next press or a key. While the
//!   account's last read failed it is hidden, since that read learned nothing about the key.
//! - **The key appearing moves focus to Copy.** Whichever answer first shows the key after a press of Create SSH key
//!   (the done answer, or a read after a refusal) replaces the button that held focus with the command, so focus goes to
//!   Copy, the control that takes its place, unless the user has moved on since the press. Left to the
//!   focus coordinator's recovery, it would land on the next control, which is the switch that creates sandboxes without
//!   asking, so one more Space would turn on billing without a question.
//! - **Copy says it copied, for a moment.** The old Copy confirmed with "Command copied" for 1.5 seconds; the group
//!   holds the details bar's copy confirmation ([`CopyFeedback`]) for it, so a later click restarts it. What it copies is
//!   the command's raw text, the helm's, which the group draws as host text.

use farhelm_helm_client::Method;
use farhelm_proto::tensorlake::SshKeyStatus;
use gpui::{
    AnyWindowHandle, App, AppContext as _, Context, Div, Entity, FocusHandle,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Role, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, px,
};

use super::tensorlake_key::{TensorlakeAccount, path};
use super::{card, choices};
use crate::components::{Button, CommandWell, CopyFeedback, HoverHelp, Tier};
use crate::focus::{self, Mark};
use crate::host_text::{HostSentence, HostText};
use crate::theme;

/// The group's element id; its controls and lines are `-create`, `-command` (the command, whose Copy is
/// `-command-copy`), `-copied`, `-unavailable`, `-error` and `-try-again` after it.
pub(super) const ID: &str = "tensorlake-ssh-key";
pub(super) const LABEL: &str = "SSH key";

/// The lines saying why the group has no key to show, and why Create SSH key failed.
const UNAVAILABLE_ID: &str = "tensorlake-ssh-key-unavailable";
const ERROR_ID: &str = "tensorlake-ssh-key-error";
/// The line under a refusal once the read after it found no key, and its words.
const TRY_AGAIN_ID: &str = "tensorlake-ssh-key-try-again";
const TRY_AGAIN: &str = "No key has appeared since. Press Create SSH key to try again.";

/// The old words.
pub(super) const HELP: &str = "Farhelm reaches sandboxes over SSH with a key of its own, kept in this helm's state. Register it \
                    once with your Tensorlake account by running the command below on any machine where Tensorlake's \
                    tl CLI is signed in.";
const NOT_CREATED: &str = "This helm has no SSH key for Tensorlake yet.";
const UNCHECKED: &str = "Farhelm cannot check that the key is registered, because an API key cannot list SSH keys. An \
                         unregistered key shows up as an error the first time Farhelm sets up a sandbox.";

/// The group's gap and the room above its title under the rule, as the section's other parts
/// (`.host-settings-section`, `.host-settings-section + .host-settings-section`).
const PART_GAP: f32 = 14.;

/// The SSH key group, a view of its own that the Tensorlake section makes with itself and keeps for the opening.
pub(super) struct SshKey {
    account: Entity<TensorlakeAccount>,
    /// Whether Create SSH key is out.
    creating: bool,
    /// The helm's words for the last refused Create SSH key, until the next one is sent.
    failure: Option<HostText>,
    /// The account's count of answers when the last refusal asked for a read ([`TensorlakeAccount::answers`]): a larger
    /// count means the account has answered since, and with no key in it, the line saying to try again shows.
    read_after_refusal: Option<u64>,
    /// Copy's confirmation, which "Command copied" shows while it lasts.
    copied: Entity<CopyFeedback>,
    /// Copy's focus handle, where a done Create SSH key moves focus.
    copy_focus: FocusHandle,
    /// The window the group is drawn in, for that move: an answer arrives without one.
    window: AnyWindowHandle,
    /// When Create SSH key was last pressed, until the key appears and focus moves to Copy (the module docs).
    pressed: Option<Mark>,
    /// The account's changes and the confirmation's, which redraw the group.
    _account: Subscription,
    _copied: Subscription,
}

impl SshKey {
    /// The group over `account`, the opening's, drawn in `window`.
    pub(super) fn new(
        account: Entity<TensorlakeAccount>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let observing = cx.observe(&account, |group: &mut Self, _, cx| group.follow_account(cx));
        let copied = cx.new(|_| CopyFeedback::default());
        let copied_changes = cx.observe(&copied, |_, _, cx| cx.notify());
        SshKey {
            account,
            creating: false,
            failure: None,
            read_after_refusal: None,
            copied,
            copy_focus: cx.focus_handle().tab_stop(true),
            window: window.window_handle(),
            pressed: None,
            _account: observing,
            _copied: copied_changes,
        }
    }

    /// The account changed: once it shows a key, a refused create's line has nothing left to say, and a press of Create
    /// SSH key that has not yet moved focus moves it to Copy (the module docs).
    fn follow_account(&mut self, cx: &mut Context<Self>) {
        if matches!(self.status(cx), Some(SshKeyStatus::Ready { .. })) {
            self.failure = None;
            self.read_after_refusal = None;
            if let Some(mark) = self.pressed.take() {
                // Copy is drawn by the next frame; the coordinator keeps the move until then.
                let target = self.copy_focus.clone();
                let _ = self.window.update(cx, |_, window, cx| {
                    focus::focus_in_layer(&target, &mark, window, cx)
                });
            }
        }
        cx.notify();
    }

    /// The key's state as the account last said it; `None` until the read answers.
    fn status(&self, cx: &App) -> Option<SshKeyStatus> {
        self.account
            .read(cx)
            .settings()
            .map(|settings| settings.ssh_key.clone())
    }

    /// Create SSH key, pressed at `mark`: ask the helm to generate the key. Does nothing while one is out or once the
    /// account knows a key other than "not created", when the button is busy or not drawn.
    fn create(&mut self, mark: Mark, cx: &mut Context<Self>) {
        if self.creating || self.status(cx) != Some(SshKeyStatus::NotCreated) {
            return;
        }
        let path = match path(&["ssh-key"]) {
            Ok(path) => path,
            Err(error) => {
                tracing::debug!(?error, "Create SSH key path not built");
                return;
            }
        };
        let group = cx.weak_entity();
        let sent = crate::operations::send(
            Method::Post,
            path,
            None,
            "POST /api/tensorlake/ssh-key",
            cx,
            move |answer, cx| {
                let _ = group.update(cx, |group, cx| group.created(answer, cx));
            },
        );
        if sent {
            self.creating = true;
            self.failure = None;
            self.read_after_refusal = None;
            self.pressed = Some(mark);
            cx.notify();
        }
    }

    /// Create SSH key's answer: a done one is the helm's settings with the key in them, which the account takes (and
    /// whose key then moves focus, [`SshKey::follow_account`]); a refused one is shown in the helm's words and reads the
    /// account again, once (the module docs).
    fn created(
        &mut self,
        answer: Result<Option<serde_json::Value>, HostText>,
        cx: &mut Context<Self>,
    ) {
        self.creating = false;
        match answer {
            Ok(body) => {
                self.account
                    .update(cx, |account, cx| account.acknowledged(body, cx));
            }
            Err(reason) => {
                self.failure = Some(reason);
                self.read_after_refusal = Some(self.account.read(cx).answers());
                self.account
                    .update(cx, |account, cx| account.read_again(cx));
            }
        }
        cx.notify();
    }

    /// The registration command in its well with Copy beside it (`.tensorlake-command`), Copy confirming each copy
    /// through the group's [`CopyFeedback`] and taking the focus a done Create SSH key moves to it.
    fn command(&self, command: &str) -> CommandWell {
        let text = HostText::from_host(command.to_owned());
        let copied = self.copied.clone();
        let raw = text.raw().to_owned();
        CommandWell::new(
            format!("{ID}-command"),
            text,
            "Copy",
            HoverHelp::tooltip(
                "Copy: the command that registers Farhelm's SSH key with your Tensorlake account",
            ),
        )
        .on_copy(move |cx| copied.update(cx, |copied, cx| copied.confirm(&raw, cx)))
        .accessible_name("Copy the registration command")
        .focus(&self.copy_focus)
    }
}

impl Render for SshKey {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let status = self.status(cx);
        let ready = matches!(status, Some(SshKeyStatus::Ready { .. }));
        let not_created = status == Some(SshKeyStatus::NotCreated);
        let state: Option<Div> = match status {
            None => None,
            Some(SshKeyStatus::NotCreated) => {
                let weak = cx.weak_entity();
                let create = Button::new(
                    format!("{ID}-create"),
                    "Create SSH key",
                    HoverHelp::tooltip(
                        "Create the SSH key Farhelm uses for Tensorlake sandboxes, in this helm's state",
                    ),
                    Role::Button,
                )
                .tier(Tier::Neutral)
                // Busy, not disabled, while it is out, so the pressed button keeps focus (SPEC_impl.md, "Drafts").
                .busy(self.creating)
                .on_click(move |_, _, cx| {
                    let mark = Mark::take(cx);
                    let _ = weak.update(cx, |group, cx| group.create(mark, cx));
                });
                Some(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(PART_GAP))
                        .child(choices::help(NOT_CREATED, cx))
                        // In a row of its own, so the button is as wide as its words (`.tensorlake-settings > .btn`).
                        .child(div().flex().child(create)),
                )
            }
            Some(SshKeyStatus::Ready {
                register_command, ..
            }) => Some(div().child(self.command(&register_command))),
            Some(SshKeyStatus::Unavailable { reason }) => Some(
                div().child(choices::host_failure_line(
                    UNAVAILABLE_ID,
                    HostSentence::new(UNAVAILABLE_ID)
                        .text("Farhelm has no usable SSH key for Tensorlake: ")
                        .host(&HostText::from_host(reason)),
                    cx,
                )),
            ),
        };
        let failure = self.failure.as_ref().map(|reason| {
            choices::host_failure_line(ERROR_ID, HostSentence::new(ERROR_ID).host(reason), cx)
        });
        // Drawn as the refusal is, under it, once the read the refusal sent has answered and the account, not failing,
        // still shows no key (the module docs).
        let account = self.account.read(cx);
        let gave_up = self.failure.is_some()
            && self
                .read_after_refusal
                .is_some_and(|asked| account.answers() > asked)
            && not_created
            && account.unreadable().is_none();
        let try_again = gave_up.then(|| choices::failure_line(TRY_AGAIN_ID.into(), TRY_AGAIN, cx));
        // Under the command, in the group's own gap, as the old line was the section's next paragraph.
        let copied = (ready && self.copied.read(cx).confirming()).then(|| {
            choices::outcome_line(
                format!("{ID}-copied"),
                Role::Status,
                theme::color(palette.fg_1),
            )
            .aria_label("Command copied")
            .child("Command copied")
        });
        div()
            .id(ID)
            .role(Role::Region)
            .aria_label("Farhelm's SSH key")
            .flex()
            .flex_col()
            .gap(px(8.))
            .w_full()
            .min_w_0()
            .child(
                card::heading(LABEL, None, None, cx)
                    .id(SharedString::from(format!("{ID}-heading")))
                    .role(Role::Heading)
                    .aria_label(LABEL),
            )
            .child(card::rows(
                [div()
                    .flex()
                    .flex_col()
                    .gap(px(PART_GAP))
                    .child(choices::help(HELP, cx))
                    .children(state)
                    .children(copied)
                    .children(failure)
                    .children(try_again)
                    .child(choices::help(UNCHECKED, cx))],
                cx,
            ))
    }
}
