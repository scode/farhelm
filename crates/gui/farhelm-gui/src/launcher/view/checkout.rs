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
//!   "The launcher's requests debounce for 150 ms and discard stale host/query generations"). The list floats under the
//!   field on the layer manager's non-focusing popover, as the model field's does (`field_list`): a valid `owner/repo`
//!   typed into the field is always its first row, so an incomplete scan never stops a choice (SPEC.md: "Incomplete
//!   discovery is visible and does not prevent selecting a valid manually entered pair"), and an incomplete scan says
//!   so under the field. Enter picks the highlighted row and never launches. While the composer search holds a `gh:`
//!   query, that query is what the read asks about instead, with or without a managed checkout chosen, and the search
//!   offers the same choices ([`Launcher::search_repositories`]); the old launcher's one discovery read served both the
//!   same way ("global search takes priority only while the user explicitly searches with gh:").
//! - **The preview** (`POST /api/github-checkout-preview`) is asked for whenever what it answers for changes: the host,
//!   its connection and installation, the repository (or choosing it again), the checkout's name, the agent choices and
//!   the configuration revision last observed (SPEC.md: "Host, installation, destination, title, agent choice and
//!   observed configuration changes invalidate an undispatched preview"). Launch waits for it
//!   ([`super::super::request::Preview`]).
//! - **Latest wins, one read at a time**, for both reads, as the folder check does (`latest`): an answer counts only
//!   while the launcher still asks what it was asked, so a late answer can neither bring back an old destination nor
//!   move focus ("Late responses cannot restore its authority or steal focus"); and a read is not sent while another of
//!   its kind is out, the next going out when that answers. The old launcher sent a preview per change; one at a time
//!   keeps a name typed key by key from queuing a preview per key behind each other on the ordered channel.
//! - **The configuration revision.** The helm's checkout configuration (the folder a host's checkouts go in) changes
//!   from Settings or the command line, and a preview taken before the change is stale. The launcher learns the
//!   current revision from its launch-history read ([`Launcher::observe_checkout_config_revision`]); a preview is
//!   asked again when it rises, and an answer from before the last observed revision is never accepted.
//! - **Names.** The name a checkout's folder is named after is the draft's ([`LaunchDraft::checkout_title`]); a
//!   numbered default (Clone's `<title>-clone`, a template's base name) whose preview the helm refuses as taken moves on
//!   to its next candidate ([`LaunchDraft::advance_checkout_name`]), which is previewed in turn. The supervisor says
//!   "taken" in fixed words, which this matches, as the folder check matches the folder words: matching only proposes
//!   another name, which still takes the user's press of Launch; a quick launch never shown follows its own base name's
//!   numbering instead (the last bullet).
//! - **A lost race.** A create the helm proves it never accepted (another create took the previewed folder, or the
//!   preview went stale) comes back with "nothing was accepted" after the helm's words, and a new preview is asked for;
//!   nothing is sent again until the user presses Launch (SPEC.md: "it never submits automatically or silently chooses
//!   another directory").
//! - **No checkout folder.** When the host has no checkout folder in effect (the suggestions read says so), the shared
//!   explicit-save field for the default folder shows above the repository field (`settings::checkout_field`); a save
//!   reads the suggestions and the preview again without reopening the launcher (SPEC.md, "Managed checkouts").
//! - **Launches that wait for the preview.** The YOLO question's "Start guarded session instead" changes the
//!   permission, which makes the preview stale, and a quick launch (a launch template's shortcut, started without
//!   showing the launcher) has not asked for its preview yet. Instead of refusing, either waits for the preview and
//!   starts once it is ready, as the old launcher's guarded submit and quick launch did; any edit, a close, a failed
//!   preview or another preview cancels the wait, and a quick launch that does not start is shown
//!   ([`Launcher::submit_or_wait_for_preview`]). The one exception is a quick launch's own template base name taken:
//!   it waits for the next numbered candidate's preview instead, out of sight ([`Launcher::settle_waiting_launch`]).

use std::time::Duration;

use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::session_requests::{
    GithubCheckoutPreviewBody, GithubRepositoriesBody, GithubRepositoriesReply,
};
use farhelm_proto::ui_channel::{HostStateView, HostView};
use farhelm_proto::{AcceptedGithubPreview, GithubRepo, parse_github_repo};
use gpui::{
    AnyElement, AnyWindowHandle, App, AppContext as _, Context, Div, Entity,
    InteractiveElement as _, IntoElement, ParentElement as _, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity, Window, div, px,
};

use super::field_list::{Drawn, FieldList};
use super::latest::Latest;
use super::{LABEL_COLUMN, LABEL_GAP, Launcher, choices::status_line, help_line, row};
use crate::assets::Icon;
use crate::components::{
    HoverHelp, OptionRow, RadioGroup, RadioOption, Role, TextInputEvent, TextInputState, Vertical,
};
use crate::focus::Mark;
use crate::host_text::{HostSentence, HostText};
use crate::launcher::draft::{
    AgentChoices, CommandChoices, DestinationKind, Field, Tab, repo_identifier,
};
use crate::launcher::request::Preview;
use crate::launcher::request::WAIT_FOR_PREVIEW;
use crate::launcher::search::{Scope, scoped};
use crate::settings::{CheckoutField, FolderScope, Saved};
use crate::theme::{self, metrics};
use crate::{operations, ticker};

/// How long the query (the repository field's, or the search's `gh:` text), host and connection must stay unchanged
/// before suggestions are read.
const SUGGEST_AFTER: Duration = Duration::from_millis(150);
/// The most rows the suggestions list offers, the old `repository_choices` cap.
const MAX_CHOICES: usize = 100;
/// The suggestions list's element id (`.launch-composer-repository-options`), which tests find it by.
const REPOSITORY_LIST: &str = "launcher-repository-list";

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
/// The supervisor's words refusing a checkout name that is taken (its `NameError::Occupied`), found anywhere in the
/// refusal since the helm passes them on whole.
const OCCUPIED: &str = "a directory with that checkout name already exists";
/// What a create the helm proves it never accepted adds after the helm's words, the old launcher's.
pub(super) const NOTHING_ACCEPTED: &str =
    "; nothing was accepted; review the refreshed preview and press Launch again";
/// The note under the inline checkout-folder field, the old launcher's.
const SETUP_NOTE: &str = "Sets the default checkout folder for every host without its own. ~ means each host's home \
                          folder. Change the default later in Settings → Checkouts, or set a host's own folder in its \
                          settings.";
const STALE_PREVIEW: &str =
    "The preview is stale or belongs to a different host installation; select the repository again";

// ---------------------------------------------------------------------------------------------------------------------
// What the reads ask, one at a time, latest wins (`super::latest`)
// ---------------------------------------------------------------------------------------------------------------------

/// What a suggestions read asks: on which host connection and installation, for which typed text.
#[derive(Clone, Debug, PartialEq, Eq)]
struct SuggestKey {
    host: i64,
    incarnation: u64,
    identity: String,
    query: String,
    /// How many times the suggestions were asked for again (after the inline setup saved a folder), so a save reads
    /// them again even for the same text.
    reread: u64,
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
    /// How many times the preview was asked for again without the draft changing (a lost race, a saved checkout
    /// folder), so it is asked again.
    retry: u64,
}

impl PreviewKey {
    /// Whether this key is `earlier` with only the name changed: what the taken-name rule's move to the next numbered
    /// candidate does, and nothing else (a host, repository, revision or agent change differs elsewhere too).
    fn renamed_from(&self, earlier: &PreviewKey) -> bool {
        self.title != earlier.title
            && PreviewKey {
                title: earlier.title.clone(),
                ..self.clone()
            } == *earlier
    }
}

/// A launch waiting for its managed checkout's preview ([`Launcher::submit_or_wait_for_preview`]): the mark it counts
/// from, the preview it waits for now, the window it starts in, and whether it is a quick launch never shown (which
/// stays out of sight while it waits, and is shown if it does not start). It waits for one preview at a time: the one
/// due when it was armed, or, for a quick launch never shown whose base name was taken, its next candidate's
/// ([`Launcher::settle_waiting_launch`]).
struct Armed {
    mark: Mark,
    key: PreviewKey,
    window: AnyWindowHandle,
    unseen: bool,
}

/// A host connection and installation, which whether a host has a checkout folder is known for.
#[derive(Clone, Debug, PartialEq, Eq)]
struct HostKey {
    host: i64,
    incarnation: u64,
    identity: String,
}

/// The helm's answer to a preview: the folder, or its refusal in its words.
type PreviewAnswer = Result<AcceptedGithubPreview, HostText>;

/// The suggestions read's answer, or why there is none.
type SuggestAnswer = Result<GithubRepositoriesReply, HostText>;

// ---------------------------------------------------------------------------------------------------------------------
// The destination's state
// ---------------------------------------------------------------------------------------------------------------------

/// What the managed-checkout destination holds besides the draft: its two fields, the suggestions list, the reads, and
/// the configuration revision observed. None of it is shared with any other surface (the composer search's `gh:` rows
/// read the same suggestions, within the launcher), and none of it survives the launcher.
pub(super) struct CheckoutState {
    /// The repository field: the chosen repository, or what is typed.
    pub(super) repository: TextInputState,
    /// The read-only folder field shown in place of the folder while a managed checkout is chosen: the preview's
    /// path, escaped, or empty with a placeholder saying why there is none.
    path: TextInputState,
    /// The suggestions list under the repository field, with the row moved to with the arrows.
    list: FieldList,
    suggestions: Latest<SuggestKey, SuggestAnswer>,
    preview: Latest<PreviewKey, PreviewAnswer>,
    /// The highest checkout configuration revision observed, which never goes back down; `None` until one is.
    revision: Option<i64>,
    /// The placeholders last given to the read-only folder field and the name field, so a frame that changes neither
    /// leaves the fields alone (setting one redraws its field).
    path_placeholder: &'static str,
    pub(super) name_placeholder: String,
    /// See [`SuggestKey::reread`] and [`PreviewKey::retry`].
    reread: u64,
    retry: u64,
    /// Whether the host, as of the last suggestions answer about it, needs a checkout folder set up (the reply's
    /// `needs_checkout_root`, the helm's reading of the host's effective configuration). Kept across a re-read, so the
    /// setup field stays put, with its draft and focus, while a save is checked.
    needs_root: Option<(HostKey, bool)>,
    /// Set by a save in the setup field until the next suggestions answer: whether the host needs a folder is to be
    /// asked again, even with a repository chosen.
    root_recheck: bool,
    /// Set by a save in the setup field, to the save's moment: if the re-check then finds the host no longer needs a
    /// folder and the field goes, focus moves on to the repository field (the next thing to fill in) unless the user has
    /// moved on since. Without it, focus would fall to whatever the focus coordinator recovers to, rows further down.
    focus_after_setup: Option<Mark>,
    /// The inline setup field, made the first time it shows and kept while the launcher lives, with its subscription.
    setup: Option<(Entity<CheckoutField>, Subscription)>,
    /// The guarded answer's launch, waiting for the preview: the answer's mark, which the launch's moved-on check counts
    /// from ([`Launcher::submit_or_wait_for_preview`]).
    armed: Option<Armed>,
    /// The preview whose "taken" answer last moved a numbered name on to its next candidate (the taken-name rule, and
    /// nothing else), so a hidden quick launch waiting for that preview can follow its own name's numbering rather than
    /// give up ([`Launcher::settle_waiting_launch`]). Set on every such advance, a shown launcher's included, and cleared
    /// only when a waiting launch follows it; a stale value is harmless, since it counts only when it equals the
    /// waiting launch's own key.
    advanced_from: Option<PreviewKey>,
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
                launcher.checkout.list.close(window, cx);
                cx.notify();
            });
        });
        CheckoutState {
            repository,
            path,
            list: FieldList::default(),
            suggestions: Latest::default(),
            preview: Latest::default(),
            revision: None,
            path_placeholder: "",
            name_placeholder: String::new(),
            reread: 0,
            retry: 0,
            needs_root: None,
            root_recheck: false,
            focus_after_setup: None,
            setup: None,
            armed: None,
            advanced_from: None,
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
    /// either read can ask about, since the helm binds both to that identity. None while a new environment is chosen:
    /// its checkout's folder is previewed by the helm once the environment's Farhelm connects, and the host's checkout
    /// folder setup is not its concern (SPEC.md, "Managed checkouts": a recipe-target launch "is another exception").
    fn checkout_host(&self, cx: &App) -> Option<HostView> {
        if self.draft.target.is_some() {
            return None;
        }
        self.connected_host(cx)
    }

    /// The chosen host as the store holds it now, while it is connected with an installation identity. The repository
    /// suggestions ask it whatever the placement, since a new environment's repository is chosen the same way.
    fn connected_host(&self, cx: &App) -> Option<HostView> {
        let view = self.chosen_host(cx)?.clone();
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
            || self.checkout.list.is_open(window, cx)
    }

    /// The chosen host's connection and installation, while it can be asked.
    fn checkout_host_key(&self, cx: &App) -> Option<HostKey> {
        let host = self.checkout_host(cx)?;
        Some(HostKey {
            host: host.id,
            incarnation: host.incarnation,
            identity: host.identity.unwrap_or_default(),
        })
    }

    /// Whether the chosen host needs a checkout folder set up, as last answered about it; `None` while unknown.
    fn needs_root(&self, cx: &App) -> Option<bool> {
        let host = self.checkout_host_key(cx)?;
        self.checkout
            .needs_root
            .as_ref()
            .filter(|(known, _)| *known == host)
            .map(|(_, needs)| *needs)
    }

    /// The `gh:` query the composer search holds, while it holds one.
    fn search_repository_query(&self, cx: &App) -> Option<String> {
        let text = self.search.text(cx);
        let (scope, value) = scoped(&text);
        (scope == Scope::Github).then(|| value.to_owned())
    }

    /// A suggestions read about `query` on the chosen host, while it can be asked.
    fn suggest_key_for(&self, query: String, cx: &App) -> Option<SuggestKey> {
        let host = self.connected_host(cx)?;
        Some(SuggestKey {
            host: host.id,
            incarnation: host.incarnation,
            identity: host.identity.unwrap_or_default(),
            query,
            reread: self.checkout.reread,
        })
    }

    /// The repositories the suggestions answer lists, while it is about `key`; none otherwise.
    fn discovered(&self, key: Option<SuggestKey>) -> &[GithubRepo] {
        key.and_then(|key| self.checkout.suggestions.answer_for(&key))
            .and_then(|answer| answer.as_ref().ok())
            .map_or(&[], |reply| reply.repos.as_slice())
    }

    /// The search's rows for its `gh:` query: a valid typed pair first, then the host's discovered repositories that
    /// match, from the suggestions answer while it is about this host and query (the module docs); empty for any other
    /// query.
    pub(super) fn search_repositories(&self, cx: &App) -> Vec<GithubRepo> {
        let Some(query) = self.search_repository_query(cx) else {
            return Vec::new();
        };
        repository_choices(
            &query,
            self.discovered(self.suggest_key_for(query.clone(), cx)),
        )
    }

    /// What a suggestions read would ask now: the search's `gh:` query while there is one; otherwise, while the user is
    /// choosing, and also, with a repository chosen, once per host while whether it needs a checkout folder is not known
    /// (so a Clone's checkout still learns it), on a host that can be asked.
    fn suggest_key(&self, window: &Window, cx: &App) -> Option<SuggestKey> {
        if let Some(query) = self.search_repository_query(cx) {
            return self.suggest_key_for(query, cx);
        }
        let checkout = self.draft.checkout.as_ref()?;
        if !self.choosing_repository(window, cx)
            && self.needs_root(cx).is_some()
            && !self.checkout.root_recheck
        {
            return None;
        }
        self.suggest_key_for(checkout.field_text(), cx)
    }

    /// Whether the composer search holds a `gh:` query, which owns the repository suggestions and their note while it
    /// is there (the module docs), and makes Launch wait: until a repository is picked (the query then goes), the launch
    /// would quietly use a destination the user is replacing. SPEC.md, "Managed checkouts": "Invalid
    /// `gh:` input cannot launch a hidden existing directory"; the old launcher's "Select a GitHub repository before
    /// launching", after the host's own reasons.
    pub(super) fn search_holds_repository_query(&self, cx: &App) -> bool {
        self.search_repository_query(cx).is_some()
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
            retry: self.checkout.retry,
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
            let deadline = ticker::after(SUGGEST_AFTER, cx, move |cx| {
                let _ = launcher.update(cx, |launcher, cx| launcher.send_suggestions(key, cx));
            });
            self.checkout.suggestions.wait(deadline);
        }
        self.follow_preview(cx);
        self.show_checkout_path(window, cx);
        // The name field follows a name the draft changed on its own: a numbered default moving on, or the copied title
        // ignored or shown again as the destination changes. Typing changes the draft to the field's text, so this
        // never fights the user.
        let shown = self.draft.shown_name();
        if self.name.text(cx).as_ref() != shown {
            self.name.set_text(shown, window, cx);
        }
    }

    /// Ask for the preview as soon as what it answers for changes (one at a time; the module docs). Part of following
    /// the destination while drawing, and called on its own for a quick launch, which is never drawn.
    fn follow_preview(&mut self, cx: &mut Context<Self>) {
        let key = self.preview_key(cx);
        if let Some(key) = self.checkout.preview.follow(key) {
            self.send_preview(key, cx);
        }
    }

    /// A launch to start once its managed checkout's preview is ready: the YOLO question's guarded answer, whose
    /// switch to Guarded made the preview stale, and a quick launch prepared out of sight (`prepare_unseen`), whose
    /// preview has not been asked for yet. When the first thing the launch waits for is that preview, it waits for it
    /// ([`Self::settle_waiting_launch`]) and returns `true`; otherwise it is submitted now, refused as a press would be,
    /// and this returns `false`. A quick launch waits out of sight, as the old app's did ("quick launch waits for a
    /// managed preview and starts directly").
    pub(in crate::launcher) fn submit_or_wait_for_preview(
        &mut self,
        mark: Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        // A quick launch's launcher is never drawn, so its preview is asked for here rather than while drawing.
        self.follow_preview(cx);
        // Compared with the launcher's own constant, never with helm words: a reworded reason must reword the constant.
        if self.refusal(cx).as_deref() == Some(WAIT_FOR_PREVIEW)
            && self.checkout_preview(cx) == Preview::Waiting
            && let Some(key) = self.preview_key(cx)
        {
            self.checkout.armed = Some(Armed {
                mark,
                key,
                window: window.window_handle(),
                unseen: self.quick_launch_unseen,
            });
            cx.notify();
            return true;
        }
        self.submit(false, false, mark, window, cx);
        false
    }

    /// A preview answered: settle the launch waiting for one, if there is one. It waits for one preview only, the one
    /// due when it was armed; anything else that asked for a new preview meanwhile (the host reconnecting, a numbered
    /// name moving on after a taken folder, a newer configuration revision, a save in the inline setup) drops it, since
    /// its folder is one the user has not seen yet (SPEC.md: Launch "never submits automatically or silently chooses
    /// another directory"), as does a failed preview. A ready one starts it with its mark, if its launcher is still
    /// showing or was never shown. A quick launch that does not start comes back to be seen, with the failed preview's
    /// line or the new preview, as any quick launch that did not start does. The old launcher's guarded submit compared
    /// the whole preview identity the same way.
    ///
    /// One exception: a quick launch never shown follows its own template base name's numbering. When the preview it
    /// waits for came back taken and the taken-name rule moved the name on to its next candidate, with nothing else
    /// changed, it waits for that candidate's preview instead (SPEC.md, "Managed checkouts": a shortcut "obtains the
    /// same exact preview and launches without showing the path first", and a base name "uses its first free
    /// candidate"; the old app's "quick launch numbers a template base name when its checkout is taken"). The user
    /// pressed the shortcut for exactly that numbering. A shown launcher's wait (the YOLO question's guarded answer)
    /// still gives up, since the user sees the name change and presses Launch again for the new folder.
    fn settle_waiting_launch(launcher: &WeakEntity<Launcher>, cx: &mut App) {
        let Some(entity) = launcher.upgrade() else {
            return;
        };
        let settled = entity.update(cx, |launcher, cx| {
            let armed = launcher.checkout.armed.as_ref()?;
            let renamed = launcher.preview_key(cx).filter(|key| {
                armed.unseen
                    && launcher.checkout.advanced_from.as_ref() == Some(&armed.key)
                    && key.renamed_from(&armed.key)
            });
            if let Some(next) = renamed {
                // Never drawn, so its next preview is asked for here, as when it was armed.
                launcher.checkout.advanced_from = None;
                launcher.follow_preview(cx);
                if let Some(armed) = launcher.checkout.armed.as_mut() {
                    armed.key = next;
                }
                return None;
            }
            let armed = launcher.checkout.armed.as_ref()?;
            let start = if launcher.preview_key(cx).as_ref() != Some(&armed.key) {
                false
            } else {
                match launcher.checkout_preview(cx) {
                    Preview::Waiting => return None,
                    Preview::Failed => false,
                    Preview::Ready(_) => true,
                }
            };
            let armed = launcher.checkout.armed.take()?;
            let start = start && (armed.unseen || launcher.layer.is_some());
            Some((start, armed))
        });
        let Some((start, armed)) = settled else {
            return;
        };
        if start {
            let _ = cx.update_window(armed.window, |_, window, cx| {
                entity.update(cx, |launcher, cx| {
                    launcher.submit(false, false, armed.mark, window, cx)
                });
            });
        }
        // A launch that went out moved the quick launch's launcher to waiting for its reply; one that did not comes
        // back to be seen.
        if armed.unseen {
            super::super::launch::reveal_prepared(entity, armed.window, armed.mark, cx);
        }
    }

    /// Cancel the guarded answer's waiting launch: the draft changed, so it would launch something else than was
    /// answered.
    pub(super) fn disarm_checkout_launch(&mut self) {
        self.checkout.armed = None;
    }

    /// This launcher's create came back refused ([`super::super::launch::RefusedLaunch`]): if the helm proved it never
    /// accepted a managed checkout's create, ask for a new preview (see the module docs).
    pub(super) fn checkout_refused(&mut self, unaccepted: bool) {
        let checkout = self.draft.checkout.as_ref();
        if unaccepted && checkout.is_some_and(|checkout| checkout.repo.is_some()) {
            self.checkout.retry += 1;
        }
    }

    /// What follows the helm's words in the refusal alert: "nothing was accepted" for a managed checkout's create the
    /// helm proved it never accepted, else nothing.
    pub(super) fn refusal_suffix(&self) -> &'static str {
        let unaccepted = self
            .refused
            .as_ref()
            .is_some_and(|refused| refused.unaccepted);
        let checkout = self
            .draft
            .checkout
            .as_ref()
            .is_some_and(|checkout| checkout.repo.is_some());
        if unaccepted && checkout {
            NOTHING_ACCEPTED
        } else {
            ""
        }
    }

    /// Read the suggestions for `key`, if it is still what the launcher would ask and no read is out (then the
    /// answer sends it).
    fn send_suggestions(&mut self, key: SuggestKey, cx: &mut Context<Self>) {
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
        if !self.checkout.suggestions.begin(&key) {
            return;
        }
        let launcher = cx.weak_entity();
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
                )
                .and_then(|reply| {
                    // The helm names the installation that answered; one other than the launcher asked about is no
                    // answer about this host (the old `RepositoryAuthority::accepts`).
                    let same = reply.host == key.host.to_string()
                        && reply.incarnation == key.incarnation
                        && reply.installation_identity == key.identity;
                    if same {
                        Ok(reply)
                    } else {
                        Err(HostText::from_host(OTHER_INSTALLATION))
                    }
                });
                let needs_root = answer.as_ref().ok().map(|reply| reply.needs_checkout_root);
                let _ = launcher.update(cx, |launcher, cx| {
                    let finished = launcher.checkout.suggestions.finish(&key, Some(answer));
                    if finished.kept {
                        if let Some(needs_root) = needs_root {
                            let host = HostKey {
                                host: key.host,
                                incarnation: key.incarnation,
                                identity: key.identity.clone(),
                            };
                            launcher.checkout.needs_root = Some((host, needs_root));
                            launcher.checkout.root_recheck = false;
                            // The field stays: the save did not give the host a folder, and focus stays in it.
                            if needs_root {
                                launcher.checkout.focus_after_setup = None;
                            }
                        }
                        cx.notify();
                    }
                    if let Some(now) = finished.next {
                        launcher.send_suggestions(now, cx);
                    }
                });
            },
        );
        if !sent {
            self.checkout.suggestions.unsent();
        }
    }

    /// Ask for the preview `key` answers for, if it is still current and no preview is out (then the answer sends
    /// it). The preview is a read: it creates and reserves nothing (SPEC.md: "Preview creates no directory and reserves
    /// nothing").
    fn send_preview(&mut self, key: PreviewKey, cx: &mut Context<Self>) {
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
        if !self.checkout.preview.begin(&key) {
            return;
        }
        let launcher = cx.weak_entity();
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
                )
                .and_then(|preview| {
                    if preview_matches(&key, &preview) {
                        Ok(preview)
                    } else {
                        Err(HostText::from_host(STALE_PREVIEW))
                    }
                });
                let occupied = answer
                    .as_ref()
                    .is_err_and(|words| words.raw().contains(OCCUPIED));
                let _ = launcher.update(cx, |launcher, cx| {
                    let finished = launcher.checkout.preview.finish(&key, Some(answer));
                    if finished.kept {
                        // A numbered default name that is taken moves on; its next candidate is then previewed, since
                        // the name is part of what the preview answers for.
                        if occupied && launcher.draft.advance_checkout_name(&key.title) {
                            launcher.checkout.advanced_from = Some(key.clone());
                        }
                        cx.notify();
                    }
                    if let Some(now) = finished.next {
                        launcher.send_preview(now, cx);
                    }
                });
                // A launch waiting for this preview starts, or gives up, now (it is not drawn if it is a quick launch).
                Launcher::settle_waiting_launch(&launcher, cx);
            },
        );
        if !sent {
            self.checkout.preview.unsent();
        }
    }

    /// The helm's launch-history read observed the checkout configuration at `revision` (its
    /// `checkout_config_revision`): a preview taken under an older revision may name a folder the host's checkouts no
    /// longer go in, so a higher revision than any observed asks for the preview again, and an answer from before it is
    /// never accepted. A lower or equal one is only noted. The revision never goes back down: an older read answering
    /// late cannot undo a newer one.
    ///
    /// Called by the launcher's launch-history read (`history`), which brings the revision with every answer.
    pub(crate) fn observe_checkout_config_revision(
        &mut self,
        revision: i64,
        cx: &mut Context<Self>,
    ) {
        if self.checkout.revision.is_some_and(|seen| seen >= revision) {
            return;
        }
        self.checkout.revision = Some(revision);
        // The checkout folder setting may be what changed: ask again whether the host still needs one set up.
        self.checkout.root_recheck = true;
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
    /// The list's rows for the field as it is now, from the suggestions answer while it is about this host and text.
    fn repository_rows(&self, window: &Window, cx: &App) -> Vec<GithubRepo> {
        let query = self.checkout.repository.text(cx).to_string();
        repository_choices(&query, self.discovered(self.suggest_key(window, cx)))
    }

    /// The user typed in the repository field: the draft's query, the highlight back to none, and the list open.
    fn repository_typed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.checkout.repository.text(cx).to_string();
        self.draft.edit_repository_query(text);
        self.checkout.list.highlighted = None;
        self.open_repository_list(window, cx);
        self.changed(cx);
    }

    /// Up or Down in the field: move the highlighted row, wrapping, while the list is open and has rows.
    fn repository_arrow(&mut self, arrow: Vertical, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.repository_rows(window, cx).len();
        if self.checkout.list.is_open(window, cx) && self.checkout.list.arrow(arrow, count) {
            cx.notify();
        }
    }

    /// Enter in the field: pick the highlighted row (or, with none highlighted, the first) while the list is open, or
    /// open it. Never launches: in the old field, choosing a repository and launching were separate keys.
    fn repository_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.checkout.list.is_open(window, cx) {
            self.open_repository_list(window, cx);
            cx.notify();
            return;
        }
        let rows = self.repository_rows(window, cx);
        let highlighted = self.checkout.list.highlighted.unwrap_or(0);
        if let Some(repo) = rows.get(highlighted).cloned() {
            self.pick_repository(repo, window, cx);
        }
    }

    /// A row picked by a click or Enter, here or as the search's `gh:` row: the draft's repository, the field showing
    /// it, the list closed, and a `gh:` query in the search box gone, since the pick answers it (the old launcher's
    /// "a deliberate field selection retires the older global gh: query").
    pub(super) fn pick_repository(
        &mut self,
        repo: GithubRepo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft.choose_repository(repo);
        self.checkout.list.close(window, cx);
        if self.search_holds_repository_query(cx) {
            self.search.set_text("", window, cx);
        }
        self.sync_fields(window, cx);
        self.changed(cx);
    }

    /// Open the list under the field with nothing highlighted, unless it is open already. Not while the field is
    /// read-only (the mode holds the destination).
    fn open_repository_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.draft.holds(Field::Destination) || self.checkout.list.is_open(window, cx) {
            return;
        }
        self.checkout.list.highlighted = None;
        self.checkout
            .list
            .open(REPOSITORY_LIST, draw_repository_rows, window, cx);
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
        self.checkout.list.close(window, cx);
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
            (Some(_), _) if self.draft.target.is_some() => {
                (String::new(), "Cloned inside the new environment")
            }
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
    /// The Where row: Folder or Managed checkout ([`destination_choice`]). Not drawn where the mode holds the
    /// destination.
    pub(super) fn where_row(
        &self,
        palette: &theme::Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if self.draft.holds(Field::Destination) {
            return None;
        }
        let weak = cx.weak_entity();
        let group = destination_choice(
            "launcher-destination",
            self.draft.destination_kind(),
            HoverHelp::tooltip(
                "Where: an existing folder, or a new managed checkout of a repository",
            ),
            move |kind, window, cx| {
                let _ = weak.update(cx, |launcher, cx| {
                    launcher.choose_destination(kind, window, cx)
                });
            },
        );
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
        self.checkout.list.follow(window, cx);
        let Some(checkout) = self.draft.checkout.clone() else {
            return Vec::new();
        };
        let tracker = self
            .checkout
            .list
            .track_anchor("launcher-repository-anchor", window, cx);
        let weak = cx.weak_entity();
        let field = self
            .launcher_input(
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
        let mut rows = Vec::new();
        // The old order: the setup field above the repository field, which it is the precondition for.
        let setup = self.setup_field(window, cx);
        let setup_shown = setup.is_some();
        rows.extend(setup);
        rows.push(
            row("launcher-repository-row", "Repository", control, palette).into_any_element(),
        );
        // The setup field says what the note would (no checkout folder), so the note waits while it shows; while the
        // search holds a `gh:` query the note is about that query, and is drawn under the search box instead.
        if self.choosing_repository(window, cx)
            && !setup_shown
            && !self.search_holds_repository_query(cx)
            && let Some(note) = self.repository_note("launcher-repository-note", window, cx)
        {
            rows.push(note.into_any_element());
        }
        if let Some(repo) = &checkout.repo {
            rows.push(self.preview_block(repo, palette, cx));
        }
        rows.push(help_line(EXPLANATION, palette).into_any_element());
        rows
    }

    /// The inline checkout-folder field, while the chosen host needs a checkout folder set up: the shared explicit-save
    /// field over the default folder (SPEC.md: "When the selected host has no checkout folder in effect, the launcher's
    /// managed-checkout destination offers the same explicit-save field"), in the control column. Made the first time it
    /// shows; a save it reports asks for the suggestions and the preview again.
    fn setup_field(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.needs_root(cx) != Some(true) {
            // The field just went after a save: focus moves on to the repository field (see `focus_after_setup`).
            if let Some(mark) = self.checkout.focus_after_setup.take() {
                let handle = self.checkout.repository.focus_handle(cx);
                cx.defer_in(window, move |_, window, cx| {
                    crate::focus::focus_in_layer(&handle, &mark, window, cx)
                });
            }
            return None;
        }
        if self.checkout.setup.is_none() {
            let field =
                cx.new(|cx| CheckoutField::folder(FolderScope::Default, SETUP_NOTE, window, cx));
            let saved = cx.subscribe(&field, |launcher: &mut Launcher, _, _: &Saved, cx| {
                launcher.checkout.reread += 1;
                launcher.checkout.retry += 1;
                launcher.checkout.root_recheck = true;
                launcher.checkout.focus_after_setup = Some(Mark::take(cx));
                cx.notify();
            });
            self.checkout.setup = Some((field, saved));
        }
        let (field, _) = self.checkout.setup.as_ref()?;
        Some(
            div()
                .debug_selector(|| "launcher-checkout-setup".into())
                .pl(px(LABEL_COLUMN + LABEL_GAP))
                .child(field.clone())
                .into_any_element(),
        )
    }

    /// [`Self::repository_note`] for the search's `gh:` query, drawn under the search box, where the old launcher drew
    /// it, so a Folder destination (which has no repository field) still shows an incomplete or failed scan (SPEC.md,
    /// "Managed checkouts": "Incomplete discovery is visible"). Not while the checkout-folder setup shows above the
    /// repository field, which says the same thing.
    pub(super) fn search_repository_note(&self, window: &Window, cx: &App) -> Option<AnyElement> {
        if !self.search_holds_repository_query(cx)
            || (self.draft.checkout.is_some() && self.needs_root(cx) == Some(true))
        {
            return None;
        }
        // The search box spans the dialog, so its note starts at the box's edge rather than the fields' column.
        self.repository_note("launcher-search-repository-note", window, cx)
            .map(|note| note.pl_0().into_any_element())
    }

    /// The note on an incomplete scan, or a failed read (`.launch-composer-repository-note`), under the repository
    /// field or, for a `gh:` query, the search box: the helm's reason in the warning color, or, for a scan it cut short
    /// without one, a quiet hint that a typed pair can still be chosen.
    fn repository_note(
        &self,
        id: &'static str,
        window: &Window,
        cx: &App,
    ) -> Option<Stateful<Div>> {
        let key = self.suggest_key(window, cx)?;
        let palette = theme::palette(cx);
        match self.checkout.suggestions.answer_for(&key)? {
            Ok(reply) => match &reply.scan_error {
                Some(error) => Some(status_line(id, error.clone(), palette)),
                // A quiet hint, but announced as the other notes are.
                None if reply.truncated => Some(help_line(INCOMPLETE, palette).role(Role::Status)),
                None => None,
            },
            Err(words) => Some(status_line(id, words.raw().to_owned(), palette)),
        }
    }

    /// The preview (`.launch-composer-checkout-preview`): which repository on which host, the exact folder once the
    /// helm has answered, and while it has not, that the launcher waits, or the helm's refusal. A polite live status,
    /// so a change is announced without interrupting.
    fn preview_block(&self, repo: &GithubRepo, palette: &theme::Palette, cx: &App) -> AnyElement {
        if self.draft.target.is_some() {
            return environment_checkout(repo, palette);
        }
        let host = self
            .chosen_host(cx)
            .map(super::host_name_with_phase)
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

    /// The folder field while a managed checkout is chosen: read-only, showing the preview's path (SPEC.md: "the
    /// composer's folder field shows the current preview's effective path read-only, or a pending or error state").
    pub(super) fn checkout_folder_field(&self) -> AnyElement {
        self.launcher_input(
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
            // A new environment's checkout has no folder yet: the helm names it once the environment connects.
            (Some(repo), _) if self.draft.target.is_some() => {
                format!("gh:{}", repo_identifier(repo))
            }
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
}

/// The Folder or Managed checkout choice, with their glyphs and words, as the launcher's Where row and the template
/// editor's destination both draw it (the old app's shared `DestinationTypeControl`), under `id`, with `chosen` chosen
/// and `help` as the group's hover help. It only reports a choice through `on_change`: what changing the kind does (the
/// launcher restores a folder draft; the editor keeps the destination's text) is each caller's own. Enter on it only
/// chooses, as the old control's did.
pub(crate) fn destination_choice(
    id: &'static str,
    chosen: DestinationKind,
    help: HoverHelp,
    on_change: impl Fn(DestinationKind, &mut Window, &mut App) + 'static,
) -> RadioGroup<DestinationKind> {
    RadioGroup::new(id, "Destination type", help, Role::Group)
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
        .chosen(Some(chosen))
        .on_change(move |kind, window, cx| on_change(*kind, window, cx))
}

/// The open suggestions list's rows (`field_list`): a branch glyph and the `owner/repo` per suggestion, or, with none,
/// a status line saying what to type.
fn draw_repository_rows(
    launcher: &Launcher,
    list: Stateful<Div>,
    weak: &WeakEntity<Launcher>,
    window: &Window,
    cx: &App,
) -> Drawn {
    let palette = theme::palette(cx);
    let list = list.aria_label("Repositories");
    let rows = launcher.repository_rows(window, cx);
    if rows.is_empty() {
        // The old list became a status saying what to type, a warning once something typed matches nothing.
        let typed = !launcher.checkout.repository.text(cx).trim().is_empty();
        let (words, color) = if typed {
            (NO_MATCH, palette.warn)
        } else {
            (TYPE_A_PAIR, palette.fg_1)
        };
        let empty = div()
            .id("launcher-repository-empty")
            .role(Role::Label)
            .aria_label(SharedString::from(words))
            .px(px(8.))
            .py(px(4.))
            .text_size(metrics::TEXT_SM)
            .text_color(theme::color(color))
            .child(words)
            .into_any_element();
        return Drawn {
            list: list.role(Role::Status),
            children: vec![empty],
            highlighted: None,
        };
    }
    let active = launcher.checkout.list.highlighted;
    let mut children = Vec::with_capacity(rows.len());
    for (index, repo) in rows.into_iter().enumerate() {
        let identifier = repo_identifier(&repo);
        let id = format!("launcher-repository-option-{identifier}");
        let words = HostSentence::new(SharedString::from(format!("{id}-words")))
            .host(&HostText::from_host(identifier.as_str()));
        let help = format!("Managed checkout of {}", words.accessible_name());
        let weak = weak.clone();
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
                    // The row colors its words itself (the fill's ink when highlighted); the glyph is quieter when
                    // not, as the old list's was.
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
    // Every child is a row, so the highlighted row's index is its child's.
    Drawn {
        list: list.role(Role::ListBox),
        children,
        highlighted: active,
    }
}

/// The preview line of a managed checkout into a new environment: no folder to preview yet, since the helm obtains the
/// preview once the environment's Farhelm connects and clones the repository inside it (SPEC_impl.md, "Tensorlake
/// sandbox sessions": "carry repository and naming intent until the new supervisor connects").
fn environment_checkout(repo: &GithubRepo, palette: &theme::Palette) -> AnyElement {
    let heading = HostSentence::new("launcher-preview-heading")
        .text("Managed checkout of ")
        .host(&HostText::from_host(repo_identifier(repo)))
        .text(" in the new environment");
    div()
        .id("launcher-preview")
        .debug_selector(|| "launcher-preview-environment".to_owned())
        .role(Role::Status)
        .aria_label(heading.accessible_name())
        .pl(px(LABEL_COLUMN + LABEL_GAP))
        .flex()
        .flex_col()
        .gap(px(2.))
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(palette.fg_1))
        .child(heading)
        .child(div().child("Cloned inside it once it starts"))
        .into_any_element()
}
