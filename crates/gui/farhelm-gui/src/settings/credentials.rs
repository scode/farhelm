//! Settings → Credentials: the helm's catalog of named secrets (GitHub tokens, agent subscription logins and API
//! endpoint tokens) in three lists, each credential edited in place with a write-only secret, and the Tensorlake API key
//! form above them.
//!
//! SPEC.md, "Session list" (the Credentials paragraphs) and "Security" (the credential catalog stores values
//! unencrypted); SPEC_impl.md, "Named credential catalog". Ported from the old app's `CredentialsSettings` and
//! `CredentialEditor` (`credentials_settings.rs`) and their storage warning (`StorageWarning` in
//! `tensorlake_settings.rs`), with their words, at reference commit `24bc0580`. Here's the TLDR of its rules:
//!
//! - **Metadata, read whenever the section shows.** The helm does not push the catalog, so the section reads `GET
//!   /api/credentials` (names, kinds, times and the recipes that use each, never a value) each time it is shown in an
//!   opening of the dialog, so that coming back from another section (Recipes, once it is built) shows the recipes'
//!   current use, and again after each of its own saves and deletes, as the old one reloaded after one. Each answer
//!   replaces the list: the UI channel answers commands in the order they were sent (SPEC_impl.md, "The UI channel":
//!   each command starts only after the previous one replied), so an earlier read never answers after a later one, and
//!   the old app's generation check has nothing to guard. A failed read keeps the last good list and shows the helm's
//!   reason with "Retry loading".
//! - **One editor in place.** Add or Edit replaces the overview with an [`Editor`] (Back, Name, the secret, Delete,
//!   Cancel and Save). A name and a kind are fixed once created: replacing sends only the new secret, and a blank one
//!   keeps the saved value (SPEC.md: "An empty replacement keeps the saved value"). The secret is never returned, so the
//!   editor of a saved credential starts empty. Back, Cancel and closing Settings drop the editor and its text without a
//!   question. The editor checks the name's grammar and the secret's shape before sending, as the old one did, and says
//!   why it sent nothing; the helm checks both again.
//! - **Saves are the operation store's work** under [`OpKey::credential`], the key of the credential they act on (the
//!   key names the resource saved, as a host's or a template's does). The secret is a [`DraftInput`]: a refused save
//!   keeps it with the helm's words for correction (SPEC.md: "a failed save keeps the draft and shows its reason"). A
//!   done save closes the editor and reads the catalog again; Delete does the same. A save outlives the editor: Back
//!   during one drops the editor, and the reply then reaches nobody, its delivery failures included. A Delete's reply
//!   closes only the editor that sent it, never one opened since.
//! - **Delivery failures after a save.** A replacement the helm committed but could not deliver everywhere still closes
//!   the editor (the value is saved; SPEC.md), and the overview then lists each host it did not reach with the helm's
//!   reason, until the next editor closes (Back and Cancel clear them, as the old app's did).
//! - **An open editor follows each read.** A read's answer refreshes the open editor's credential, so its "Used by
//!   recipes" line and Delete's availability follow it without touching the unsaved secret (SPEC.md: "Reference changes
//!   refresh an open credential editor"). The old app read again whenever the helm's change feed moved; the helm pushes
//!   neither credentials nor recipes over the UI channel, so here a change an agent or the command line makes while the
//!   section stays shown arrives with the next read, and a Delete it made stale is refused by the helm with its own words
//!   (`docs/gpui/parity/settings-and-hosts.md`, "Deviations").

mod editor;

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::credentials::{
    CODEX_LOGIN_CAP, CreateCredential, Credential, CredentialKind, DeliveryFailure, SaveCredential,
    SavedCredential, check_secret,
};
use gpui::{
    AnyElement, AnyWindowHandle, App, AppContext as _, Context, Div, ElementId, Entity, EntityId,
    FocusHandle, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    Role, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use super::choices::{self, LABEL_LINE_HEIGHT, LABEL_TEXT};
use super::tensorlake_key::TensorlakeKey;
use crate::components::{Button, HoverHelp, Tier};
use crate::focus::{self, Mark};
use crate::host_text::{HostSentence, HostText};
use crate::local_time::{self, Zone};
use crate::operations::{Command, Work};
use crate::theme::{self, metrics};
pub(super) use editor::Editor;

/// `.host-settings-section`'s gap between a group's rows.
const GROUP_GAP: f32 = 14.;
/// `.host-settings-section + .host-settings-section`: the rule's padding above each group after the first.
const GROUP_RULE_PAD: f32 = 20.;
/// `.recipe-list`'s gap between rows.
const ROW_GAP: f32 = 6.;
/// `.credential-storage-warning p`'s size, and its line: `line-height: normal` at 13 pixels in the bundled face.
const WARNING_TEXT: f32 = 13.;
const WARNING_LINE_HEIGHT: f32 = 15.7;

/// The old editor's words for a name outside the recipe-name grammar.
const NAME_RULE: &str = "Use lowercase letters, digits and hyphens; start with a letter, end with a letter or digit, and \
                         use at most 32 characters.";

/// `/api/credentials`, or with `name` the credential's own route.
fn path(name: Option<&str>) -> Result<ApiPath, PathError> {
    match name {
        Some(name) => ApiPath::api(&["credentials", name]),
        None => ApiPath::api(&["credentials"]),
    }
}

/// `method` on `path` with `body`, a save of the credential the route names, named in a wordless refusal by its method
/// and route (a credential's name is the user's own, not an internal id, so it is shown).
fn command(method: Method, path: ApiPath, body: Option<serde_json::Value>) -> Command {
    let shown = format!("{} {}", method.as_str(), path.as_str());
    // A deletion is the editor's explicit write too, so it is a save in the store's terms: one kind for every editor,
    // busy only on its own key.
    Command::new(Work::Save, method, path, body, shown)
}

/// What Save sends: `POST /api/credentials` with the name, kind and secret for a new credential, `PUT
/// /api/credentials/<name>` with only the secret for a saved one (the helm keeps the saved value for a blank one).
/// Turning either into JSON cannot fail (strings and an enum); if it ever did, the request would go without a body
/// and the helm would refuse it with its own words.
fn save_command(
    name: &str,
    kind: CredentialKind,
    replacing: bool,
    secret: &str,
) -> Result<Command, PathError> {
    if replacing {
        let body = SaveCredential {
            secret: Some(secret.to_owned()),
        };
        Ok(command(
            Method::Put,
            path(Some(name))?,
            serde_json::to_value(body).ok(),
        ))
    } else {
        let body = CreateCredential {
            name: name.to_owned(),
            kind,
            secret: secret.to_owned(),
        };
        Ok(command(
            Method::Post,
            path(None)?,
            serde_json::to_value(body).ok(),
        ))
    }
}

/// What a done save reports as not delivered: a replacement's acknowledgement lists them, a creation has none (no
/// recipe can use a name that did not exist).
fn delivery_failures(replacing: bool, body: Option<serde_json::Value>) -> Vec<DeliveryFailure> {
    if !replacing {
        return Vec::new();
    }
    body.and_then(|body| serde_json::from_value::<SavedCredential>(body).ok())
        .map(|saved| saved.delivery_failures)
        .unwrap_or_default()
}

/// The recipe-use line: the recipes' names, which are the user's own catalog entries, never host text.
fn usage(credential: Option<&Credential>) -> String {
    match credential {
        Some(credential) if !credential.used_by.is_empty() => {
            format!("Used by recipes: {}", credential.used_by.join(", "))
        }
        _ => "No recipes use this credential.".into(),
    }
}

/// When the credential was saved or last replaced, to the minute in `zone`, the viewer's local time: the old list's
/// `Saved 2026-11-14, 09:05` without the "UTC" it wrote then (the maintainer decided on 2026-11-15 that the app's times
/// are local).
fn saved_status(credential: &Credential, zone: Zone) -> String {
    let at = credential.updated_at;
    let verb = if credential.revision == 1 {
        "Saved"
    } else {
        "Replaced"
    };
    format!(
        "{verb} {}, {}",
        local_time::date(at, zone),
        local_time::clock(at, zone)
    )
}

/// What the editor checks before sending, as the old one did: a new credential's name against the recipe-name grammar,
/// and the secret's shape unless a saved credential's replacement is blank (which keeps the saved value). `None` when
/// both pass; the helm checks both again.
fn malformed(
    name: &str,
    kind: CredentialKind,
    replacing: bool,
    secret: &str,
) -> Option<&'static str> {
    if !replacing && farhelm_proto::recipes::check_recipe_name(name).is_err() {
        return Some(NAME_RULE);
    }
    if replacing && secret.trim().is_empty() {
        return None;
    }
    check_secret(kind, secret).err()
}

// ---- The three lists ----------------------------------------------------------------------------------------------

/// One of the overview's three lists, by its place in [`LISTS`]. Agent logins hold both agents' kinds behind one Add.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Group(usize);

/// What one list says and is found by, and the kind a credential its Add makes starts as.
struct List {
    heading: &'static str,
    add: &'static str,
    /// The Add button's element id, which tests and the test bridge find it by, and the prefix of the list's own ids.
    add_id: &'static str,
    /// Claude Code for an agent login, which the editor lets the user switch to Codex.
    new_kind: CredentialKind,
}

/// The lists, in the old overview's order.
const LISTS: [List; 3] = [
    List {
        heading: "GitHub tokens",
        add: "Add GitHub token",
        add_id: "credentials-add-github",
        new_kind: CredentialKind::GithubToken,
    },
    List {
        heading: "Agent subscription logins",
        add: "Add agent login",
        add_id: "credentials-add-agent-login",
        new_kind: CredentialKind::ClaudeLogin,
    },
    List {
        heading: "API endpoint tokens",
        add: "Add endpoint token",
        add_id: "credentials-add-endpoint",
        new_kind: CredentialKind::EndpointToken,
    },
];

impl Group {
    /// The agent logins' list, the one with an agent choice.
    const AGENT_LOGINS: Group = Group(1);

    /// The list a credential of `kind` is in.
    fn of(kind: CredentialKind) -> Group {
        match kind {
            CredentialKind::GithubToken => Group(0),
            CredentialKind::ClaudeLogin | CredentialKind::CodexLogin => Group::AGENT_LOGINS,
            CredentialKind::EndpointToken => Group(2),
        }
    }

    fn words(self) -> &'static List {
        &LISTS[self.0]
    }
}

// ---- The overview -------------------------------------------------------------------------------------------------

/// The Credentials section, a view of its own that Settings makes the first time the section shows in an opening and
/// keeps until the dialog closes (see the module docs).
pub(super) struct Credentials {
    /// The catalog as the last answered read listed it; `None` until a read answers.
    catalog: Option<Vec<Credential>>,
    /// Why the last read failed, until a read answers.
    unreadable: Option<HostText>,
    /// The open editor, in place of the overview.
    editor: Option<Entity<Editor>>,
    /// The hosts the last done replacement did not reach, until the next editor closes (Back and Cancel included).
    undelivered: Vec<DeliveryFailure>,
    key: Entity<TensorlakeKey>,
    /// Each list's Add button, by list, where focus goes when an editor closes.
    add: [FocusHandle; 3],
    window: AnyWindowHandle,
}

impl Credentials {
    /// The section, reading the catalog now, with `key` as its Tensorlake API key form.
    pub(super) fn new(
        key: Entity<TensorlakeKey>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut credentials = Credentials {
            catalog: None,
            unreadable: None,
            editor: None,
            undelivered: Vec::new(),
            key,
            add: [(); 3].map(|_| cx.focus_handle().tab_stop(true)),
            window: window.window_handle(),
        };
        credentials.read(cx);
        credentials
    }

    /// The open editor, for tests that read its secret's text: GPUI Kit's field reports no accessible value.
    #[cfg(test)]
    pub(super) fn editor(&self) -> Option<Entity<Editor>> {
        self.editor.clone()
    }

    /// The Tensorlake API key form, for tests that read its typed key: the field is masked.
    #[cfg(test)]
    pub(super) fn key(&self) -> Entity<TensorlakeKey> {
        self.key.clone()
    }

    /// Read the catalog; its answer replaces the list (the module docs say why answers cannot cross). The dialog calls
    /// it each time it shows the section again.
    pub(super) fn read(&mut self, cx: &mut Context<Self>) {
        let view = cx.weak_entity();
        choices::read_route(
            path(None),
            "GET /api/credentials",
            cx,
            move |answer: Result<Vec<Credential>, HostText>, cx| {
                let _ = view.update(cx, |view, cx| {
                    match answer {
                        Ok(catalog) => {
                            // An open editor takes its credential's current use (SPEC.md: "Reference changes refresh
                            // an open credential editor without discarding its unsaved replacement text").
                            if let Some(editor) = &view.editor {
                                editor.update(cx, |editor, cx| editor.refresh(&catalog, cx));
                            }
                            view.catalog = Some(catalog);
                            view.unreadable = None;
                        }
                        Err(reason) => view.unreadable = Some(reason),
                    }
                    cx.notify();
                });
            },
        );
    }

    /// Open an editor in place of the overview, for `existing` or a new credential of `group`'s first kind, with focus
    /// on its first field the user can type into once it is drawn, unless the user moves on before.
    fn open_editor(
        &mut self,
        group: Group,
        existing: Option<Credential>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mark = Mark::take(cx);
        let parent = cx.weak_entity();
        let editor = cx.new(|cx| Editor::new(parent, group, existing, window, cx));
        let target = editor.read(cx).start(cx);
        self.editor = Some(editor);
        focus::focus_in_layer(&target, &mark, window, cx);
        cx.notify();
    }

    /// The editor `editor` (by its entity id) is done: a press of Back or Cancel, or the done answer of a save or a
    /// Delete it sent. If it is still the open editor it closes, with focus on its list's (`group`'s) Add button unless
    /// the user moved on since `mark`, the press that closed it or that started the save; if another editor has opened
    /// since (Back while a Delete was out, then Add), that one stays open with its text. Either way `undelivered`
    /// replaces the overview's delivery-failure lines and the catalog is read again, so a deleted or created entry shows
    /// as it is now; Back and Cancel pass none, clearing the lines, as the old app's did. `window` is the window when
    /// the press itself closes the editor; a reply has none at hand, and then the window the section is drawn in is
    /// updated for the move.
    fn close_editor(
        &mut self,
        editor: EntityId,
        group: Group,
        undelivered: Vec<DeliveryFailure>,
        mark: &Mark,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        self.undelivered = undelivered;
        self.read(cx);
        if self.editor.as_ref().map(Entity::entity_id) != Some(editor) {
            cx.notify();
            return;
        }
        self.editor = None;
        // The button is drawn again by the next frame; the coordinator keeps the move until then.
        let target = self.add[group.0].clone();
        match window {
            Some(window) => focus::focus_in_layer(&target, mark, window, cx),
            None => {
                let mark = *mark;
                let _ = self.window.update(cx, |_, window, cx| {
                    focus::focus_in_layer(&target, &mark, window, cx)
                });
            }
        }
        cx.notify();
    }

    /// One list: its heading with Add, then a row per credential with Edit, or "None saved.".
    fn group(&self, group: Group, cx: &Context<Self>) -> gpui::Stateful<Div> {
        let palette = theme::palette(cx);
        let weak = cx.weak_entity();
        let loaded = self.catalog.is_some();
        let add = Button::new(
            group.words().add_id,
            group.words().add,
            HoverHelp::tooltip(group.words().add),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .focus(&self.add[group.0])
        .disabled(!loaded)
        .on_click({
            let weak = weak.clone();
            move |_, window, cx| {
                let _ = weak.update(cx, |view, cx| view.open_editor(group, None, window, cx));
            }
        });
        let heading = div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .child(
                div()
                    .id(ElementId::Name(
                        format!("{}-heading", group.words().add_id).into(),
                    ))
                    .role(Role::Heading)
                    .text_size(px(LABEL_TEXT))
                    .line_height(px(LABEL_LINE_HEIGHT))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::color(palette.fg_0))
                    .child(group.words().heading),
            )
            .child(add);
        let listed: Vec<&Credential> = self
            .catalog
            .iter()
            .flatten()
            .filter(|credential| Group::of(credential.kind) == group)
            .collect();
        let none = (loaded && listed.is_empty()).then(|| choices::help("None saved.", cx));
        let zone = local_time::zone(cx);
        let rows = listed.into_iter().map(|credential| {
            let edit_label = format!("Edit {}", credential.name);
            let existing = credential.clone();
            let weak = weak.clone();
            // `.recipe-row`: the name, its use and when it was saved, beside its Edit.
            div()
                .id(ElementId::Name(
                    format!("credential-{}", credential.name).into(),
                ))
                .role(Role::ListItem)
                .aria_label(SharedString::from(credential.name.clone()))
                .flex()
                .items_center()
                .gap(px(10.))
                .py(px(8.))
                .px(px(10.))
                .rounded(metrics::RADIUS_SM)
                .bg(theme::color(palette.bg_1))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .font_family(crate::assets::TERMINAL_FONT)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::color(palette.fg_0))
                                .child(credential.name.clone()),
                        )
                        .child(
                            div()
                                .text_size(metrics::TEXT_SM)
                                .text_color(theme::color(palette.fg_1))
                                .child(usage(Some(credential))),
                        )
                        .child(
                            div()
                                .text_size(metrics::TEXT_SM)
                                .text_color(theme::color(palette.fg_1))
                                .child(saved_status(credential, zone)),
                        ),
                )
                .child(
                    Button::new(
                        format!("credential-edit-{}", credential.name),
                        "Edit",
                        HoverHelp::tooltip(edit_label.clone()),
                        Role::Button,
                    )
                    .accessible_name(edit_label)
                    .tier(Tier::Neutral)
                    .on_click(move |_, window, cx| {
                        let existing = existing.clone();
                        let _ = weak.update(cx, |view, cx| {
                            view.open_editor(group, Some(existing), window, cx)
                        });
                    }),
                )
        });
        div()
            .id(ElementId::Name(
                format!("credentials-{}", group.words().add_id).into(),
            ))
            .role(Role::Group)
            .aria_label(group.words().heading)
            .flex()
            .flex_col()
            .gap(px(GROUP_GAP))
            .min_w_0()
            .border_t_1()
            .border_color(theme::color(palette.border_dim))
            .pt(px(GROUP_RULE_PAD))
            .child(heading)
            .children(none)
            .child(
                div()
                    .id(ElementId::Name(
                        format!("{}-list", group.words().add_id).into(),
                    ))
                    .role(Role::List)
                    .aria_label(group.words().heading)
                    .flex()
                    .flex_col()
                    .gap(px(ROW_GAP))
                    .children(rows),
            )
    }

    /// The overview: the delivery failures of the last replacement, the help, the warning, the Tensorlake key form and
    /// the three lists.
    fn overview(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        let palette = theme::palette(cx);
        let mut parts: Vec<AnyElement> = self
            .undelivered
            .iter()
            .enumerate()
            .map(|(index, failure)| {
                let id = format!("credentials-undelivered-{index}");
                let sentence = HostSentence::new(ElementId::Name(id.clone().into()))
                    .text("Saved; not delivered to ")
                    .host(&HostText::from_host(failure.host.clone()))
                    .text(": ")
                    .host(&HostText::from_host(failure.reason.clone()));
                choices::outcome_line(id, Role::Status, theme::color(palette.warn_bright))
                    .aria_label(sentence.accessible_name())
                    .child(sentence)
                    .into_any_element()
            })
            .collect();
        parts.push(
            choices::help(
                "Secrets Farhelm hands to hosts for you. Recipes refer to them by name, so replacing a token here never \
                 means editing a recipe.",
                cx,
            )
            .into_any_element(),
        );
        parts.push(
            storage_warning(
                "credentials-storage-warning",
                "Farhelm stores these, including the Tensorlake API key, in plain text in this helm's state, on the \
                 machine the helm runs on. Anyone who can read that folder can read them. Farhelm never shows a saved \
                 value again.",
                cx,
            )
            .into_any_element(),
        );
        parts.push(self.key.clone().into_any_element());
        if self.catalog.is_none() && self.unreadable.is_none() {
            parts.push(
                choices::help("Loading credentials…", cx)
                    .id("credentials-loading")
                    .role(Role::Status)
                    .into_any_element(),
            );
        }
        if let Some(reason) = &self.unreadable {
            let weak = cx.weak_entity();
            let retry = choices::retry_loading(
                "credentials-retry",
                "Retry loading",
                "Load the credential catalog again",
                move |cx| {
                    let _ = weak.update(cx, |view, cx| {
                        view.unreadable = None;
                        view.read(cx);
                        cx.notify();
                    });
                },
            );
            parts.extend(choices::failure_with_retry(
                choices::failure_outcome("credentials-load-error", reason, cx),
                retry,
            ));
        }
        parts.extend(
            [0, 1, 2]
                .map(Group)
                .map(|group| self.group(group, cx).into_any_element()),
        );
        parts
    }
}

impl Render for Credentials {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let shown: Vec<AnyElement> = match &self.editor {
            Some(editor) => vec![editor.clone().into_any_element()],
            None => self.overview(cx),
        };
        div()
            .id("credentials")
            .flex()
            .flex_col()
            .gap(px(GROUP_GAP))
            .w_full()
            .min_w_0()
            .children(shown)
    }
}

/// The danger-tinted "Not encrypted on disk" callout (`.credential-storage-warning`): its title in the danger ink over
/// `body`, on the dim danger fill with a 4-pixel danger edge on its left. Each of the overview and the editor draws one
/// (SPEC.md: "each show one prominent danger-tinted Not encrypted on disk warning"), and so does the Tensorlake API key
/// form where Settings → Tensorlake draws it, worded for that one key (`super::tensorlake_key`).
pub(super) fn storage_warning(
    id: &'static str,
    body: &'static str,
    cx: &App,
) -> gpui::Stateful<Div> {
    let palette = theme::palette(cx);
    div()
        .id(id)
        .debug_selector(|| id.to_owned())
        .role(Role::Note)
        .aria_label(SharedString::from(format!("Not encrypted on disk. {body}")))
        .flex()
        .flex_col()
        .gap(px(6.))
        .py(px(12.))
        .px(px(14.))
        .rounded(metrics::RADIUS_SM)
        .border_l_4()
        .border_color(theme::color(palette.danger))
        .bg(theme::color(palette.danger_bg_dim))
        .child(
            div()
                .text_size(px(LABEL_TEXT))
                .line_height(px(LABEL_LINE_HEIGHT))
                .font_weight(FontWeight::BOLD)
                .text_color(theme::color(palette.danger))
                .child("Not encrypted on disk"),
        )
        .child(
            div()
                .text_size(px(WARNING_TEXT))
                .line_height(px(WARNING_LINE_HEIGHT))
                .text_color(theme::color(palette.fg_0))
                .child(body),
        )
}

/// Read a chosen Codex login file's text: at most [`CODEX_LOGIN_CAP`] bytes, checked before reading, and UTF-8. The
/// errors are the old editor's words and never quote the file. Runs on the background executor, so a slow disk never
/// holds the window.
fn read_login(path: &std::path::Path) -> Result<String, &'static str> {
    const TOO_LARGE: &str = "A Codex login file must be at most 64 KiB.";
    const UNREADABLE: &str = "Could not read that login file.";
    let size = std::fs::metadata(path).map_err(|_| UNREADABLE)?.len();
    if size > CODEX_LOGIN_CAP as u64 {
        return Err(TOO_LARGE);
    }
    std::fs::read_to_string(path).map_err(|_| UNREADABLE)
}
