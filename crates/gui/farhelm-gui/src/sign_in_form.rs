//! One SSH question's form, shared by the attention popover and Add host.

use std::rc::Rc;

use farhelm_proto::ui_channel::{ApiPath, Method, SshQuestion};
use gpui::{
    App, Context, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    Role, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, px,
};
use serde_json::json;

use crate::components::{Button, HoverHelp, TextInput, TextInputEvent, TextInputState, Tier};
use crate::focus::Mark;
use crate::host_text::{self, HostSentence, HostText};
use crate::theme::{self, metrics};

/// Capture a possible focus handoff while the answer still owns its field. Run it only after the helm accepts
/// the reply. The collection can remove the form before that reply comes back.
pub(crate) type PrepareHandoff = Rc<dyn Fn(Mark, &Window, &App) -> Handoff>;

type Handoff = Box<dyn FnOnce(&mut App)>;

/// The form holds the secret only while this exact question is drawn. Its callback captures the reply id once.
pub(crate) struct SignInForm {
    question: SshQuestion,
    field: Option<TextInputState>,
    cancel: FocusHandle,
    show: bool,
    sending: bool,
    problem: Option<HostText>,
    prepare_handoff: PrepareHandoff,
    _input: Option<Subscription>,
}

impl SignInForm {
    pub(crate) fn new(
        question: SshQuestion,
        prepare_handoff: PrepareHandoff,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let field = (!question.notice).then(|| TextInputState::new_masked(window, cx));
        let input = field.as_ref().map(|field| {
            field.subscribe(window, cx, |this, event, window, cx| {
                if matches!(event, TextInputEvent::Submitted { held: false }) {
                    this.submit(false, window, cx);
                }
            })
        });
        Self {
            question,
            field,
            cancel: cx.focus_handle(),
            show: false,
            sending: false,
            problem: None,
            prepare_handoff,
            _input: input,
        }
    }

    /// A question starts in its answer field. A display-only notice starts on its one action.
    pub(crate) fn first_stop(&self, cx: &App) -> FocusHandle {
        self.field
            .as_ref()
            .map_or_else(|| self.cancel.clone(), |field| field.focus_handle(cx))
    }

    fn reveal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(field) = &self.field else {
            return;
        };
        self.show = !self.show;
        field.set_masked(!self.show, window, cx);
        cx.notify();
    }

    /// Use the exact reply id, even if the host asked something else before the click reached the helm.
    fn submit(&mut self, cancel: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.sending {
            return;
        }
        let Ok(path) = ApiPath::api(&["ssh-questions", &self.question.id]) else {
            self.problem = Some(HostText::from_host(
                "This SSH question could not be addressed.",
            ));
            cx.notify();
            return;
        };
        let mut carry_focus = false;
        let body = if cancel {
            json!({ "action": "cancel" })
        } else {
            let Some(field) = &self.field else {
                return;
            };
            let value = field.text(cx);
            if value.len() > 1023 || value.contains('\0') {
                self.problem = Some(HostText::from_host(
                    "Enter an SSH answer of at most 1023 bytes.",
                ));
                cx.notify();
                return;
            }
            carry_focus = field.focus_handle(cx).is_focused(window);
            json!({ "action": "send", "answer": value.as_ref() })
        };
        let mark = Mark::take(cx);
        let handoff = carry_focus.then(|| (self.prepare_handoff)(mark, window, cx));
        let weak = cx.weak_entity();
        let sent = crate::operations::send(
            Method::Post,
            path,
            Some(body),
            "SSH sign-in",
            cx,
            move |result, cx| {
                if result.is_ok()
                    && let Some(handoff) = handoff
                {
                    handoff(cx);
                }
                let _ = weak.update(cx, |this, cx| {
                    this.sending = false;
                    if let Some(field) = &this.field {
                        field.set_read_only(false, cx);
                    }
                    if let Err(problem) = result {
                        this.problem = Some(problem);
                    }
                    cx.notify();
                });
            },
        );
        if sent {
            self.sending = true;
            self.problem = None;
            if let Some(field) = &self.field {
                field.set_masked(true, window, cx);
                field.set_text("", window, cx);
                field.set_read_only(true, cx);
                self.show = false;
            }
            cx.notify();
        }
    }
}

impl Render for SignInForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let weak = cx.weak_entity();
        let cancel = Button::new(
            "ssh-sign-in-cancel",
            "Cancel",
            HoverHelp::tooltip("Cancel this SSH sign-in"),
            Role::Button,
        )
        .focus(&self.cancel)
        .disabled(self.sending)
        .on_click(move |_, window, cx| {
            let _ = weak.update(cx, |this, cx| this.submit(true, window, cx));
        });
        let field = self.field.as_ref().map(|field| {
            let weak = cx.weak_entity();
            let visibility = Button::new(
                "ssh-sign-in-visibility",
                if self.show { "Hide" } else { "Show" },
                HoverHelp::tooltip(if self.show {
                    "Hide the answer as you type"
                } else {
                    "Show the answer as you type"
                }),
                Role::Button,
            )
            .compact()
            .keeps_focus_on_press()
            .on_click(move |_, window, cx| {
                let _ = weak.update(cx, |this, cx| this.reveal(window, cx));
            });
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(div().flex_1().min_w_0().child(TextInput::new(
                    "ssh-sign-in-answer",
                    field,
                    "Answer",
                    HoverHelp::tooltip("Answer the host's question"),
                )))
                .child(visibility)
        });
        let send = self.field.as_ref().map(|_| {
            let weak = cx.weak_entity();
            Button::new(
                "ssh-sign-in-send",
                "Send",
                HoverHelp::tooltip("Send the answer and continue signing in"),
                Role::Button,
            )
            .tier(Tier::Primary)
            .disabled(self.sending)
            .keeps_focus_on_press()
            .on_click(move |_, window, cx| {
                let _ = weak.update(cx, |this, cx| this.submit(false, window, cx));
            })
        });
        div()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .id("ssh-sign-in-question")
                    .max_h(px(240.))
                    .overflow_y_scroll()
                    .p(px(8.))
                    .rounded(metrics::RADIUS_SM)
                    .bg(theme::color(palette.bg_1))
                    .children(
                        self.question
                            .prompt
                            .split('\n')
                            .enumerate()
                            .map(|(index, line)| {
                                HostSentence::new(format!("ssh-sign-in-question-{index}"))
                                    .mono(&HostText::from_host(line))
                            }),
                    ),
            )
            .children(field)
            .children(self.problem.as_ref().map(|problem| {
                host_text::element(problem).text_color(theme::color(palette.danger_strong))
            }))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(cancel)
                    .children(send),
            )
    }
}
