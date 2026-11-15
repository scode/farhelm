//! The launcher's managed-checkout destination: the Where row's Folder or Managed checkout choice, the repository field
//! with its suggestions, and the preview of the exact folder the checkout gets, which the folder field then shows.
//!
//! SPEC.md, "Managed checkouts" (the launcher parts) and SPEC_impl.md, "Checkout configuration and discovery". Ported
//! from the old launcher's `DestinationTypeControl` (`launch_controls.rs`), `github_checkout.rs` and the destination and
//! preview parts of `list/create_form.rs`, at reference commit `24bc0580`, with their words. What the user chose lives
//! in the draft ([`crate::launcher::draft::Checkout`]); what the helm said about it lives here. Here's the TLDR:
//!
//! - **The Where row** chooses the destination kind. Managed checkout reveals the repository field, the preview and a
//!   sentence on the folder's lifetime, and makes the folder field a read-only display of the preview's path; Folder
//!   brings back the folder as it was. Enter on it only chooses, as the old control's did.
//! - **Repository suggestions** (`POST /api/github-repositories`) are read while the user is choosing (no repository
//!   chosen, or the list open), 150 ms after the field, the host or its connection stops changing (SPEC_impl.md:
//!   "Browser requests debounce for 150 ms and discard stale host/query generations"). The list floats under the field
//!   on the layer manager's non-focusing popover, as the model field's does: a valid `owner/repo` typed into the field
//!   is always its first row, so an incomplete scan never stops a choice (SPEC.md: "Incomplete discovery is visible and
//!   does not prevent selecting a valid manually entered pair"), and an incomplete scan says so under the field. Enter
//!   picks the highlighted row and never launches.
//! - **The preview** (`POST /api/github-checkout-preview`) is asked for whenever what it answers for changes: the host,
//!   its connection and installation, the repository (or choosing it again), the checkout's name, the agent choices and
//!   the configuration revision last observed (SPEC.md: "Host, installation, destination, title, agent choice and
//!   observed configuration changes invalidate an undispatched preview"). Launch waits for it
//!   ([`super::super::request::Preview`]).
//! - **Latest wins, one read at a time**, for both reads, as the folder check does: an answer counts only while the
//!   launcher still asks what it was asked, so a late answer can neither bring back an old destination nor move focus
//!   ("Late responses cannot restore its authority or steal focus"); and a read is not sent while another of its kind
//!   is out, the next going out when that answers. The old launcher sent a preview per change; one at a time keeps a
//!   name typed key by key from queuing a preview per key behind each other on the ordered channel.
//! - **The configuration revision.** The helm's checkout configuration (the folder a host's checkouts go in) changes
//!   from Settings or the command line, and a preview taken before the change is stale. The launcher learns the
//!   current revision from its launch-history read ([`Launcher::observe_checkout_config_revision`]); a preview is
//!   asked again when it rises, and an answer from before the last observed revision is never accepted.
//!
//! NOTE: Not built yet, with the piece that brings them: the checkout naming rules beyond an untouched copied title
//! (Clone's `<title>-clone` default and its numbering), a lost race's refreshed preview, the inline checkout-folder setup
//! for a host without one, and a Clone of a managed-checkout session starting a checkout of its repository.

use std::time::Duration;

use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::session_requests::{
    GithubCheckoutPreviewBody, GithubRepositoriesBody, GithubRepositoriesReply,
};
use farhelm_proto::ui_channel::{HostStateView, HostView};
use farhelm_proto::{AcceptedGithubPreview, GithubRepo, parse_github_repo};
use gpui::{
    AnyElement, App, AppContext as _, Context, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, ScrollHandle, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, WeakEntity, Window, div, px,
};

use super::{LABEL_COLUMN, LABEL_GAP, Launcher, choices::status_line, help_line, row};
use crate::assets::Icon;
use crate::components::{
    HoverHelp, OptionRow, RadioGroup, RadioOption, Role, TextInput, TextInputEvent, TextInputState,
    Vertical,
};
use crate::focus::Mark;
use crate::host_text::{HostSentence, HostText};
use crate::launcher::draft::{
    AgentChoices, CommandChoices, DestinationKind, Field, Tab, repo_identifier,
};
use crate::launcher::request::Preview;
use crate::layers::{self, Anchor, LayerId, Popover};
use crate::theme::{self, metrics};
use crate::{operations, ticker};

/// How long the repository field, host and connection must stay unchanged before suggestions are read.
const SUGGEST_AFTER: Duration = Duration::from_millis(150);
/// The most rows the suggestions list offers, the old `repository_choices` cap.
const MAX_CHOICES: usize = 100;
/// `.launch-composer-repository-options`: at most 260 pixels tall, scrolling past that, like the launcher's other
/// floating lists; padded and spaced as they are.
const LIST_MAX_HEIGHT: f32 = 260.;
const LIST_PAD: f32 = 6.;
const LIST_GAP: f32 = 2.;

/// The sentence under the repository field and preview while a managed checkout is chosen, the old launcher's
/// `.launch-composer-checkout-explanation` for an installed host.
const EXPLANATION: &str = "Farhelm clones the repository into a new folder named after the session, under this host's \
                           checkout folder. When the last session using it is deleted, the folder moves to the trash.";
/// The suggestions list's words when it has no row: nothing typed yet, or nothing that matches or parses.
const TYPE_A_PAIR: &str = "Type owner/repo to choose a repository";
const NO_MATCH: &str = "No repository matches; enter owner/repo";
/// What the field note says for a scan the helm cut short, when it gave no reason.
const INCOMPLETE: &str =
    "Repository search is incomplete; a valid owner/repo can still be selected";
/// The words for an answer about another installation than the one the launcher sees now.
const OTHER_INSTALLATION: &str = "Repository suggestions belong to a different host installation";
const STALE_PREVIEW: &str =
    "The preview is stale or belongs to a different host installation; select the repository again";

// ---------------------------------------------------------------------------------------------------------------------
// One read at a time, latest wins
// ---------------------------------------------------------------------------------------------------------------------

/// One kind of read the destination makes, keyed by what it asks about: the last key asked, its answer while it is
/// still the current one, whether a read is out and whether another is due when it answers, and the debounce deadline
/// (see the module docs). The folder check keeps the same facts for its own read.
struct Latest<K, A> {
    /// The key the pending deadline, the read out, or the last answer is about.
    asked: Option<K>,
    /// The answer for `asked`, once it came.
    answered: Option<A>,
    /// Whether a read is out, which holds the next one until it answers.
    out: bool,
    /// Whether the key changed while a read was out: its answer then sends the read for the current key.
    due: bool,
    /// The debounce deadline before the next read, for a read that waits for the input to settle; dropping it cancels
    /// it.
    _waiting: Option<ticker::Deadline>,
}

impl<K, A> Default for Latest<K, A> {
    fn default() -> Self {
        Latest {
            asked: None,
            answered: None,
            out: false,
            due: false,
            _waiting: None,
        }
    }
}

impl<K: Clone + PartialEq, A> Latest<K, A> {
    /// Follow `key`, what the read would ask about now: returns the key to read now when it changed and nothing holds
    /// it, else `None`. A change forgets the answer for the old key at once, so nothing about it shows a moment longer.
    fn follow(&mut self, key: Option<K>) -> Option<K> {
        if key == self.asked {
            return None;
        }
        self.asked = key.clone();
        self.answered = None;
        self._waiting = None;
        self.due = false;
        key
    }

    /// The answer, while it is about `key`.
    fn answer_for(&self, key: &K) -> Option<&A> {
        (self.asked.as_ref() == Some(key))
            .then_some(self.answered.as_ref())
            .flatten()
    }
}

/// What a suggestions read asks: on which host connection and installation, for which typed text.
#[derive(Clone, Debug, PartialEq, Eq)]
struct SuggestKey {
    host: i64,
    incarnation: u64,
    identity: String,
    query: String,
}

/// What a preview answers for (the module docs): a change to any of it asks again.
#[derive(Clone, Debug, PartialEq)]
struct PreviewKey {
    host: i64,
    incarnation: u64,
    identity: String,
    repo: GithubRepo,
    /// How many times a repository was chosen, so choosing the same one again asks again.
    chosen: u64,
    title: String,
    /// The agent choices as the draft holds them: SPEC.md lists "agent choice" among what invalidates a preview.
    agent: (Tab, AgentChoices, CommandChoices),
    /// The configuration revision last observed when this was asked.
    revision: Option<i64>,
}

/// The helm's answer to a preview: the folder, or its refusal in its words.
type PreviewAnswer = Result<AcceptedGithubPreview, HostText>;

/// The suggestions read's answer, or why there is none.
type SuggestAnswer = Result<GithubRepositoriesReply, HostText>;

// ---------------------------------------------------------------------------------------------------------------------
// The destination's state
// ---------------------------------------------------------------------------------------------------------------------

/// What the managed-checkout destination holds besides the draft: its two fields, the suggestions list, the reads, and
/// the configuration revision observed. None of it is shared with any other surface, and none of it survives the
/// launcher.
pub(super) struct CheckoutState {
    /// The repository field: the chosen repository, or what is typed.
    pub(super) repository: TextInputState,
    /// The read-only folder field shown in place of the folder while a managed checkout is chosen: the preview's
    /// path, escaped, or empty with a placeholder saying why there is none.
    path: TextInputState,
    /// The open suggestions list's layer, if open; the layer manager closes it itself on Escape or a press outside,
    /// so [`Launcher::repository_list_open`] asks it.
    list: Option<LayerId>,
    /// The row moved to with the arrows, as an index into the current rows.
    active: Option<usize>,
    /// Where the repository field is drawn, for the list to open under it; refreshed every frame.
    anchor: Option<Anchor>,
    suggestions: Latest<SuggestKey, SuggestAnswer>,
    preview: Latest<PreviewKey, PreviewAnswer>,
    /// The highest checkout configuration revision observed, which never goes back down; `None` until one is.
    revision: Option<i64>,
    /// The placeholders last given to the read-only folder field and the name field, so a frame that changes neither
    /// leaves the fields alone (setting one redraws its field).
    path_placeholder: &'static str,
    pub(super) name_placeholder: String,
    _subscriptions: Vec<Subscription>,
}

impl CheckoutState {
    /// The destination's fields for a launcher opening on `field_text` in the repository field, with their own
    /// subscriptions: what is typed edits the draft, Enter picks a suggestion and never launches, and focus opens
    /// and closes the list.
    pub(super) fn new(field_text: &str, window: &mut Window, cx: &mut Context<Launcher>) -> Self {
        let repository = TextInputState::new(window, cx);
        repository.set_text(field_text.to_owned(), window, cx);
        repository.set_placeholder("owner/repo", window, cx);
        let path = TextInputState::new(window, cx);
        path.set_read_only(true, cx);
        let events =
            repository.subscribe(window, cx, |launcher: &mut Launcher, event, window, cx| {
                match event {
                    TextInputEvent::Changed => launcher.repository_typed(window, cx),
                    TextInputEvent::Submitted { held } => {
                        if !held {
                            launcher.repository_enter(window, cx);
                        }
                    }
                }
            });
        let handle = repository.focus_handle(cx);
        // Deferred out of the focus phase, as the model field's are (`model_field`), so the list shows at once.
        let focus = cx.on_focus(&handle, window, |_, window, cx| {
            cx.defer_in(window, |launcher, window, cx| {
                launcher.open_repository_list(window, cx);
                cx.notify();
            });
        });
        let blur = cx.on_blur(&handle, window, |_, window, cx| {
            cx.defer_in(window, |launcher, window, cx| {
                launcher.close_repository_list(window, cx);
                cx.notify();
            });
        });
        CheckoutState {
            repository,
            path,
            list: None,
            active: None,
            anchor: None,
            suggestions: Latest::default(),
            preview: Latest::default(),
            revision: None,
            path_placeholder: "",
            name_placeholder: String::new(),
            _subscriptions: vec![events, focus, blur],
        }
    }
}

/// The suggestions for `query`: a valid typed pair first, then the discovered repositories whose `owner/repo` contains
/// the query, in the helm's order, each once, at most [`MAX_CHOICES`] (the old `repository_choices`).
pub(in crate::launcher) fn repository_choices(
    query: &str,
    discovered: &[GithubRepo],
) -> Vec<GithubRepo> {
    let query = query.trim().to_ascii_lowercase();
    let mut choices = Vec::new();
    if let Ok(repo) = parse_github_repo(&query) {
        choices.push(repo);
    }
    for repo in discovered {
        if choices.len() == MAX_CHOICES {
            break;
        }
        if repo_identifier(repo).contains(&query) && !choices.contains(repo) {
            choices.push(repo.clone());
        }
    }
    choices
}

// ---------------------------------------------------------------------------------------------------------------------
// Following the draft: when to read
// ---------------------------------------------------------------------------------------------------------------------

impl Launcher {
    /// The chosen host as the store holds it now, while it is connected with an installation identity: the only host
    /// either read can ask about, since the helm binds both to that identity.
    fn checkout_host(&self, cx: &App) -> Option<HostView> {
        let id = self.draft.host?;
        let view = self
            .store
            .read(cx)
            .collections()
            .ordered::<HostView>()
            .into_iter()
            .find(|view| view.id == id)?
            .clone();
        let connected = matches!(view.state, HostStateView::Connected { .. });
        let identified = view.identity.as_deref().is_some_and(|id| !id.is_empty());
        (connected && identified).then_some(view)
    }

    /// Whether the user is choosing a repository: no repository chosen yet, or the list open over a chosen one.
    fn choosing_repository(&self, window: &Window, cx: &App) -> bool {
        self.draft
            .checkout
            .as_ref()
            .is_some_and(|checkout| checkout.repo.is_none())
            || self.repository_list_open(window, cx)
    }

    /// What a suggestions read would ask now, if the user is choosing on a host that can be asked.
    fn suggest_key(&self, window: &Window, cx: &App) -> Option<SuggestKey> {
        let checkout = self.draft.checkout.as_ref()?;
        if !self.choosing_repository(window, cx) {
            return None;
        }
        let host = self.checkout_host(cx)?;
        Some(SuggestKey {
            host: host.id,
            incarnation: host.incarnation,
            identity: host.identity.unwrap_or_default(),
            query: checkout.field_text(),
        })
    }

    /// What a preview would answer for now, if a repository is chosen on a host that can be asked.
    fn preview_key(&self, cx: &App) -> Option<PreviewKey> {
        let checkout = self.draft.checkout.as_ref()?;
        let repo = checkout.repo.clone()?;
        let host = self.checkout_host(cx)?;
        Some(PreviewKey {
            host: host.id,
            incarnation: host.incarnation,
            identity: host.identity.unwrap_or_default(),
            repo,
            chosen: checkout.chosen,
            title: self.draft.checkout_title(),
            agent: (
                self.draft.tab,
                self.draft.agent.clone(),
                self.draft.command.clone(),
            ),
            revision: self.checkout.revision,
        })
    }

    /// Follow the destination as drawn: ask for suggestions 150 ms after what they would be about last changed, and
    /// for a preview as soon as what it answers for changes, each one at a time (the module docs). Called while drawing,
    /// since a host's connection changes by a push with no edit of the launcher's; it also keeps the folder field's
    /// display of the preview in step.
    pub(super) fn follow_checkout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let key = self.suggest_key(window, cx);
        if let Some(key) = self.checkout.suggestions.follow(key) {
            let launcher = cx.weak_entity();
            self.checkout.suggestions._waiting =
                Some(ticker::after(SUGGEST_AFTER, cx, move |cx| {
                    let _ = launcher.update(cx, |launcher, cx| launcher.send_suggestions(key, cx));
                }));
        }
        let key = self.preview_key(cx);
        if let Some(key) = self.checkout.preview.follow(key) {
            self.send_preview(key, cx);
        }
        self.show_checkout_path(window, cx);
    }

    /// Read the suggestions for `key`, if it is still what the launcher would ask and no read is out (then the
    /// answer sends it).
    fn send_suggestions(&mut self, key: SuggestKey, cx: &mut Context<Self>) {
        let state = &mut self.checkout.suggestions;
        if state.asked.as_ref() != Some(&key) {
            return;
        }
        if state.out {
            state.due = true;
            return;
        }
        let Ok(path) = ApiPath::api(&["github-repositories"]) else {
            return;
        };
        let body = GithubRepositoriesBody {
            host: key.host,
            expected_incarnation: Some(key.incarnation),
            query: key.query.clone(),
        };
        let Ok(body) = serde_json::to_value(&body) else {
            return;
        };
        let launcher = cx.weak_entity();
        state.out = true;
        let sent = operations::send(
            Method::Post,
            path,
            Some(body),
            "POST /api/github-repositories",
            cx,
            move |answer, cx| {
                let answer = operations::decode::<GithubRepositoriesReply>(
                    answer,
                    "POST /api/github-repositories",
                );
                let _ = launcher.update(cx, |launcher, cx| {
                    let state = &mut launcher.checkout.suggestions;
                    state.out = false;
                    if state.asked.as_ref() == Some(&key) {
                        state.answered = Some(answer.and_then(|reply| {
                            // The helm names the installation that answered; one other than the launcher asked about is
                            // no answer about this host (the old `RepositoryAuthority::accepts`).
                            let same = reply.host == key.host.to_string()
                                && reply.incarnation == key.incarnation
                                && reply.installation_identity == key.identity;
                            if same {
                                Ok(reply)
                            } else {
                                Err(HostText::from_host(OTHER_INSTALLATION))
                            }
                        }));
                        cx.notify();
                    }
                    if std::mem::take(&mut launcher.checkout.suggestions.due)
                        && let Some(now) = launcher.checkout.suggestions.asked.clone()
                    {
                        launcher.send_suggestions(now, cx);
                    }
                });
            },
        );
        if !sent {
            self.checkout.suggestions.out = false;
        }
    }

    /// Ask for the preview `key` answers for, if it is still current and no preview is out (then the answer sends
    /// it). The preview is a read: it creates and reserves nothing (SPEC.md: "Preview creates no directory and reserves
    /// nothing").
    fn send_preview(&mut self, key: PreviewKey, cx: &mut Context<Self>) {
        let state = &mut self.checkout.preview;
        if state.asked.as_ref() != Some(&key) {
            return;
        }
        if state.out {
            state.due = true;
            return;
        }
        let Ok(path) = ApiPath::api(&["github-checkout-preview"]) else {
            return;
        };
        let body = GithubCheckoutPreviewBody {
            host: key.host,
            expected_incarnation: Some(key.incarnation),
            repo: repo_identifier(&key.repo),
            // The old launcher always sent the name, an empty one for an unnamed checkout.
            title: Some(key.title.clone()),
        };
        let Ok(body) = serde_json::to_value(&body) else {
            return;
        };
        let launcher = cx.weak_entity();
        state.out = true;
        let sent = operations::send(
            Method::Post,
            path,
            Some(body),
            "POST /api/github-checkout-preview",
            cx,
            move |answer, cx| {
                let answer = operations::decode::<AcceptedGithubPreview>(
                    answer,
                    "POST /api/github-checkout-preview",
                );
                let _ = launcher.update(cx, |launcher, cx| {
                    let state = &mut launcher.checkout.preview;
                    state.out = false;
                    if state.asked.as_ref() == Some(&key) {
                        state.answered = Some(answer.and_then(|preview| {
                            if preview_matches(&key, &preview) {
                                Ok(preview)
                            } else {
                                Err(HostText::from_host(STALE_PREVIEW))
                            }
                        }));
                        cx.notify();
                    }
                    if std::mem::take(&mut launcher.checkout.preview.due)
                        && let Some(now) = launcher.checkout.preview.asked.clone()
                    {
                        launcher.send_preview(now, cx);
                    }
                });
            },
        );
        if !sent {
            self.checkout.preview.out = false;
        }
    }

    /// The helm's launch-history read observed the checkout configuration at `revision` (its
    /// `checkout_config_revision`): a preview taken under an older revision may name a folder the host's checkouts no
    /// longer go in, so a higher revision than any observed asks for the preview again, and an answer from before it is
    /// never accepted. A lower or equal one is only noted. The revision never goes back down: an older read answering
    /// late cannot undo a newer one.
    ///
    /// Called by the launcher's launch-history read, which brings the revision; until that read exists only the tests
    /// call it.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn observe_checkout_config_revision(
        &mut self,
        revision: i64,
        cx: &mut Context<Self>,
    ) {
        if self.checkout.revision.is_some_and(|seen| seen >= revision) {
            return;
        }
        self.checkout.revision = Some(revision);
        cx.notify();
    }

    /// The preview as [`super::super::request::Preview`] puts it for the draft as it is now.
    pub(super) fn checkout_preview(&self, cx: &App) -> Preview<'_> {
        let Some(key) = self.preview_key(cx) else {
            return Preview::Waiting;
        };
        match self.checkout.preview.answer_for(&key) {
            Some(Ok(preview)) => Preview::Ready(preview),
            Some(Err(_)) => Preview::Failed,
            None => Preview::Waiting,
        }
    }
}

/// Whether `preview` answers for `key`: the host, its connection and installation it was asked about, under a
/// configuration revision no older than the one observed when asked (the old `PreviewAuthority::accepts`). A preview may
/// see a revision newer than the launcher has observed, since the history read can lag a configuration change.
fn preview_matches(key: &PreviewKey, preview: &AcceptedGithubPreview) -> bool {
    preview.host == key.host.to_string()
        && preview.incarnation == key.incarnation
        && !key.identity.is_empty()
        && preview.installation_identity == key.identity
        && key
            .revision
            .is_none_or(|seen| preview.binding.config_revision >= seen)
}

// ---------------------------------------------------------------------------------------------------------------------
// The repository field and its list
// ---------------------------------------------------------------------------------------------------------------------

impl Launcher {
    /// Whether the suggestions list is open: opened by the field and not since closed by the layer manager.
    pub(super) fn repository_list_open(&self, window: &Window, cx: &App) -> bool {
        self.checkout
            .list
            .is_some_and(|layer| layers::is_open(layer, window, cx))
    }

    /// The list's rows for the field as it is now, from the suggestions answer while it is about this host and text.
    fn repository_rows(&self, window: &Window, cx: &App) -> Vec<GithubRepo> {
        let query = self.checkout.repository.text(cx).to_string();
        let discovered = self
            .suggest_key(window, cx)
            .and_then(|key| self.checkout.suggestions.answer_for(&key).cloned())
            .and_then(Result::ok)
            .map(|reply| reply.repos)
            .unwrap_or_default();
        repository_choices(&query, &discovered)
    }

    /// The user typed in the repository field: the draft's query, the highlight back to none, and the list open.
    fn repository_typed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.checkout.repository.text(cx).to_string();
        self.draft.edit_repository_query(text);
        self.checkout.active = None;
        if !self.repository_list_open(window, cx) {
            self.open_repository_list(window, cx);
        }
        self.changed(cx);
    }

    /// Up or Down in the field: move the highlighted row, wrapping, while the list is open and has rows.
    fn repository_arrow(&mut self, arrow: Vertical, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.repository_rows(window, cx).len();
        if !self.repository_list_open(window, cx) || count == 0 {
            return;
        }
        let active = &mut self.checkout.active;
        *active = Some(match (arrow, *active) {
            (Vertical::Down, None) => 0,
            (Vertical::Down, Some(index)) => (index + 1) % count,
            (Vertical::Up, None) => count - 1,
            (Vertical::Up, Some(index)) => (index + count - 1) % count,
        });
        cx.notify();
    }

    /// Enter in the field: pick the highlighted row (or, with none highlighted, the first) while the list is open, or
    /// open it. Never launches: in the old field, choosing a repository and launching were separate keys.
    fn repository_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.repository_list_open(window, cx) {
            self.open_repository_list(window, cx);
            cx.notify();
            return;
        }
        let rows = self.repository_rows(window, cx);
        if let Some(repo) = rows.get(self.checkout.active.unwrap_or(0)).cloned() {
            self.pick_repository(repo, window, cx);
        }
    }

    /// A row picked by a click or Enter: the draft's repository, the field showing it, the list closed.
    fn pick_repository(&mut self, repo: GithubRepo, window: &mut Window, cx: &mut Context<Self>) {
        self.draft.choose_repository(repo);
        self.close_repository_list(window, cx);
        self.sync_fields(window, cx);
        self.changed(cx);
    }

    /// Open the list under the field, as a popover that never takes focus. Not while the field is read-only (the mode
    /// holds the destination).
    fn open_repository_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.draft.holds(Field::Destination) || self.repository_list_open(window, cx) {
            return;
        }
        let Some(anchor) = self.checkout.anchor.clone() else {
            return;
        };
        self.checkout.active = None;
        let launcher = cx.weak_entity();
        let rows = cx.new(|cx| RepositoryRows::new(launcher, cx));
        let container = cx.focus_handle();
        let mark = Mark::take(cx);
        self.checkout.list = layers::open_popover(
            Popover::floating(rows, container, anchor),
            &mark,
            window,
            cx,
        );
    }

    /// Close the list if it is open, and drop the highlight.
    fn close_repository_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(layer) = self.checkout.list.take()
            && layers::is_open(layer, window, cx)
        {
            layers::close(layer, window, cx);
        }
        self.checkout.active = None;
    }

    /// The Where row chose `kind`: the draft's destination, with the fields it changes put back in step.
    fn choose_destination(
        &mut self,
        kind: DestinationKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if kind == self.draft.destination_kind() {
            return;
        }
        self.draft.choose_destination(kind);
        self.close_repository_list(window, cx);
        self.sync_fields(window, cx);
        self.changed(cx);
    }

    /// Put the read-only folder field in step with the preview: its path, escaped as host text is, or empty with a
    /// placeholder saying why there is none (the old placeholders).
    fn show_checkout_path(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(checkout) = &self.draft.checkout else {
            return;
        };
        let (text, placeholder) = match (&checkout.repo, self.checkout_preview(cx)) {
            (None, _) => (String::new(), "Select a repository"),
            (Some(_), Preview::Ready(preview)) => (
                farhelm_proto::text::escaped(&preview.binding.cwd).to_string(),
                "",
            ),
            (Some(_), Preview::Waiting) => (String::new(), "Waiting for checkout preview"),
            (Some(_), Preview::Failed) => (String::new(), "Checkout preview unavailable"),
        };
        let path = &self.checkout.path;
        if path.text(cx).as_ref() != text {
            path.set_text(text, window, cx);
        }
        if self.checkout.path_placeholder != placeholder {
            path.set_placeholder(placeholder, window, cx);
            self.checkout.path_placeholder = placeholder;
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------------------------------------------------

impl Launcher {
    /// The Where row: Folder or Managed checkout, with their glyphs (the old `DestinationTypeControl`). Not drawn where
    /// the mode holds the destination.
    pub(super) fn where_row(
        &self,
        palette: &theme::Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if self.draft.holds(Field::Destination) {
            return None;
        }
        let weak = cx.weak_entity();
        let group = RadioGroup::new(
            "launcher-destination",
            "Destination type",
            HoverHelp::tooltip("Where: an existing folder, or a new managed checkout of a repository"),
            Role::Group,
        )
        .option(
            RadioOption::new(
                "Folder",
                HoverHelp::tooltip("Folder: start in an existing folder on this host"),
                DestinationKind::Folder,
            )
            .icon(Icon::Folder),
        )
        .option(
            RadioOption::new(
                "Managed checkout",
                HoverHelp::tooltip(
                    "Managed checkout: Farhelm clones a repository into a new folder for this session",
                ),
                DestinationKind::Checkout,
            )
            .icon(Icon::Branch),
        )
        .chosen(Some(self.draft.destination_kind()))
        .on_change(move |kind, window, cx| {
            let _ = weak.update(cx, |launcher, cx| {
                launcher.choose_destination(*kind, window, cx)
            });
        });
        Some(row("launcher-where-row", "Where", group, palette).into_any_element())
    }

    /// The managed checkout's rows, under the host: the repository field, the note on an incomplete scan, the preview
    /// and the lifetime sentence. Empty for a folder destination.
    ///
    /// It also notices a list the layer manager closed on its own (Escape, a press outside), as the model field does.
    pub(super) fn checkout_rows(
        &mut self,
        palette: &theme::Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        if self.checkout.list.is_some() && !self.repository_list_open(window, cx) {
            self.close_repository_list(window, cx);
        }
        let Some(checkout) = self.draft.checkout.clone() else {
            return Vec::new();
        };
        let anchor = Anchor::for_element("launcher-repository-anchor", window, cx).matching_width();
        let tracker = anchor.track();
        self.checkout.anchor = Some(anchor.clone());
        let weak = cx.weak_entity();
        let field = TextInput::new(
            "launcher-repository",
            &self.checkout.repository,
            "Repository",
            HoverHelp::tooltip(
                "Repository: the GitHub owner/repo to clone; type to filter, then choose one",
            ),
        )
        .monospace()
        .on_vertical(move |arrow, window, cx| {
            let _ = weak.update(cx, |launcher, cx| {
                launcher.repository_arrow(arrow, window, cx)
            });
        });
        let control = div().relative().min_w_0().child(field).child(tracker);
        let mut rows =
            vec![row("launcher-repository-row", "Repository", control, palette).into_any_element()];
        if self.choosing_repository(window, cx)
            && let Some(note) = self.repository_note(window, cx)
        {
            rows.push(note);
        }
        if let Some(repo) = &checkout.repo {
            rows.push(self.preview_block(repo, palette, cx));
        }
        rows.push(help_line(EXPLANATION, palette).into_any_element());
        rows
    }

    /// The note under the field on an incomplete scan, or a failed read (`.launch-composer-repository-note`): the
    /// helm's reason in the warning color, or, for a scan it cut short without one, a quiet hint that a typed pair can
    /// still be chosen.
    fn repository_note(&self, window: &Window, cx: &App) -> Option<AnyElement> {
        let key = self.suggest_key(window, cx)?;
        let palette = theme::palette(cx);
        match self.checkout.suggestions.answer_for(&key)? {
            Ok(reply) => match &reply.scan_error {
                Some(error) => Some(
                    status_line("launcher-repository-note", error.clone(), palette)
                        .into_any_element(),
                ),
                // A quiet hint, but announced as the other notes are.
                None if reply.truncated => Some(
                    help_line(INCOMPLETE, palette)
                        .role(Role::Status)
                        .into_any_element(),
                ),
                None => None,
            },
            Err(words) => Some(
                status_line("launcher-repository-note", words.raw().to_owned(), palette)
                    .into_any_element(),
            ),
        }
    }

    /// The preview (`.launch-composer-checkout-preview`): which repository on which host, the exact folder once the
    /// helm has answered, and while it has not, that the launcher waits, or the helm's refusal. A polite live status,
    /// so a change is announced without interrupting.
    fn preview_block(&self, repo: &GithubRepo, palette: &theme::Palette, cx: &App) -> AnyElement {
        let host = self
            .draft
            .host
            .and_then(|id| {
                self.store
                    .read(cx)
                    .collections()
                    .ordered::<HostView>()
                    .into_iter()
                    .find(|view| view.id == id)
                    .map(super::host_name_with_phase)
            })
            .unwrap_or_else(|| HostText::from_host("this host"));
        let heading = HostSentence::new("launcher-preview-heading")
            .text("Managed checkout of ")
            .host(&HostText::from_host(repo_identifier(repo)))
            .text(" on ")
            .host(&host);
        let key = self.preview_key(cx);
        let answer = key
            .as_ref()
            .and_then(|key| self.checkout.preview.answer_for(key));
        let (state, detail): (&str, AnyElement) = match answer {
            Some(Ok(preview)) => (
                "ready",
                div()
                    .font_family(crate::assets::TERMINAL_FONT)
                    .child(
                        HostSentence::new("launcher-preview-path")
                            .host(&HostText::from_host(preview.binding.cwd.as_str())),
                    )
                    .into_any_element(),
            ),
            Some(Err(words)) => (
                "failed",
                div()
                    .text_color(theme::color(palette.warn_bright))
                    .child(HostSentence::new("launcher-preview-error").host(words))
                    .into_any_element(),
            ),
            None => (
                "pending",
                div()
                    .child("Waiting for a current checkout preview")
                    .into_any_element(),
            ),
        };
        div()
            .id("launcher-preview")
            .debug_selector(move || format!("launcher-preview-{state}"))
            .role(Role::Status)
            .aria_label(heading.accessible_name())
            .pl(px(LABEL_COLUMN + LABEL_GAP))
            .flex()
            .flex_col()
            .gap(px(2.))
            .text_size(metrics::TEXT_SM)
            .text_color(theme::color(palette.fg_1))
            .child(heading)
            .child(detail)
            .into_any_element()
    }

    /// The read-only folder field's focus handle, where the launcher starts while a managed checkout is chosen.
    pub(super) fn checkout_folder_focus(&self, cx: &App) -> gpui::FocusHandle {
        self.checkout.path.focus_handle(cx)
    }

    /// The folder field while a managed checkout is chosen: read-only, showing the preview's path (SPEC.md: "the
    /// composer's folder field shows the current preview's effective path read-only, or a pending or error state").
    pub(super) fn checkout_folder_field(&self) -> AnyElement {
        TextInput::new(
            "launcher-checkout-folder",
            &self.checkout.path,
            "Folder",
            HoverHelp::tooltip(
                "Folder: the new managed checkout's folder on the host, as the helm previews it",
            ),
        )
        .monospace()
        .into_any_element()
    }

    /// The footer's and main action's words for the destination while a managed checkout is chosen, in the old
    /// footer's words: what to do while no repository is chosen, the repository while its preview is not in, and the
    /// previewed folder once it is. `None` for a folder destination.
    pub(super) fn checkout_summary(&self, cx: &App) -> Option<HostText> {
        let checkout = self.draft.checkout.as_ref()?;
        let words = match (&checkout.repo, self.checkout_preview(cx)) {
            (None, _) if checkout.query.is_empty() => {
                "Managed checkout (select a repository)".to_owned()
            }
            (None, _) => format!("Managed checkout: {} (select a repository)", checkout.query),
            (Some(_), Preview::Ready(preview)) => preview.binding.cwd.clone(),
            (Some(repo), Preview::Waiting | Preview::Failed) => {
                format!("gh:{} (preview pending)", repo_identifier(repo))
            }
        };
        Some(HostText::from_host(words))
    }

    /// The name field's placeholder: the folder name an unnamed managed checkout gets, once previewed (SPEC.md:
    /// "Whenever a checkout launch is unnamed, the name field shows the `repo-N` it will get as placeholder text").
    pub(super) fn checkout_name_placeholder(&self, cx: &App) -> String {
        if self.draft.checkout.is_none() || !self.draft.checkout_title().is_empty() {
            return String::new();
        }
        match self.checkout_preview(cx) {
            Preview::Ready(preview) => preview.binding.basename.clone(),
            Preview::Waiting | Preview::Failed => String::new(),
        }
    }
}

/// What the tests read of the destination's own state, which nothing outside this module touches otherwise.
#[cfg(test)]
impl Launcher {
    /// The read-only folder field's text while a checkout is chosen.
    pub(crate) fn checkout_path_text(&self, cx: &App) -> String {
        self.checkout.path.text(cx).to_string()
    }

    /// The read-only folder field's placeholder.
    pub(crate) fn checkout_path_placeholder(&self) -> &'static str {
        self.checkout.path_placeholder
    }

    /// The repository field's text.
    pub(crate) fn repository_field_text(&self, cx: &App) -> String {
        self.checkout.repository.text(cx).to_string()
    }

    /// The name field's placeholder as last set for a managed checkout.
    pub(crate) fn checkout_name_placeholder_shown(&self) -> String {
        self.checkout.name_placeholder.clone()
    }

    /// Whether the repository field holds focus.
    pub(crate) fn repository_focused(&self, window: &Window, cx: &App) -> bool {
        self.checkout.repository.focus_handle(cx).is_focused(window)
    }

    /// Whether the read-only checkout folder field holds focus.
    pub(crate) fn checkout_folder_focused(&self, window: &Window, cx: &App) -> bool {
        self.checkout.path.focus_handle(cx).is_focused(window)
    }
}

/// The open suggestions list's rows, drawn from the launcher's state and kept scrolled to the highlighted row, as the
/// model list's are (`model_field`).
pub(super) struct RepositoryRows {
    launcher: WeakEntity<Launcher>,
    scroll: ScrollHandle,
    scrolled_to: Option<usize>,
    _redraw: Option<Subscription>,
}

impl RepositoryRows {
    fn new(launcher: WeakEntity<Launcher>, cx: &mut Context<Self>) -> Self {
        let redraw = launcher
            .upgrade()
            .map(|entity| cx.observe(&entity, |_, _, cx| cx.notify()));
        RepositoryRows {
            launcher,
            scroll: ScrollHandle::new(),
            scrolled_to: None,
            _redraw: redraw,
        }
    }
}

impl Render for RepositoryRows {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let list = layers::surface(cx)
            .id("launcher-repository-list")
            .debug_selector(|| "launcher-repository-list".into())
            .aria_label("Repositories")
            .flex()
            .flex_col()
            .gap(px(LIST_GAP))
            .p(px(LIST_PAD))
            .max_h(px(LIST_MAX_HEIGHT))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .text_size(metrics::TEXT_MD);
        let Some(launcher) = self.launcher.upgrade() else {
            return list;
        };
        let launcher_ref = launcher.read(cx);
        let rows = launcher_ref.repository_rows(window, cx);
        if rows.is_empty() {
            // The old list became a status saying what to type, a warning once something typed matches nothing.
            let typed = !launcher_ref.checkout.repository.text(cx).trim().is_empty();
            let (words, color) = if typed {
                (NO_MATCH, palette.warn)
            } else {
                (TYPE_A_PAIR, palette.fg_1)
            };
            return list.role(Role::Status).child(
                div()
                    .id("launcher-repository-empty")
                    .role(Role::Label)
                    .aria_label(SharedString::from(words))
                    .px(px(8.))
                    .py(px(4.))
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(color))
                    .child(words),
            );
        }
        let active = launcher_ref.checkout.active;
        let mut children = Vec::with_capacity(rows.len());
        for (index, repo) in rows.into_iter().enumerate() {
            let identifier = repo_identifier(&repo);
            let id = format!("launcher-repository-option-{identifier}");
            let words = HostSentence::new(SharedString::from(format!("{id}-words")))
                .host(&HostText::from_host(identifier.as_str()));
            let help = format!("Managed checkout of {}", words.accessible_name());
            let weak = self.launcher.clone();
            children.push(
                OptionRow::new(
                    id,
                    words.accessible_name(),
                    HoverHelp::tooltip(help),
                    Role::ListBoxOption,
                )
                .selected(active == Some(index))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        // The highlighted row's words and glyph take the fill's ink, as the model list's do.
                        .text_color(theme::color(if active == Some(index) {
                            palette.on_fill
                        } else {
                            palette.fg_0
                        }))
                        .child(
                            gpui::svg()
                                .path(Icon::Branch.path())
                                .size(px(12.))
                                .flex_none()
                                .text_color(theme::color(if active == Some(index) {
                                    palette.on_fill
                                } else {
                                    palette.fg_1
                                })),
                        )
                        .child(words),
                )
                .on_click(move |_, window, cx| {
                    let repo = repo.clone();
                    let _ = weak.update(cx, |launcher, cx| {
                        launcher.pick_repository(repo, window, cx)
                    });
                })
                .into_any_element(),
            );
        }
        if active != self.scrolled_to {
            if let Some(index) = active {
                self.scroll.scroll_to_item(index);
            }
            self.scrolled_to = active;
        }
        list.role(Role::ListBox).children(children)
    }
}
