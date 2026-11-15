//! Settings → Notifications, its ntfy alerts: whether the helm sends an alert when a session waits for the user, and the
//! ntfy server, topic, access token and priority it sends with.
//!
//! SPEC.md, "Session list" (the Notifications sentences) and "Status" (Ntfy alerts); SPEC_impl.md, "Ntfy waiting
//! alerts". Ported from the old app's `NtfySettings` (`ntfy_settings.rs`) with its words and its look from the
//! `.ntfy-settings` and `.checkout-root-*` rules in `app.css`, at reference commit `24bc0580`. Here's the TLDR of its
//! rules:
//!
//! - **Helm configuration, read once.** The settings live behind `/api/ntfy-alerts`, outside the preference row and
//!   not pushed, so the form reads them when it is made (the first time the section shows in an opening of the dialog,
//!   the pattern Agent trust follows), and a reply reaches only that form. Until the read answers, the form shows the
//!   helm's defaults, as the old one did, with every control unavailable and the fields read-only, so a slow read
//!   cannot turn into a save of the defaults; a failed read shows the helm's reason with "Retry loading".
//! - **One explicit Save of the whole form.** The route takes the whole configuration in one `PUT`, and its answer is
//!   what the helm stored (the server address normalized, whether a token is saved), which the form then shows, as the
//!   old one did. So the form is one [`Draft`] saved with [`drafts::save`], not a [`crate::drafts::DraftInput`] per
//!   field: `DraftInput::save` sends and settles only its own field, so one whole-form save cannot be built from three
//!   of them. That is a deliberate exception to the rule of one `DraftInput` per explicitly saved field, decided by the
//!   migration's settings-and-hosts area on 2026-11-14, until a second whole-form save makes it worth lifting into
//!   `drafts.rs`. The three text fields are bound to the draft here instead: an edit goes into the draft (refused, and
//!   the field put back, while the save is in flight), and the fields are read-only while it is. The token field is
//!   also read-only while Clear saved token is on, as the old one was.
//! - **The token is write-only.** The helm never returns it; the form shows whether one is saved (`token_set`) and
//!   offers to replace it (type one) or clear it ("Clear saved token"). A blank token field sends `null`, which the helm
//!   reads as "keep the saved token", but only on the same server: changing to another server clears it unless a new
//!   one is typed, and the form says so before saving ([`server_changed`]).
//! - **Saves are the operation store's work** under [`OpKey::preferences`], the preference row's key, although this is
//!   helm configuration, not a preference: helm configuration has no key of its own, so a Save here is refused while
//!   any other Settings save is in flight, and the other way round ([`super::agent_trust`] has the reasoning, accepted
//!   by this area on 2026-11-14). A Save the key refuses sends nothing and says so under the form ([`Refused`]) until
//!   the next save starts. A done save shows "ntfy alert settings saved" until the next edit and fills the form from
//!   the helm's answer, emptying the token field; a refusal keeps the form as typed with the helm's words under it,
//!   until the next save starts.
//! - **Send test is not a save.** It asks the helm to send one alert with the settings it stores, even while alerts are
//!   off, and sends nothing of the form (SPEC.md: "Send test uses the saved configuration"). It changes nothing, so it
//!   is no operation on a key: it goes out with [`crate::operations::send`] and its answer shows in place, "Test alert
//!   sent" or the helm's reason, until the next edit, save or test. A refused save's words stay above it until the next
//!   save, as a draft's refusal does, where the old form showed one outcome at a time. While it is out the form is
//!   locked as for a save, as the old one was, so a Save cannot overtake it. It moves no focus and opens nothing.
//!
//! NOTE: The Sounds sub-heading above ntfy alerts, with its three switches, is not this form's: they are preference
//! switches like the other sections', which the dialog draws above this form (`SettingsDialog::sounds`).

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::ntfy::{Priority, SaveSettings, Settings};
use gpui::{
    AnyWindowHandle, App, Context, Div, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Role, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, div, px,
};

use super::Refused;
use super::choices::{self, LABEL_LINE_HEIGHT, LABEL_TEXT};
use crate::components::{
    Button, Checkbox, HoverHelp, RadioGroup, RadioOption, TextInput, TextInputEvent,
    TextInputState, Tier,
};
use crate::drafts::{self, Draft};
use crate::host_text::HostText;
use crate::operations::{Command, OpKey, Outcome, Work};
use crate::theme;

/// `.host-settings-section`'s gap between the form's rows.
const ROW_GAP: f32 = 14.;
/// The form's name for [`Refused`]: its Save is the one control here whose press the key can refuse, and the line saying
/// so is drawn under the id [`Refused::line`] makes of it, `ntfy-busy`.
const REFUSED: &str = "ntfy";
/// The route the form reads and saves, as a refusal without words names it.
const ROUTE: &str = "/api/ntfy-alerts";
/// The priority segments' hover help, the old buttons' `data-tooltip`, which the group repeats as its own.
const PRIORITY_HELP: &str = "Priority used for session alerts; Save applies the selected priority";
/// Every priority, in the order the old segments showed them.
const PRIORITIES: [Priority; 5] = [
    Priority::Min,
    Priority::Low,
    Priority::Default,
    Priority::High,
    Priority::Urgent,
];

/// What the form holds: the stored settings as edited, the token typed (empty: none typed) and whether Clear saved
/// token is on.
#[derive(Clone, PartialEq)]
struct Form {
    enabled: bool,
    server_url: String,
    topic: String,
    priority: Priority,
    token: String,
    clear: bool,
}

impl Form {
    /// The form over `stored`, with no token typed and nothing to clear.
    fn over(stored: &Settings) -> Form {
        Form {
            enabled: stored.enabled,
            server_url: stored.server_url.clone(),
            topic: stored.topic.clone(),
            priority: stored.priority,
            token: String::new(),
            clear: false,
        }
    }

    /// The save the helm takes: an explicit clear sends `""`, a typed token replaces the saved one, and a blank field
    /// sends none, which the helm applies its same-server rule to (the module docs).
    fn save(&self) -> SaveSettings {
        let token = if self.clear {
            Some(String::new())
        } else {
            Some(self.token.clone()).filter(|token| !token.is_empty())
        };
        SaveSettings {
            enabled: self.enabled,
            server_url: self.server_url.clone(),
            topic: self.topic.clone(),
            priority: self.priority,
            token,
        }
    }

    /// `PUT /api/ntfy-alerts` with [`Form::save`]. Turning it into JSON cannot fail (plain strings, a bool and an
    /// enum); if it ever did, the request would go without a body and the helm would refuse it with its own words.
    fn command(&self) -> Result<Command, PathError> {
        Ok(Command::new(
            Work::Save,
            Method::Put,
            path(&[])?,
            serde_json::to_value(self.save()).ok(),
            format!("PUT {ROUTE}"),
        ))
    }
}

/// The route the form reads and saves ([`ROUTE`]), or with `tail` a route under it (Send test's `test`). `ApiPath`
/// builds a path from all its segments at once, so the route's own segment is spelled here only.
fn path(tail: &[&str]) -> Result<ApiPath, PathError> {
    let segments: Vec<&str> = std::iter::once("ntfy-alerts")
        .chain(tail.iter().copied())
        .collect();
    ApiPath::api(&segments)
}

/// Whether `draft` names another server than `saved`, the way the helm decides whether a saved token goes with it:
/// scheme, host and effective port, default ports normalized. An address that does not parse yet (half typed) names no
/// server, so it promises nothing about the token; the helm decides for real when the form is saved.
fn server_changed(saved: &str, draft: &str) -> bool {
    let origin = |value: &str| {
        url::Url::parse(value)
            .ok()
            .filter(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
            .map(|url| url.origin())
    };
    match (origin(saved), origin(draft)) {
        (Some(saved), Some(draft)) => saved != draft,
        _ => false,
    }
}

/// The form's three text fields, each bound to its part of the [`Form`].
#[derive(Clone, Copy)]
enum Field {
    Server,
    Topic,
    Token,
}

impl Field {
    const ALL: [Field; 3] = [Field::Server, Field::Topic, Field::Token];

    /// The text input drawn for this field.
    fn input(self, alerts: &NtfyAlerts) -> &TextInputState {
        match self {
            Field::Server => &alerts.server,
            Field::Topic => &alerts.topic,
            Field::Token => &alerts.token,
        }
    }

    /// The part of `form` this field edits. Mutable, so one match serves both typing into the form and reading it
    /// ([`NtfyAlerts::fill`] reads through a copy).
    fn value(self, form: &mut Form) -> &mut String {
        match self {
            Field::Server => &mut form.server_url,
            Field::Topic => &mut form.topic,
            Field::Token => &mut form.token,
        }
    }
}

/// The ntfy alerts form, a view of its own that Settings makes the first time Notifications shows in an opening and
/// keeps until the dialog closes (see the module docs).
pub(super) struct NtfyAlerts {
    /// What the helm last said it stores: the read's answer, then each done save's. `None` until the read answers.
    stored: Option<Settings>,
    /// Why the last read failed, until a read answers.
    unreadable: Option<HostText>,
    form: Draft<Form>,
    server: TextInputState,
    topic: TextInputState,
    token: TextInputState,
    /// Whether the last save was done; its line shows until the next edit touches the form, or a test starts.
    saved: bool,
    /// Whether Send test is out; the form is locked meanwhile.
    testing: bool,
    /// How the last Send test went, until the next edit, save or test.
    tested: Option<Result<(), HostText>>,
    /// Whether the last Save sent nothing because another Settings save held the key, which the form says until its
    /// next save starts.
    refused: Refused,
    /// The window the form is drawn in, where answers that arrive later set the fields' text.
    window: AnyWindowHandle,
    _edits: Vec<Subscription>,
}

impl NtfyAlerts {
    /// The form, reading the helm's settings now.
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let server = TextInputState::new(window, cx);
        let topic = TextInputState::new(window, cx);
        let token = TextInputState::new_masked(window, cx);
        let mut alerts = NtfyAlerts {
            stored: None,
            unreadable: None,
            form: Draft::load(Form::over(&Settings::default())),
            server,
            topic,
            token,
            saved: false,
            testing: false,
            tested: None,
            refused: Refused::default(),
            window: window.window_handle(),
            _edits: Vec::new(),
        };
        alerts._edits = Field::ALL
            .iter()
            .map(|&field| {
                field
                    .input(&alerts)
                    .subscribe(window, cx, move |alerts, event, window, cx| {
                        if let TextInputEvent::Changed = event {
                            alerts.typed(field, window, cx);
                        }
                    })
            })
            .collect();
        // The fields show the loaded defaults, locked, until the read answers, as the old form's did.
        alerts.fill(window, cx);
        alerts.hold(cx);
        alerts.sync(window, cx);
        Self::read(alerts.window, cx);
        alerts
    }

    /// Read the helm's settings, the answer going to this form if it is still open, under [`choices::read_route`]'s
    /// contract (once, on its own, after the calling update has returned).
    fn read(window: AnyWindowHandle, cx: &mut Context<Self>) {
        let alerts = cx.weak_entity();
        let answered = move |answer: Result<Settings, HostText>, cx: &mut App| {
            let _ = window.update(cx, |_, window, cx| {
                let _ = alerts.update(cx, |alerts, cx| alerts.answered(answer, window, cx));
            });
        };
        choices::read_route(path(&[]), format!("GET {ROUTE}"), cx, answered);
    }

    /// A read, or a done save, told what the helm stores: an untouched form takes it (emptying the token field), and
    /// the controls become available; or the read's reason shows.
    fn answered(
        &mut self,
        answer: Result<Settings, HostText>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match answer {
            Ok(stored) => {
                self.unreadable = None;
                if self.form.follow(Form::over(&stored)) {
                    self.fill(window, cx);
                }
                self.stored = Some(stored);
                self.hold(cx);
            }
            Err(reason) => self.unreadable = Some(reason),
        }
        self.sync(window, cx);
        cx.notify();
    }

    /// Set the three text fields to the draft's values. Setting a field raises no change event, so this is not an edit.
    fn fill(&self, window: &mut Window, cx: &mut App) {
        let mut form = self.form.value().clone();
        for field in Field::ALL {
            field
                .input(self)
                .set_text(field.value(&mut form).clone(), window, cx);
        }
    }

    /// The topic field, for tests that must reach past its read-only hold to show what the form does with an edit
    /// that arrives anyway.
    #[cfg(test)]
    pub(super) fn topic_input(&self) -> TextInputState {
        self.topic.clone()
    }

    /// Whether the controls take no input: before the read has answered, and while a save or Send test is out.
    fn locked(&self) -> bool {
        self.stored.is_none() || self.form.is_saving() || self.testing
    }

    /// Hold the text fields read-only while the form is locked, and the token field also while Clear saved token is
    /// on; free them otherwise. Called whenever either changes.
    fn hold(&self, cx: &mut App) {
        for field in Field::ALL {
            let held = match field {
                Field::Server | Field::Topic => self.locked(),
                Field::Token => self.locked() || self.form.value().clear,
            };
            field.input(self).set_read_only(held, cx);
        }
    }

    /// Change the form as `change` says, unless it is locked. Returns whether the draft took it.
    fn edit(&mut self, change: impl FnOnce(&mut Form), cx: &mut Context<Self>) -> bool {
        if self.locked() {
            return false;
        }
        let mut form = self.form.value().clone();
        change(&mut form);
        let taken = self.form.edit(form);
        if taken {
            self.tested = None;
        }
        cx.notify();
        taken
    }

    /// The user typed into `field`: the draft takes the text, or, refused, the field shows the draft's again.
    fn typed(&mut self, field: Field, window: &mut Window, cx: &mut Context<Self>) {
        let text = field.input(self).text(cx).to_string();
        if !self.edit(|form| *field.value(form) = text, cx) {
            let mut form = self.form.value().clone();
            field
                .input(self)
                .set_text(field.value(&mut form).clone(), window, cx);
        }
        self.sync(window, cx);
    }

    /// The token field's placeholder, which says what Save does with a blank field: the old field's words.
    fn sync(&self, window: &mut Window, cx: &mut App) {
        let placeholder = if self.token_will_clear() {
            "Saved token will be cleared"
        } else if self.token_saved() {
            "Token saved; leave blank to keep it"
        } else {
            "No token saved"
        };
        self.token.set_placeholder(placeholder, window, cx);
    }

    fn token_saved(&self) -> bool {
        self.stored.as_ref().is_some_and(|stored| stored.token_set)
    }

    /// Whether the form names another server than the one the helm stores.
    fn server_changed(&self) -> bool {
        self.stored
            .as_ref()
            .is_some_and(|stored| server_changed(&stored.server_url, &self.form.value().server_url))
    }

    /// Whether Save would clear the saved token, as far as the form can tell: asked to, or a blank field on another
    /// server.
    fn token_will_clear(&self) -> bool {
        let form = self.form.value();
        self.token_saved() && (form.clear || (self.server_changed() && form.token.is_empty()))
    }

    /// Save the form. Does nothing while it is locked, when Save is drawn busy or unavailable; a Save the operation store
    /// refuses because another Settings save holds the key says so under the form ([`Refused`]).
    fn save(&mut self, cx: &mut Context<Self>) {
        if self.locked() {
            return;
        }
        let window = self.window;
        let started = drafts::save(
            self,
            |alerts: &mut NtfyAlerts| &mut alerts.form,
            &OpKey::preferences(),
            Form::command,
            cx,
            move |alerts, settled, cx| {
                alerts.hold(cx);
                let Outcome::Done(body) = &settled.outcome else {
                    return;
                };
                // The answer is what the helm stored, which the form shows as the old one did. The fields need the
                // window, which this completion does not have, so that waits until it has returned. An answer that
                // does not read as the settings (never sent by the helm) is read again instead, with no saved line,
                // so the form goes on to show what the helm stores.
                let Some(Ok(stored)) = body.clone().map(serde_json::from_value::<Settings>) else {
                    Self::read(window, cx);
                    return;
                };
                alerts.saved = true;
                let alerts = cx.weak_entity();
                cx.defer(move |cx| {
                    let _ = window.update(cx, |_, window, cx| {
                        let _ =
                            alerts.update(cx, |alerts, cx| alerts.answered(Ok(stored), window, cx));
                    });
                });
            },
        );
        // Another Settings save may hold the key, which the form then says.
        self.refused.record(REFUSED, &started);
        match started {
            Ok(()) => {
                self.saved = false;
                self.tested = None;
                self.hold(cx);
            }
            Err(not_started) => tracing::debug!(?not_started, "ntfy alert settings not saved"),
        }
        cx.notify();
    }

    /// Send test: ask the helm to send one alert with the settings it stores (the module docs). Does nothing while the
    /// form is locked, or when there is no UI session to send through.
    fn send_test(&mut self, cx: &mut Context<Self>) {
        if self.locked() {
            return;
        }
        let path = match path(&["test"]) {
            Ok(path) => path,
            Err(error) => {
                tracing::debug!(?error, "ntfy test path not built");
                return;
            }
        };
        let shown = format!("POST {}", path.as_str());
        let alerts = cx.weak_entity();
        let sent =
            crate::operations::send(Method::Post, path, None, shown, cx, move |answer, cx| {
                let _ = alerts.update(cx, |alerts, cx| {
                    alerts.testing = false;
                    alerts.tested = Some(answer.map(|_| ()));
                    alerts.hold(cx);
                    cx.notify();
                });
            });
        if sent {
            self.testing = true;
            self.tested = None;
            self.saved = false;
            self.hold(cx);
            cx.notify();
        }
    }

    /// One field with its words above it, as wide as the section (`.checkout-root-label`).
    fn field(
        &self,
        field: Field,
        id: &'static str,
        label: &'static str,
        help: &'static str,
    ) -> Div {
        let input = TextInput::new(id, field.input(self), label, HoverHelp::tooltip(help))
            .height(px(choices::FIELD_HEIGHT));
        let input = match field {
            Field::Server => input.monospace(),
            Field::Topic | Field::Token => input,
        };
        choices::labelled(label, id, input)
    }

    /// A field's or the switches' words, at the old labels' size.
    fn label(text: &'static str) -> Div {
        div()
            .text_size(px(LABEL_TEXT))
            .line_height(px(LABEL_LINE_HEIGHT))
            .child(text)
    }

    /// A switch with its words, toggling through this form.
    fn switch(
        &self,
        id: &'static str,
        label: &'static str,
        help: &'static str,
        on: bool,
        change: fn(&mut Form, bool),
        cx: &Context<Self>,
    ) -> Div {
        let weak = cx.weak_entity();
        div()
            .text_size(px(LABEL_TEXT))
            .line_height(px(LABEL_LINE_HEIGHT))
            .child(
                Checkbox::new(id, label, HoverHelp::tooltip(help), Role::Switch)
                    .switch()
                    .checked(on)
                    .disabled(self.locked())
                    .on_toggle(move |on, window, cx| {
                        let _ = weak.update(cx, |alerts, cx| {
                            let _ = alerts.edit(|form| change(form, on), cx);
                            alerts.hold(cx);
                            alerts.sync(window, cx);
                        });
                    }),
            )
    }
}

impl Render for NtfyAlerts {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let form = self.form.value().clone();
        let weak = cx.weak_entity();
        let failure =
            |id: SharedString, reason: &HostText| choices::failure_outcome(id, reason, cx);

        let token_help = if form.clear {
            "The saved access token will be cleared when you save"
        } else if self.server_changed() && self.token_saved() {
            "Optional token for this server; leaving it blank clears the previous server's saved token"
        } else {
            "Optional server access token; leave blank to preserve the saved token"
        };
        let origin_hint =
            (self.token_saved() && self.server_changed() && form.token.is_empty() && !form.clear)
                .then(|| {
                    choices::help(
                        "Changing the server clears the saved token when you save",
                        cx,
                    )
                    .id("ntfy-token-origin-hint")
                    .debug_selector(|| "ntfy-token-origin-hint".to_owned())
                });

        // `.segmented-control` of `aria-pressed` buttons, as the old form drew the five priorities: each its own tab
        // stop, a press choosing it into the form and saving nothing.
        let choose = weak.clone();
        let priority = div()
            .flex()
            .flex_col()
            .gap(px(choices::LABEL_GAP))
            .w_full()
            .min_w_0()
            .child(Self::label("Priority"))
            .child(
                PRIORITIES
                    .into_iter()
                    .fold(
                        RadioGroup::new(
                            "ntfy-priority",
                            "Priority",
                            HoverHelp::tooltip(PRIORITY_HELP),
                            Role::Group,
                        ),
                        |group, priority| {
                            group.option(RadioOption::new(
                                priority.display_label(),
                                HoverHelp::tooltip(PRIORITY_HELP),
                                priority,
                            ))
                        },
                    )
                    .chosen(Some(form.priority))
                    .disabled(self.locked())
                    .on_change(move |&chosen, _, cx| {
                        let _ = choose.update(cx, |alerts, cx| {
                            let _ = alerts.edit(|form| form.priority = chosen, cx);
                        });
                    }),
            );

        // "Retry loading" while the read has failed, as in the old form.
        let retry = self.unreadable.is_some().then(|| {
            let weak = weak.clone();
            Button::new(
                "ntfy-retry",
                "Retry loading",
                HoverHelp::tooltip("Load the saved ntfy alert settings again"),
                Role::Button,
            )
            .tier(Tier::Neutral)
            .on_click(move |_, _, cx| {
                let _ = weak.update(cx, |alerts, cx| {
                    alerts.unreadable = None;
                    Self::read(alerts.window, cx);
                    cx.notify();
                });
            })
        });
        let save = Button::new(
            "ntfy-save",
            "Save",
            HoverHelp::tooltip("Save ntfy alert settings for every window using this helm"),
            Role::Button,
        )
        .tier(Tier::Primary)
        .disabled(self.stored.is_none())
        // Busy, not disabled, while saving or testing, so a clicked Save keeps focus (SPEC_impl.md, "Drafts").
        .busy(self.form.is_saving() || self.testing)
        .on_click({
            let weak = weak.clone();
            move |_, _, cx| {
                let _ = weak.update(cx, |alerts, cx| alerts.save(cx));
            }
        });
        let send_test = Button::new(
            "ntfy-send-test",
            "Send test",
            HoverHelp::tooltip(
                "Send one alert using the saved settings, including while alerts are switched off",
            ),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .disabled(self.stored.is_none())
        .busy(self.form.is_saving() || self.testing)
        .on_click(move |_, _, cx| {
            let _ = weak.update(cx, |alerts, cx| alerts.send_test(cx));
        });

        let saving = self.form.is_saving();
        let tested = self.tested.as_ref().map(|tested| match tested {
            Ok(()) => {
                choices::outcome_line("ntfy-tested", Role::Status, theme::color(palette.fg_1))
                    .aria_label("Test alert sent")
                    .child("Test alert sent")
            }
            Err(reason) => failure("ntfy-tested".into(), reason),
        });
        let saved = (self.saved && !saving && !self.form.is_touched()).then(|| {
            choices::outcome_line("ntfy-saved", Role::Status, theme::color(palette.fg_1))
                .aria_label("ntfy alert settings saved")
                .child("ntfy alert settings saved")
        });

        div()
            .id("ntfy-alerts")
            .role(Role::Region)
            .aria_label("ntfy alerts")
            .flex()
            .flex_col()
            .gap(px(ROW_GAP))
            .w_full()
            .min_w_0()
            .child(
                div()
                    .id("ntfy-heading")
                    .role(Role::Heading)
                    .aria_label("ntfy alerts")
                    .text_size(px(LABEL_TEXT))
                    .line_height(px(LABEL_LINE_HEIGHT))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::color(palette.fg_0))
                    .child("ntfy alerts"),
            )
            .child(choices::help(
                "Send a session's title and host when it waits for you. Applies to every window using this helm. \
                 In the desktop app, keep the window open.",
                cx,
            ))
            .child(self.switch(
                "ntfy-enabled",
                "Send ntfy alerts",
                "Send alerts when a session waits for you; Save applies this to every window using this helm",
                form.enabled,
                |form, on| form.enabled = on,
                cx,
            ))
            .child(self.field(
                Field::Server,
                "ntfy-server",
                "Server URL",
                "ntfy server that receives session alerts",
            ))
            .child(self.field(
                Field::Topic,
                "ntfy-topic",
                "Topic",
                "Topic to publish alerts to; subscribe to the same topic in your ntfy client",
            ))
            .child(choices::help(
                "Anyone who knows an ntfy.sh topic can read its alerts. Choose a hard-to-guess topic.",
                cx,
            ))
            .child(self.field(Field::Token, "ntfy-token", "Access token (optional)", token_help))
            .children(origin_hint)
            .child(self.switch(
                "ntfy-clear-token",
                "Clear saved token",
                "Remove the saved access token when you Save",
                form.clear,
                |form, on| form.clear = on,
                cx,
            ))
            .child(priority)
            .children(self.unreadable.as_ref().map(|reason| failure("ntfy-load-error".into(), reason)))
            .children(retry)
            // `.dialog-actions`: the form's buttons at its end, Send test before Save. They share a row, not a
            // submission: Send test never sends the form.
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .w_full()
                    .child(send_test)
                    .child(save),
            )
            .child(choices::help(
                "Save before testing. Send test uses the saved settings, not this form's unsaved changes.",
                cx,
            ))
            .children((saving || self.testing).then(|| {
                choices::outcome_line("ntfy-working", Role::Status, theme::color(palette.fg_0))
                    .aria_label("Working…")
                    .child("Working…")
            }))
            .children(self.form.refusal().map(|reason| failure("ntfy-error".into(), reason)))
            .children(
                self.refused
                    .line(REFUSED, |id, text| failure(id.into(), &HostText::from_host(text))),
            )
            .children(saved)
            .children(tested)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unrelated saves must keep a token the form never received, and an explicit replacement or clear must not be
    /// taken for that keeping: the old form's `secret_updates_are_explicit`. Specified: a blank token field sends
    /// none, a typed one sends it, and Clear saved token sends `""` whatever was typed.
    #[test]
    fn a_blank_token_keeps_the_saved_one_and_a_typed_or_cleared_one_does_not() {
        let mut form = Form::over(&Settings::default());
        assert_eq!(form.save().token, None);
        form.token = "replacement".into();
        assert_eq!(form.save().token.as_deref(), Some("replacement"));
        form.clear = true;
        assert_eq!(form.save().token.as_deref(), Some(""));
    }

    /// The warning that Save clears the token must follow the helm's own rule, which compares servers, not spellings:
    /// equal addresses must not promise a removal, and a half-typed one has no server yet. The old form's
    /// `token_warning_compares_normalized_server_origins`. Specified: default ports, a path, a trailing slash and a
    /// capitalised host name are the same server; another scheme, host or port is another; an unfinished address is
    /// neither.
    #[test]
    fn the_server_changes_only_with_its_scheme_host_or_port() {
        for same in [
            "https://example.test",
            "https://example.test:443/prefix",
            "https://EXAMPLE.test/",
        ] {
            assert!(!server_changed("https://example.test", same), "{same}");
        }
        for other in [
            "http://example.test",
            "https://other.test",
            "https://example.test:8443",
        ] {
            assert!(server_changed("https://example.test", other), "{other}");
        }
        assert!(!server_changed("https://example.test", "https://"));
    }
}
