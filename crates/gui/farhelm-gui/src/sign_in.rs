//! Present the helm's open SSH questions through the surface that is already drawn.
//!
//! One owner per window decides whether a question can open on its own. Buttons supply their drawn anchors, and the
//! owner keeps input marks and a single open layer. A question removed by the helm cannot keep an editable field.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use farhelm_helm_client::Collections;
use farhelm_proto::provisioning::{ProvisioningView, RunStatus};
use farhelm_proto::ui_channel::{HostStateView, HostView, SshConnectionPurpose, SshQuestion};
use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, FontWeight, Global,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, RenderOnce, Role,
    Styled as _, WeakEntity, Window, WindowId, div, px,
};

use crate::components::{Button, HoverHelp, Tier};
use crate::focus::{self, Mark};
use crate::host_text::{HostSentence, HostText};
use crate::hosts::setup_sign_in::Waiting;
use crate::layers::{self, Anchor, LayerId, Popover};
use crate::sign_in_form::SignInForm;
use crate::{session, theme};

#[derive(Default)]
struct Windows(HashMap<WindowId, WeakEntity<SignIn>>);

impl Global for Windows {}

/// The same sign-in status is shown by the sidebar, rail, Hosts and the unavailable session card. The question
/// collection takes precedence over the phase because an ownership refusal can remain sticky across a retry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    Waiting,
    Needed,
}

impl Status {
    pub(crate) fn words(self) -> &'static str {
        match self {
            Self::Waiting => "Waiting for sign-in",
            Self::Needed => "Sign-in needed",
        }
    }
}

pub(crate) fn status(host: &HostView, cx: &App) -> Option<Status> {
    if session::current_store(cx)
        .is_some_and(|store| pending_on(store.read(cx).collections(), host.id).is_some())
    {
        Some(Status::Waiting)
    } else if host.state == HostStateView::SignInNeeded {
        Some(Status::Needed)
    } else {
        None
    }
}

fn question(host: i64, cx: &App) -> Option<SshQuestion> {
    let store = session::current_store(cx)?;
    pending_on(store.read(cx).collections(), host).cloned()
}

/// The first question for a registered host, even if its connection phase still records an earlier refusal.
pub(crate) fn pending_on(collections: &Collections, host: i64) -> Option<&SshQuestion> {
    collections
        .ordered::<SshQuestion>()
        .into_iter()
        .filter(|question| question.target.host() == Some(host))
        .min_by_key(|question| {
            (
                question.purpose != SshConnectionPurpose::Setup,
                question.expires_at,
            )
        })
}

/// Why the window may focus a question without a press on its own control.
enum Intent {
    /// A first dial started with the helm can ask before the window has received user input.
    Startup,
    /// A user-started connection attempt on this host.
    Host(i64),
    /// An Answer button can scroll its exact question into view before opening the popover.
    Question(String),
    /// A Send or security-key touch may lead to a further question within the same sign-in.
    Followup(Followup),
}

/// A handoff can follow the SSH process that was already asking. When sending from the answer field it can
/// also choose a question already waiting at the time of the Send. A later reconnect is a new attempt.
struct Followup {
    previous: String,
    attempt: String,
    already_waiting: HashSet<String>,
}

/// A confirmed Add can start both masters independently. Each gets one automatic opening. An accepted answer
/// carries the mark to the other master even if its question only arrives after the Send.
struct RunIntent {
    id: String,
    mark: Mark,
    setup: bool,
    sessions: bool,
    handoff: Option<String>,
}

impl RunIntent {
    /// The two purposes are tracked independently because the order follows the network.
    fn offered(&mut self, purpose: SshConnectionPurpose) -> &mut bool {
        match purpose {
            SshConnectionPurpose::Setup => &mut self.setup,
            SshConnectionPurpose::Sessions => &mut self.sessions,
        }
    }
}

/// A button's location and focus stop describe the same painted control. An offscreen stop can be revealed
/// through the focus coordinator's recorded scroll regions before its anchor is used.
#[derive(Clone)]
struct ActionAnchor {
    anchor: Anchor,
    focus: FocusHandle,
    visible: Rc<Cell<bool>>,
}

impl Followup {
    fn allows(&self, next: &SshQuestion, cx: &App) -> bool {
        (self.attempt == next.attempt || self.already_waiting.contains(&next.id))
            && !question_exists(&self.previous, cx)
    }
}

pub(crate) struct SignIn {
    intent: Option<(Intent, Mark)>,
    open: Option<(String, LayerId)>,
    setup: HashMap<String, Entity<Waiting>>,
    runs: session::PerSession<HashMap<i64, RunIntent>>,
    _questions: session::ObservingCurrent,
}

impl SignIn {
    /// Take the startup mark before the window accepts input, including while the helm's initial snapshot loads.
    pub(crate) fn attach(window: &mut Window, cx: &mut App) -> Entity<Self> {
        let window_id = window.window_handle().window_id();
        let handle = window.window_handle();
        let owner = cx.new(|cx| Self {
            intent: Some((Intent::Startup, Mark::at_ui_session_start(cx))),
            open: None,
            setup: HashMap::new(),
            runs: session::PerSession::default(),
            _questions: session::observe_current(cx, move |owner: &mut Self, cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    owner.close_answered(window, cx);
                    owner.clear_lost_intent(cx);
                    owner.retire_runs(cx);
                    owner.setup.retain(|_, wait| wait.read(cx).active());
                });
            }),
        });
        cx.default_global::<Windows>()
            .0
            .retain(|_, owner| owner.upgrade().is_some());
        cx.global_mut::<Windows>()
            .0
            .insert(window_id, owner.downgrade());
        owner
    }

    /// Remove the popover as soon as its exact question goes. Closing through the layer manager restores the prior
    /// focus only when the layer still held it, including when the helper's security-key notice disappears.
    fn close_answered(&mut self, window: &mut Window, cx: &mut App) {
        let Some((id, layer)) = &self.open else {
            return;
        };
        if !layers::is_open(*layer, window, cx) {
            self.open = None;
        } else if !question_exists(id, cx) {
            let layer = *layer;
            self.open = None;
            layers::close(layer, window, cx);
        }
    }

    /// A retry that settled without a prompt must not grant permission to a later reconnect on the same host.
    fn clear_lost_intent(&mut self, cx: &App) {
        let Some((intent, mark)) = &self.intent else {
            return;
        };
        let finished = if let Intent::Host(host) = intent {
            question(*host, cx).is_none()
                && session::current_store(cx).is_some_and(|store| {
                    let collections = store.read(cx).collections();
                    collections
                        .get::<HostView>(&HostView::key(*host))
                        .is_none_or(|host| {
                            !matches!(
                                host.state,
                                HostStateView::SignInNeeded | HostStateView::Connecting { .. }
                            )
                        })
                })
        } else if let Intent::Question(id) = intent {
            !question_exists(id, cx)
        } else {
            false
        };
        if focus::has_moved_on(mark, cx) || finished {
            self.intent = None;
        }
    }

    /// The run only owns the setup master while it is running. A newly added host's sessions can connect after
    /// the run ends. Once that connection settles, a later reconnect needs an independent focus intent.
    fn retire_runs(&mut self, cx: &App) {
        let runs = self.runs.current(cx);
        let Some(store) = session::current_store(cx) else {
            runs.clear();
            return;
        };
        let collections = store.read(cx).collections();
        runs.retain(|host, intent| {
            let key = HostView::key(*host);
            if let Some(run) = collections.get::<ProvisioningView>(&key) {
                intent.setup &=
                    run.run_id.as_ref() == Some(&intent.id) && run.status == RunStatus::Running;
            }
            intent.sessions &= collections.get::<HostView>(&key).is_some_and(|host| {
                !matches!(
                    host.state,
                    HostStateView::Connected { .. } | HostStateView::SignInNeeded
                )
            });
            intent.setup || intent.sessions
        });
    }

    /// A deferred request from a drawn control. All automatic opens pass the same input and layer checks.
    fn consider(
        &mut self,
        question: SshQuestion,
        name: HostText,
        location: ActionAnchor,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.close_answered(window, cx);
        if self.open.is_some() || layers::focus_layer_open(window, cx) {
            return;
        }
        let setup_mark = self
            .setup
            .get(&question.attempt)
            .map(|wait| wait.read(cx).mark());
        let run_mark = question.target.host().and_then(|host| {
            let run = self.runs.current(cx).get_mut(&host)?;
            (*run.offered(question.purpose)).then_some(run.mark)
        });
        let mark = setup_mark
            .into_iter()
            .chain(run_mark)
            .chain(self.intent.as_ref().and_then(|(intent, mark)| {
                let eligible = match intent {
                    Intent::Startup => question.startup,
                    Intent::Host(host) => {
                        question.target.host() == Some(*host)
                            && question.purpose == SshConnectionPurpose::Sessions
                    }
                    Intent::Question(id) => question.id == *id,
                    Intent::Followup(followup) => followup.allows(&question, cx),
                };
                eligible.then_some(*mark)
            }))
            .find(|mark| !focus::has_moved_on(mark, cx));
        let Some(mark) = mark else {
            return;
        };
        if location.visible.get() {
            self.show(question, name, location.anchor, mark, window, cx);
        } else {
            // The attention and Hosts lists lay out their clipped rows too. Give their scroller a frame to reveal
            // the actual control before opening a layer beside it.
            focus::reveal_control(&location.focus, window, cx);
        }
    }

    /// An explicit Answer or an allowed focus handoff both converge on this one popover.
    fn show(
        &mut self,
        question: SshQuestion,
        name: HostText,
        anchor: Anchor,
        mark: Mark,
        window: &mut Window,
        cx: &mut App,
    ) {
        if !question_exists(&question.id, cx) {
            return;
        }
        if let Some((id, layer)) = &self.open {
            if *id == question.id && layers::is_open(*layer, window, cx) {
                return;
            }
            layers::close(*layer, window, cx);
        }

        let id = question.id.clone();
        let attempt = question.attempt.clone();
        let host = question.target.host();
        let notice = question.notice;
        let purpose = question.purpose;
        let container = cx.focus_handle();
        let sent_id = id.clone();
        let sent_attempt = attempt.clone();
        let waiting = self.setup.get(&attempt).map(Entity::downgrade);
        let form = cx.new(|cx| {
            SignInForm::new(
                question,
                Rc::new(move |mark, window, cx| {
                    let already_waiting = session::current_store(cx)
                        .map(|store| {
                            store
                                .read(cx)
                                .collections()
                                .ordered::<SshQuestion>()
                                .into_iter()
                                .filter(|question| question.id != sent_id)
                                .map(|question| question.id.clone())
                                .collect()
                        })
                        .unwrap_or_default();
                    let followup = Followup {
                        previous: sent_id.clone(),
                        attempt: sent_attempt.clone(),
                        already_waiting,
                    };
                    let window = window.window_handle();
                    let ready = waiting
                        .as_ref()
                        .and_then(WeakEntity::upgrade)
                        .map(|wait| Waiting::answer_handoff(&wait, mark, cx));
                    Box::new(move |accepted, cx| {
                        if let Some(ready) = ready {
                            ready(accepted, cx);
                        }
                        if !accepted {
                            return;
                        }
                        let _ = window.update(cx, |_, window, cx| {
                            if !focus::has_moved_on(&mark, cx)
                                && let Some(owner) = owner(window, cx)
                            {
                                let _ = owner.update(cx, |owner, cx| {
                                    if let Some(run) =
                                        host.and_then(|host| owner.runs.current(cx).get_mut(&host))
                                        && run.handoff.as_ref() == Some(&followup.attempt)
                                    {
                                        run.mark = mark;
                                    }
                                    owner.intent = Some((Intent::Followup(followup), mark));
                                });
                            }
                            window.refresh();
                        });
                    })
                }),
                window,
                cx,
            )
        });
        let start = form.read(cx).first_stop(cx);
        let view = cx.new(|_| QuestionPopover {
            name,
            purpose,
            form,
        });
        self.open = layers::open_popover(
            Popover::menu(view, container, anchor, start),
            &mark,
            window,
            cx,
        )
        .map(|layer| (id.clone(), layer));
        if self.open.is_some() {
            if let Some(run) = host.and_then(|host| self.runs.current(cx).get_mut(&host))
                && std::mem::replace(run.offered(purpose), false)
            {
                run.handoff = Some(attempt.clone());
            }
            // Touching a security key is outside the app. Carry this mark to that attempt's next question even though
            // no Send button was pressed. A new attempt on this host still needs fresh input to open.
            self.intent = notice.then_some((
                Intent::Followup(Followup {
                    previous: id,
                    attempt,
                    already_waiting: HashSet::new(),
                }),
                mark,
            ));
        }
    }
}

fn question_exists(id: &str, cx: &App) -> bool {
    session::current_store(cx).is_some_and(|store| {
        store
            .read(cx)
            .collections()
            .get::<SshQuestion>(id)
            .is_some()
    })
}

fn owner(window: &Window, cx: &App) -> Option<WeakEntity<SignIn>> {
    cx.try_global::<Windows>()?
        .0
        .get(&window.window_handle().window_id())
        .cloned()
}

fn intend(intent: Intent, mark: Mark, window: &Window, cx: &mut App) {
    if !focus::has_moved_on(&mark, cx)
        && let Some(owner) = owner(window, cx)
    {
        let _ = owner.update(cx, |owner, _| owner.intent = Some((intent, mark)));
    }
}

/// Remember which host an explicit operation started. Its later question still obeys the moved-on rule.
pub(crate) fn started(host: i64, mark: Mark, window: &Window, cx: &mut App) {
    intend(Intent::Host(host), mark, window, cx);
}

/// Retain the operation hold for this window only. An unrelated setup question cannot use its input mark.
pub(crate) fn watch_setup(
    attempt: String,
    waiting: Entity<Waiting>,
    window: &mut Window,
    cx: &mut App,
) {
    if let Some(owner) = owner(window, cx) {
        let _ = owner.update(cx, |owner, _| {
            owner.setup.insert(attempt, waiting);
        });
        window.refresh();
    }
}

/// Confirmation granted permission for this particular run. An Add also gets a second, sessions sign-in.
pub(crate) fn started_run(
    host: i64,
    id: String,
    adding: bool,
    mark: Mark,
    window: &mut Window,
    cx: &mut App,
) {
    if !focus::has_moved_on(&mark, cx)
        && let Some(owner) = owner(window, cx)
    {
        let _ = owner.update(cx, |owner, cx| {
            owner.runs.current(cx).insert(
                host,
                RunIntent {
                    id,
                    mark,
                    setup: true,
                    sessions: adding,
                    handoff: None,
                },
            );
            owner.retire_runs(cx);
        });
    }
    window.refresh();
}

/// The same action opens the pending question from any of the three surfaces, or asks the host to retry. Only the
/// attention row calls it Answer. The other surfaces name the action Sign in in both states.
pub(crate) fn action(host: &HostView, state: Status, id: String, attention: bool) -> Action {
    Action {
        host: host.id,
        name: crate::hosts::identity::display_name(host),
        id,
        label: if attention && state == Status::Waiting {
            "Answer"
        } else {
            "Sign in"
        },
    }
}

#[derive(IntoElement)]
pub(crate) struct Action {
    host: i64,
    name: HostText,
    id: String,
    label: &'static str,
}

impl RenderOnce for Action {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let owner = owner(window, cx);
        let location = ActionAnchor {
            anchor: Anchor::for_element(format!("{}-anchor", self.id), window, cx).beside(),
            focus: focus::control_handle(format!("{}-focus", self.id), window, cx),
            visible: Rc::new(Cell::new(false)),
        };
        let id_host = self.host;
        let name = self.name;
        if let (Some(owner), Some(question)) = (owner.clone(), question(id_host, cx)) {
            let location = location.clone();
            let name = name.clone();
            window.defer(cx, move |window, cx| {
                let _ = owner.update(cx, |owner, cx| {
                    owner.consider(question, name, location, window, cx)
                });
            });
        }
        let track = location.anchor.track_visibility(location.visible.clone());
        let control = Button::new(
            self.id,
            self.label,
            HoverHelp::tooltip("Sign in to this SSH host"),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .compact()
        .focus(&location.focus)
        .on_click(move |_, window, cx| {
            let mark = Mark::take(cx);
            if let Some(question) = question(id_host, cx) {
                if !location.visible.get() {
                    intend(Intent::Question(question.id), mark, window, cx);
                    focus::reveal_control(&location.focus, window, cx);
                } else if let Some(owner) = &owner {
                    let _ = owner.update(cx, |owner, cx| {
                        owner.show(
                            question,
                            name.clone(),
                            location.anchor.clone(),
                            mark,
                            window,
                            cx,
                        )
                    });
                }
            } else if session::current_store(cx).is_some_and(|store| {
                store
                    .read(cx)
                    .collections()
                    .get::<HostView>(&HostView::key(id_host))
                    .is_some_and(|host| host.state == HostStateView::SignInNeeded)
            }) && crate::hosts::remedies::retry(id_host, mark, window, cx)
            {
                started(id_host, mark, window, cx);
            }
        });
        div().relative().flex_none().child(control).child(track)
    }
}

struct QuestionPopover {
    name: HostText,
    purpose: SshConnectionPurpose,
    form: Entity<SignInForm>,
}

impl Render for QuestionPopover {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let purpose = match self.purpose {
            SshConnectionPurpose::Sessions => "For sessions",
            SshConnectionPurpose::Setup => "For setup and updates",
        };
        layers::surface(cx)
            .id("ssh-sign-in-popover")
            .w(px(420.))
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div().font_weight(FontWeight::SEMIBOLD).child(
                    HostSentence::new("ssh-sign-in-title")
                        .text("Sign in to ")
                        .host(&self.name),
                ),
            )
            .child(div().text_color(theme::color(palette.fg_1)).child(purpose))
            .child(self.form.clone())
    }
}

#[cfg(test)]
pub(crate) mod tests;
