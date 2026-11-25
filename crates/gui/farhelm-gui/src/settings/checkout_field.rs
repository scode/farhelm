//! The explicitly saved checkout fields: a checkout folder (the shared field, [`CheckoutField::folder`]) and Settings'
//! all-hosts post-clone command ([`CheckoutField::post_clone`]).
//!
//! SPEC.md, "Managed checkouts" and "Session list" (Checkouts); SPEC_impl.md, "Checkout configuration and discovery".
//! Ported from the old app's `CheckoutRootField` (`checkout_root.rs`) and `CheckoutPostCloneField`
//! (`checkout_post_clone.rs`) at reference commit `24bc0580`, with their labels, notes, button words and outcome lines.
//! The two were one shape in the old app (a label, a field, a note, Save or Retry, and a saved or failure line, the
//! `.checkout-root-field` and `.checkout-post-clone-field` rules), so here they are one view whose differences are a
//! table of words per field ([`Words`]) and exhaustive methods on what it edits ([`Target`]). Here's the TLDR of
//! their rules:
//!
//! - **Helm configuration, read once.** Each field lives behind its own route, which the helm does not push, so the
//!   view reads it when it is made, and a reply reaches only that view (it holds the view weakly): the pattern the
//!   migration's orchestrator settled for reads that are not pushed. Until the read answers the field is read-only and
//!   Save does nothing, so a slow read can never turn the empty field into a clear (the old "stays locked until its
//!   initial read completes"). A failed read shows the helm's reason with Retry in place of Save.
//! - **Explicit Save, of an edited draft only.** The field is a [`DraftInput`]. Save and Use default carry the exact
//!   value last loaded, so the helm can refuse a save if the command line has since changed that field (SPEC.md,
//!   "One GUI at a time"). Both a done and a refused save lead to a fresh read. A refusal discards the edit and shows
//!   the helm's reason until the next save starts. The field is read-only through the save and the read that follows,
//!   so a second save always starts from a value the user has had a chance to see.
//! - **What Save sends.** A folder is sent trimmed, and an empty one as `null`, which clears that scope (a host then
//!   inherits the default). A command is sent verbatim, spacing and newlines included, and only an all-whitespace one
//!   as `null` (SPEC.md: "clears it when an all-whitespace field is saved"; "The command field preserves all other
//!   spacing and newlines"). A stored explicit disable (`""`) and an unset command both show as an empty field, and
//!   opening never writes either back. A host's folder also offers Use default, an explicit clear even when the draft
//!   is untouched, shown while the host has a folder of its own.
//! - **Saves are the operation store's work**, under the key of what they save: [`OpKey::preferences`] for the
//!   all-hosts configuration, as Settings' Agent trust switches do (see [`super::agent_trust`] for why helm
//!   configuration shares that key, and the module docs of [`super`] for what sharing it means for quick saves), and
//!   [`OpKey::host`] for a host's own folder. A save outlives the view; its outcome reaches the view only if it is
//!   still open. A Save or Use default pressed while another save holds the key sends nothing and says so under the
//!   field ([`Refused`]) until a save of this field starts.
//!
//! The deviations from the old fields (a refusal kept until the next save starts, the folder trimmed only in what is
//! sent, no refusal over a stored carriage return) are in `docs/gpui/parity/settings-and-hosts.md`, "Deviations".

use farhelm_helm_client::{ApiPath, Method, PathError};
use gpui::{
    AnyWindowHandle, App, Context, Div, ElementId, EventEmitter, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Role, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, div, px,
};

use super::Refused;
use super::choices;
use crate::components::{Button, HoverHelp, TextInput, TextInputState, Tier};
use crate::drafts::DraftInput;
use crate::focus::Mark;
use crate::host_text::{HostSentence, HostText};
use crate::operations::{Command, OpKey, Outcome, Work};
use crate::theme::{self, metrics};

/// `.checkout-root-field`'s gap between its label block, note, buttons and outcome line.
const FIELD_GAP: f32 = 8.;
/// The post-clone box's starting rows, the old `<textarea rows="4">`.
const COMMAND_ROWS: usize = 4;

/// The old post-clone field's note.
pub(super) const COMMAND_NOTE: &str = "Runs in each new managed checkout, on its host, after cloning and before the agent starts. \
                            Changes affect future checkouts only. A host given its own command from the command line \
                            keeps it. Save an empty field to remove the all-hosts command.";

/// Whose checkout folder a folder field edits: the route it reads and saves, and what its empty field means.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FolderScope {
    /// The all-hosts default folder (`/api/checkout-root`), used by hosts without their own. Its empty field means no
    /// default, and shows no placeholder.
    Default,
    /// One host's own folder (`/api/hosts/<id>/checkout-root`), by registry id. Its empty field means the host uses the
    /// default, which the field shows as its placeholder ("Default: <folder>", or "No default set"), and while the host
    /// has a folder of its own the field offers Use default to clear it.
    Host(String),
}

/// What a [`CheckoutField`] edits. Everything the view does differently per field is an exhaustive method here.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Target {
    Folder(FolderScope),
    PostClone,
}

/// What one kind of field says and is found by: everything per field that is a fixed string.
pub(super) struct Words {
    /// The prefix of every element id, which tests and the test bridge find the parts by: `<id>` is the field,
    /// `<id>-save`, `<id>-retry` and `<id>-use-default` its buttons, `<id>-error` and `<id>-saved` its lines.
    pub(super) id: &'static str,
    /// The words above the field, which are also its accessible name.
    pub(super) label: &'static str,
    /// The field's hover help. The old fields had none (the component layer requires one); these say what the value is
    /// for, in SPEC.md's terms.
    pub(super) help: &'static str,
    save_help: &'static str,
    retry_help: &'static str,
    /// The line after a done save.
    saved: &'static str,
    /// The route as a refusal without words names it, a host's id hidden (`operations::Command::new`).
    shown: &'static str,
}

pub(super) const DEFAULT_FOLDER: Words = Words {
    id: "checkout-folder",
    label: "Default checkout folder",
    help: "The folder new managed checkouts go in on hosts without their own folder",
    save_help: "Save the default checkout folder for hosts without their own",
    retry_help: "Retry loading the default checkout folder",
    saved: "Checkout folder saved",
    shown: "/api/checkout-root",
};

const HOST_FOLDER: Words = Words {
    id: "checkout-folder",
    label: "Checkout folder",
    help: "The folder new managed checkouts go in on this host; leave it empty to use the default",
    save_help: "Save the checkout folder for this host",
    retry_help: "Retry loading this host's checkout folder and the default",
    saved: "Checkout folder saved",
    shown: "/api/hosts/[host]/checkout-root",
};

pub(super) const POST_CLONE: Words = Words {
    id: "checkout-post-clone",
    label: "Post-clone command",
    help: "A command run in each new managed checkout after cloning and before the agent starts",
    save_help: "Save the post-clone command for future managed checkouts",
    retry_help: "Retry loading the all-hosts post-clone command",
    saved: "Post-clone command saved",
    shown: "/api/checkout-post-clone",
};

impl Target {
    fn words(&self) -> &'static Words {
        match self {
            Target::Folder(FolderScope::Default) => &DEFAULT_FOLDER,
            Target::Folder(FolderScope::Host(_)) => &HOST_FOLDER,
            Target::PostClone => &POST_CLONE,
        }
    }

    fn path(&self) -> Result<ApiPath, PathError> {
        match self {
            Target::Folder(FolderScope::Default) => ApiPath::api(&["checkout-root"]),
            Target::Folder(FolderScope::Host(id)) => ApiPath::api(&["hosts", id, "checkout-root"]),
            Target::PostClone => ApiPath::api(&["checkout-post-clone"]),
        }
    }

    /// The key a save is work on: the host's for its own folder, the preference row's for the all-hosts
    /// configuration, which has no key of its own (the module docs).
    fn key(&self) -> OpKey {
        match self {
            Target::Folder(FolderScope::Host(id)) => OpKey::host(id),
            Target::Folder(FolderScope::Default) | Target::PostClone => OpKey::preferences(),
        }
    }

    /// The scope's own value in a read's answer: a folder route's `root`, the post-clone route's `command`.
    fn value(&self, stored: &Stored) -> Option<String> {
        match self {
            Target::Folder(_) => stored.root.clone(),
            Target::PostClone => stored.command.clone(),
        }
    }

    /// Whether the field offers Use default while its scope holds a value of its own.
    fn offers_default(&self) -> bool {
        match self {
            Target::Folder(FolderScope::Host(_)) => true,
            Target::Folder(FolderScope::Default) | Target::PostClone => false,
        }
    }

    /// The empty field's placeholder once a read has answered, or `None` for none: a host's folder shows the default it
    /// inherits. The folder is the user's own configuration (typed here or with the command line on the helm's
    /// machine), not text a host wrote, and the field shows the same raw value when it is the one being edited.
    fn placeholder(&self, stored: &Stored) -> Option<String> {
        match self {
            Target::Folder(FolderScope::Host(_)) => Some(match &stored.default {
                Some(default) => format!("Default: {default}"),
                None => "No default set".to_owned(),
            }),
            Target::Folder(FolderScope::Default) | Target::PostClone => None,
        }
    }

    /// The text input this field draws in: one monospace line for a folder (`.checkout-root-field input`, in the old
    /// stylesheet's monospace list), a monospace box growing from four rows for the command, where Enter types a line
    /// break.
    fn input(&self, window: &mut Window, cx: &mut App) -> TextInputState {
        match self {
            Target::Folder(_) => TextInputState::new(window, cx),
            Target::PostClone => TextInputState::new_growing(COMMAND_ROWS, window, cx),
        }
    }

    /// Normalize the proposed value as usual, and send the raw base unchanged. An omitted comparison is how old
    /// helms accept the request; a present null is a comparison against an unset value.
    fn command(
        &self,
        value: Option<&str>,
        expected: Option<Option<&str>>,
    ) -> Result<Command, PathError> {
        let mut body = match self {
            Target::Folder(_) => {
                let root = value.map(str::trim).filter(|root| !root.is_empty());
                serde_json::json!({ "root": root })
            }
            Target::PostClone => {
                let command = value.filter(|command| !command.trim().is_empty());
                serde_json::json!({ "command": command })
            }
        };
        if let Some(expected) = expected {
            body["expected"] = serde_json::json!(expected);
        }
        Ok(Command::new(
            Work::Save,
            Method::Put,
            self.path()?,
            Some(body),
            format!("PUT {}", self.words().shown),
        ))
    }
}

/// A checkout route's answer: `root` on a folder route (with the inherited `default` on a host's), `command` on the
/// post-clone route (`""` explicitly disabled, `null` unset). Each route sends only its own fields; serde reads a field
/// a route does not send as `None`.
#[derive(serde::Deserialize)]
struct Stored {
    root: Option<String>,
    command: Option<String>,
    default: Option<String>,
    /// An older helm rejects unknown keys in a PUT, so only send the base when the read advertises it.
    #[serde(default)]
    compare_on_save: bool,
}

/// Refresh dependent views when a save succeeds or a refusal's refetch finds a different stored value. The launcher's
/// inline field can learn that the CLI configured a folder, so its old suggestions and preview need to be checked.
pub struct CheckoutUpdated {
    /// A successful save can move focus only if the user has stayed with it since pressing Save.
    /// A refetch can discover someone else's change and carries no focus request.
    pub(crate) saved_at: Option<Mark>,
}

/// One explicitly saved checkout field, as a view of its own: its label, field, note, Save (or Retry) and outcome line.
///
/// The folder field ([`CheckoutField::folder`]) is shared. Settings → Checkouts draws it over the default folder; the
/// host settings dialog draws it over a host's own folder ([`FolderScope::Host`], the default as placeholder and Use
/// default to clear the override, SPEC.md "Topology"); and the launcher's managed-checkout destination is to draw
/// it over the default folder when the chosen host has no folder in effect (SPEC.md, "Managed checkouts"), subscribing
/// to [`CheckoutUpdated`] to refresh its suggestions and preview. Each owner makes the view when its dialog or section first
/// shows (which reads the route then), keeps the entity for as long as the dialog is open, and draws it as a child: a
/// new view for each opening, so a reply reaches only the opening it was sent for. Enter in the field saves nothing, as
/// in the old field, where saving the folder and submitting a surrounding form were separate acts.
///
/// What a reusing owner must know:
/// - **It must hold a UI session.** The read goes through [`choices::read_route`], which sends nothing without one,
///   and then the field stays read-only with no line under it. Settings, the host dialog and the launcher all open only
///   over a session, and a reset closes them.
/// - **The look is Settings'.** The old launcher drew this field in its form's two columns (a 104-pixel label column,
///   `.create-session-form .checkout-root-field`) with its error in the warning color (`--warn-bright`) rather than
///   the danger color; the launcher adds those as builders here when it reuses the field.
/// - **No outside gate.** The old host dialog also locked the field while the rest of the dialog was busy (its
///   `disabled` prop). The host dialog does not port that lock (its module docs say why: saves under the host's key
///   already refuse each other), so the field has no builder for it. An owner that does need one adds it here.
pub struct CheckoutField {
    target: Target,
    /// The words under the field: the caller's for a folder (the old field's `note`), the old wording for the command.
    note: SharedString,
    input: DraftInput,
    /// Settings places Save in the dialog footer. The launcher and host settings keep the field's own buttons.
    pinned_save: bool,
    pub(super) save_receipt: super::editing::SaveReceipt,
    /// Whether the latest read completed successfully. A pending read or a failure locks the field and both saves.
    loaded: bool,
    /// The load the rendered buttons belong to. A mouse event can reach the last frame after a fresh read has
    /// answered. The old Use default button must never clear a newer override that frame did not show.
    read_generation: u64,
    compare_on_save: bool,
    /// Why the last read failed, until a read answers; Retry takes Save's place meanwhile.
    unreadable: Option<HostText>,
    /// Whether the last read found a value of the scope's own, which offers Use default ([`Target::offers_default`]).
    has_override: bool,
    /// Whether the last save was done; the saved line shows while the user has not edited the field since.
    saved: bool,
    /// Whether the last press of Save or Use default sent nothing because another save held the key, which the line
    /// under the field says until a save of this field starts.
    refused: Refused,
    /// The window the field is drawn in, where a read's answer sets the field's text.
    window: AnyWindowHandle,
}

impl EventEmitter<CheckoutUpdated> for CheckoutField {}

impl CheckoutField {
    /// A folder field over `scope`'s folder with `note` under it, reading the folder now (see [`CheckoutField`]).
    pub fn folder(
        scope: FolderScope,
        note: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new(Target::Folder(scope), note.into(), window, cx)
    }

    /// Settings' all-hosts post-clone command, reading it now.
    pub(super) fn post_clone(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new(Target::PostClone, COMMAND_NOTE.into(), window, cx)
    }

    fn new(
        target: Target,
        note: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let state = target.input(window, cx);
        // Read-only until the read answers (the module docs); the draft is loaded empty and untouched, so it takes the
        // read's value when it comes.
        state.set_read_only(true, cx);
        let input = DraftInput::new(
            "",
            state,
            |field: &mut Self| &mut field.input,
            // Enter saves nothing (see [`CheckoutField`]); the growing box never reports it anyway.
            |_, _, _| {},
            window,
            cx,
        );
        let window = window.window_handle();
        Self::send_read(&target, window, cx);
        CheckoutField {
            target,
            note,
            input,
            pinned_save: false,
            save_receipt: super::editing::SaveReceipt::default(),
            loaded: false,
            read_generation: 0,
            compare_on_save: false,
            unreadable: None,
            has_override: false,
            saved: false,
            refused: Refused::default(),
            window,
        }
    }

    /// Read the field's route ([`choices::read_route`]), the answer going to this view if it is still open.
    fn send_read(target: &Target, window: AnyWindowHandle, cx: &mut Context<Self>) {
        let field = cx.weak_entity();
        let shown = format!("GET {}", target.words().shown);
        choices::read_route(target.path(), shown, cx, move |answer, cx| {
            let _ = window.update(cx, |_, window, cx| {
                let _ = field.update(cx, |field, cx| field.read_answered(answer, window, cx));
            });
        });
    }

    /// Install a fresh base and show it before unlocking the field. Reads only happen while editing is locked,
    /// so a reload cannot discard a new edit made after the save settled.
    fn read_answered(
        &mut self,
        answer: Result<Stored, HostText>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match answer {
            Ok(stored) => {
                self.unreadable = None;
                let value = self.target.value(&stored);
                let changed_after_refusal =
                    self.refusal().is_some() && value.as_deref() != self.input.stored();
                self.has_override = value.is_some();
                self.compare_on_save = stored.compare_on_save;
                if let Some(placeholder) = self.target.placeholder(&stored) {
                    self.input.input().set_placeholder(placeholder, window, cx);
                }
                self.input.reload(value, window, cx);
                self.read_generation += 1;
                self.loaded = true;
                self.input.input().set_read_only(false, cx);
                if changed_after_refusal {
                    cx.emit(CheckoutUpdated { saved_at: None });
                }
            }
            Err(reason) => self.unreadable = Some(reason),
        }
        cx.notify();
    }

    /// Only failed reads can be retried. A queued second Retry must not leave two reads racing to replace the base.
    fn retry(&mut self, cx: &mut Context<Self>) {
        if self.unreadable.take().is_none() {
            return;
        }
        Self::send_read(&self.target, self.window, cx);
        cx.notify();
    }

    /// Put the field's Save in Settings' footer while keeping its Retry and its outcomes beside the input.
    pub(super) fn with_pinned_save(mut self) -> Self {
        self.pinned_save = true;
        self
    }

    /// Wait for the fake helm's initial read before a golden types into this field.
    #[cfg(test)]
    pub(super) fn input_for_golden(&self) -> Option<TextInputState> {
        self.loaded.then(|| self.input.input().clone())
    }

    /// Folder and post-clone command have independent baselines even though Settings can save them in one action.
    pub(super) fn edit_state(&self) -> super::editing::EditState {
        let draft = self.input.draft();
        super::editing::EditState::of(
            draft.is_saving(),
            draft.is_touched() && draft.value() != self.input.stored().unwrap_or_default(),
        )
    }

    /// Restore this field without issuing a read or writing the other checkout field.
    pub(super) fn discard(&mut self, cx: &mut Context<Self>) {
        self.input.discard(self.window, cx);
        cx.notify();
    }

    /// The footer acts on the current load. Unlike a field-local button, it cannot clear a host override.
    pub(super) fn save_from_settings(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.save(false, self.read_generation, window, cx)
    }

    /// Save the draft, or with `use_default` clear the host's folder (the module docs). Does nothing before the read
    /// has answered, while it failed, while a save is in flight, for an untouched draft, and for Use default where it
    /// is not offered, all cases in which the button is drawn busy or not drawn. A save the operation store refuses
    /// because another save holds the key says so under the field ([`Refused`]).
    fn save(
        &mut self,
        use_default: bool,
        shown_read: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let draft = self.input.draft();
        if !self.loaded || self.read_generation != shown_read || draft.is_saving() {
            return false;
        }
        let allowed = if use_default {
            self.target.offers_default() && self.has_override
        } else {
            draft.is_touched()
        };
        if !allowed {
            return false;
        }
        let key = self.target.key();
        let target = self.target.clone();
        let expected = self
            .compare_on_save
            .then(|| self.input.stored().map(str::to_owned));
        let saved_at = Mark::take(cx);
        let started = DraftInput::save(
            self,
            |field| &mut field.input,
            &key,
            move |value| {
                target.command(
                    (!use_default).then_some(value),
                    expected.as_ref().map(|base| base.as_deref()),
                )
            },
            window,
            cx,
            move |field, settled, cx| {
                field.save_answered(&settled.outcome, saved_at, cx);
                field.save_receipt.settle(&settled.outcome, cx);
            },
        );
        // Another save may hold the key (any Settings save, for the all-hosts configuration), which the field then says.
        self.refused.record(self.target.words().id, &started);
        match &started {
            Ok(()) => self.saved = false,
            Err(not_started) => tracing::debug!(?not_started, "checkout field not saved"),
        }
        cx.notify();
        started.is_ok()
    }

    /// Keep the field locked across the save and its refetch. A refusal discards the edit now so even a failed read
    /// cannot leave the rejected intent around. This route finishes synchronously, so it never returns Accepted.
    fn save_answered(&mut self, outcome: &Outcome, saved_at: Mark, cx: &mut Context<Self>) {
        match outcome {
            Outcome::Done(_) => self.saved = true,
            Outcome::Refused(_) => self.input.discard(self.window, cx),
            Outcome::Accepted(_) => return,
        }
        self.loaded = false;
        self.input.input().set_read_only(true, cx);
        Self::send_read(&self.target, self.window, cx);
        if self.saved {
            cx.emit(CheckoutUpdated {
                saved_at: Some(saved_at),
            });
        }
    }

    /// The refused save belongs to the launcher even if a successful refresh takes the inline setup field away.
    pub(crate) fn refusal(&self) -> Option<&HostText> {
        self.input.draft().refusal()
    }

    /// The field's text, for tests (Settings' and the host dialog's), which find no other way to it: GPUI Kit's field
    /// reports no accessible value.
    #[cfg(test)]
    pub(crate) fn text(&self, cx: &App) -> String {
        self.input.input().text(cx).to_string()
    }

    /// Whether the field itself holds focus, for tests that type into it and must know the keys reach it.
    #[cfg(test)]
    pub(crate) fn focused(&self, window: &Window, cx: &App) -> bool {
        self.input.input().focus_handle(cx).is_focused(window)
    }

    /// The field is drawn and focusable while its read holds it read-only.
    pub(super) fn search_focus(&self, cx: &App) -> gpui::FocusHandle {
        self.input.input().focus_handle(cx)
    }

    /// A line under the field: the note, the saved line or a failure, at the note's size and line, as wide as the field
    /// so it wraps there.
    ///
    /// NOTE: The width is set because the field's column puts its children at the start (`items_start`, so the buttons
    /// keep their own width), and there a child is as wide as its content. A failure is a [`HostSentence`], whose
    /// content width is its whole unwrapped sentence, so without this a helm refusal longer than the field ran past
    /// the host settings dialog's edge and was cut off (found driving the dialog with a relative folder).
    fn line(color: gpui::Hsla) -> Div {
        div()
            .w_full()
            .text_size(metrics::TEXT_SM)
            .line_height(px(choices::OUTCOME_LINE_HEIGHT))
            .text_color(color)
    }
}

impl Render for CheckoutField {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let words = self.target.words();
        let id = |part: &str| format!("{}-{part}", words.id);
        let draft = self.input.draft();
        let saving = draft.is_saving();
        let shown_read = self.read_generation;
        let weak = cx.weak_entity();

        let input = TextInput::new(
            words.id,
            self.input.input(),
            words.label,
            HoverHelp::tooltip(words.help),
        )
        .monospace();
        let input = match self.target {
            Target::Folder(_) => input.height(px(choices::FIELD_HEIGHT)),
            Target::PostClone => input,
        };
        // The Settings card has its own heading above the fields. Host settings and the launcher keep their scale.
        let label = if self.pinned_save {
            choices::field_with_caption(
                words.id,
                input,
                super::card::caption(
                    words.label,
                    self.edit_state() != super::editing::EditState::Clean,
                    palette,
                ),
            )
        } else {
            choices::labelled(words.label, words.id, input, None)
        };

        // Retry in place of Save while the read has failed, as in the old field; Use default under Save where offered.
        let mut buttons = Vec::new();
        if self.unreadable.is_some() {
            let weak = weak.clone();
            buttons.push(
                choices::retry_loading(id("retry"), "Retry", words.retry_help, move |cx| {
                    let _ = weak.update(cx, |field, cx| field.retry(cx));
                })
                .busy(saving),
            );
        } else if !self.pinned_save {
            let save = weak.clone();
            buttons.push(
                Button::new(
                    id("save"),
                    if saving { "Saving…" } else { "Save" },
                    HoverHelp::tooltip(words.save_help),
                    Role::Button,
                )
                .tier(Tier::Primary)
                // Busy, not disabled, so a clicked Save keeps focus through its save, as the old `aria-disabled` did.
                .busy(!self.loaded || saving || !draft.is_touched())
                .on_click(move |_, window, cx| {
                    let _ = save.update(cx, |field, cx| field.save(false, shown_read, window, cx));
                }),
            );
            if self.target.offers_default() && self.has_override {
                let weak = weak.clone();
                buttons.push(
                    Button::new(
                        id("use-default"),
                        "Use default",
                        HoverHelp::tooltip(
                            "Use the default checkout folder from Settings → Checkouts",
                        ),
                        Role::Button,
                    )
                    .tier(Tier::Neutral)
                    .busy(!self.loaded || saving)
                    .on_click(move |_, window, cx| {
                        let _ =
                            weak.update(cx, |field, cx| field.save(true, shown_read, window, cx));
                    }),
                );
            }
        }

        // A failed refetch gives Retry its own reason without hiding why the edit was discarded.
        let refusal = draft.refusal();
        let read_error_id = if refusal.is_some() {
            "read-error"
        } else {
            "error"
        };
        let failures = [
            ("error", refusal),
            (read_error_id, self.unreadable.as_ref()),
        ]
        .into_iter()
        .filter_map(|(part, reason)| {
            reason.map(|reason| {
                let sentence = HostSentence::new(ElementId::Name(id(part).into())).host(reason);
                Self::line(theme::color(palette.danger))
                    .id(ElementId::Name(id(part).into()))
                    .debug_selector(|| id(part))
                    .role(Role::Alert)
                    .aria_label(sentence.accessible_name())
                    .child(sentence)
            })
        });
        let refused = self.refused.line(words.id, |id, text| {
            Self::line(theme::color(palette.danger))
                .id(ElementId::Name(id.clone().into()))
                .debug_selector(move || id)
                .role(Role::Alert)
                .aria_label(text)
                .child(text)
        });
        let saved = (self.unreadable.is_none() && self.saved && !saving && !draft.is_touched())
            .then(|| {
                Self::line(theme::color(palette.fg_1))
                    .id(ElementId::Name(id("saved").into()))
                    .debug_selector(|| id("saved"))
                    .role(Role::Status)
                    .aria_label(words.saved)
                    .child(words.saved)
            });

        div()
            .id(ElementId::Name(id("field").into()))
            .flex()
            .flex_col()
            .items_start()
            .gap(px(FIELD_GAP))
            .w_full()
            .min_w_0()
            .child(label)
            .child(Self::line(theme::color(palette.fg_1)).child(self.note.clone()))
            .children(buttons)
            .children(failures)
            .children(refused)
            .children(saved)
    }
}
