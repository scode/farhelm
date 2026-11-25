//! The launcher's managed-checkout destination: the Where row's Folder or Managed checkout choice, the repository field
//! with its suggestions, and the Folder line that shows where the checkout will go.
//!
//! SPEC.md, "Managed checkouts" (the launcher parts) and SPEC_impl.md, "Checkout configuration". Ported
//! from the old launcher's `DestinationTypeControl` (`launch_controls.rs`), `github_checkout.rs` and the destination and
//! preview parts of `list/create_form.rs`, at reference commit `24bc0580`, with their words. What the user chose lives
//! in the draft ([`crate::launcher::draft::Checkout`]); what the helm said about it lives here. Here's the TLDR:
//!
//! - **The Where row** chooses the destination kind. Managed checkout puts Repository beside Host (or below Recipe
//!   for a new environment), with a Folder status line and its lifetime note below the address and name. Folder
//!   brings back the editable folder as it was. Enter on the switch only chooses, as the old control's did.
//! - **Repository suggestions** (`GET /api/github-repositories`) load once when the launcher opens. The Repository
//!   field and `gh:` search filter the same history in place. A valid typed `owner/repo` always appears first.
//!   Enter in the repository field accepts the highlighted choice without launching.
//! - **The preview** (`POST /api/github-checkout-preview`) is asked for whenever what it answers for changes: the host,
//!   its connection and installation, the repository (or choosing it again), the checkout's name, the agent choices and
//!   the configuration revision last observed (SPEC.md: "Host, installation, destination, title, agent choice and
//!   observed configuration changes invalidate an undispatched preview"). Launch waits for it
//!   ([`super::super::request::Preview`]).
//! - **Latest wins, one read at a time**, for the preview, as the folder check does (`latest`): an answer counts only
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
//! - **No checkout folder.** When the host's launch history reports no effective checkout folder, the shared
//!   explicit-save field for the default folder shows above the repository field (`settings::checkout_field`); a save
//!   rechecks the folder and refreshes the preview without reopening the launcher (SPEC.md, "Managed checkouts").
//! - **Launches that wait for the preview.** The YOLO question's "Start guarded session instead" changes the
//!   permission, which makes the preview stale, and a quick launch (a launch template's shortcut, started without
//!   showing the launcher) has not asked for its preview yet. Instead of refusing, either waits for the preview and
//!   starts once it is ready, as the old launcher's guarded submit and quick launch did; any edit, a close, a failed
//!   preview or another preview cancels the wait, and a quick launch that does not start is shown
//!   ([`Launcher::submit_or_wait_for_preview`]). The one exception is a quick launch's own template base name taken:
//!   it waits for the next numbered candidate's preview instead, out of sight ([`Launcher::settle_waiting_launch`]).

use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::session_requests::GithubCheckoutPreviewBody;
use farhelm_proto::ui_channel::{HostStateView, HostView, SessionRow};
use farhelm_proto::{AcceptedGithubPreview, GithubRepo, parse_github_repo};
use gpui::{
    AnyElement, AnyWindowHandle, App, AppContext as _, Context, Div, Entity,
    InteractiveElement as _, IntoElement, ParentElement as _, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity, Window, div, px,
};

use super::field_list::{Drawn, FieldList};
use super::latest::Latest;
use super::{ADDRESS_HEIGHT, Launcher, SourceConsent, field_label, help_line, help_line_container};
use crate::assets::Icon;
use crate::components::{HoverHelp, OptionRow, Role, TextInputEvent, TextInputState, Vertical};
use crate::focus::Mark;
use crate::host_text::{HostSentence, HostText};
use crate::launcher::destination_rows::destination_choice;
use crate::launcher::draft::{
    AgentChoices, CommandChoices, DestinationKind, Field, Mode, Tab, checkout_label,
    repo_identifier,
};
use crate::launcher::request::{Preview, UNVERIFIED_HOST, WAIT_FOR_PREVIEW, has_checkout_identity};
use crate::launcher::search::{Scope, repository_empty_message, scoped};
use crate::operations;
use crate::session_pane::details::{Checkout as SessionCheckout, checkout_mark};
use crate::settings::{CheckoutField, CheckoutUpdated, FolderScope};
use crate::theme::{self, metrics};

/// The most rows the suggestions list offers, the old `repository_choices` cap.
const MAX_CHOICES: usize = 100;
/// The suggestions list's element id (`.launch-composer-repository-options`), which tests find it by.
const REPOSITORY_LIST: &str = "launcher-repository-list";

/// Host checkouts and checkouts inside disposable environments have different Delete behavior.
const HOST_CHECKOUT_LIFETIME: &str =
    "New folder named after the session; it moves to the trash when its last session is deleted.";
const ENVIRONMENT_CHECKOUT_LIFETIME: &str = "New folder named after the session, inside the new environment; \
                                            deleting its last session removes it permanently.";
const EXISTING_ENVIRONMENT_CHECKOUT_LIFETIME: &str = "New folder inside this environment, named after the session. Deleting the checkout's last session removes the folder permanently.";
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
    /// A later preview can supply the destination while the answer still owns the source permission.
    source: SourceConsent,
    mark: Mark,
    key: PreviewKey,
    window: AnyWindowHandle,
    held_out_of_sight: bool,
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

// ---------------------------------------------------------------------------------------------------------------------
// The destination's state
// ---------------------------------------------------------------------------------------------------------------------

/// What the managed-checkout destination holds besides the draft: the repository field, its suggestions, the reads, and
/// the configuration revision observed. None of it is shared with any other surface (the composer search's `gh:` rows
/// read the same suggestions, within the launcher), and none of it survives the launcher.
pub(super) struct CheckoutState {
    /// The repository field: the chosen repository, or what is typed.
    pub(super) repository: TextInputState,
    /// The suggestions list under the repository field, with the row moved to with the arrows.
    list: FieldList,
    /// Repository history belongs to the helm. Load it once, independently of the selected destination.
    repositories_asked: bool,
    repositories: Option<Vec<GithubRepo>>,
    preview: Latest<PreviewKey, PreviewAnswer>,
    /// The highest checkout configuration revision observed, which never goes back down; `None` until one is.
    revision: Option<i64>,
    /// The last placeholder given to the name field. Setting it redraws the field, so only update it on change.
    pub(super) name_placeholder: String,
    retry: u64,
    /// The installed host that needed setup and the input mark from pressing Save. The repository is the next field
    /// to fill in when a recheck removes setup for this host and the user is still waiting for the save.
    focus_after_setup: Option<(HostKey, Mark)>,
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
    /// The repository field for a launcher opening on `field_text`, with its own
    /// subscriptions: what is typed edits the draft, Enter picks a suggestion and never launches, and focus opens
    /// and closes the list.
    pub(super) fn new(field_text: &str, window: &mut Window, cx: &mut Context<Launcher>) -> Self {
        let repository = TextInputState::new(window, cx);
        repository.set_text(field_text.to_owned(), window, cx);
        repository.set_placeholder("owner/repo", window, cx);
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
            list: FieldList::default(),
            repositories_asked: false,
            repositories: None,
            preview: Latest::default(),
            revision: None,
            name_placeholder: String::new(),
            retry: 0,
            focus_after_setup: None,
            setup: None,
            armed: None,
            advanced_from: None,
            _subscriptions: vec![events, focus, blur],
        }
    }
}

/// The suggestions for `query`: a valid typed pair first, then the recent repositories whose `owner/repo` contains
/// the query, in the helm's order, each once, at most [`MAX_CHOICES`] (the old `repository_choices`).
pub(in crate::launcher) fn repository_choices(
    query: &str,
    recent: &[GithubRepo],
) -> Vec<GithubRepo> {
    let query = query.trim().to_ascii_lowercase();
    let mut choices = Vec::new();
    if let Ok(repo) = parse_github_repo(&query) {
        choices.push(repo);
    }
    for repo in recent {
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
    /// Preview can only talk to a verified existing installation. A new environment provides its own
    /// preview after provisioning, and selecting it does not need this host's checkout folder.
    fn checkout_host(&self, cx: &App) -> Option<HostView> {
        if self.draft.target.is_some() {
            return None;
        }
        let view = self.chosen_host(cx)?;
        let connected = matches!(view.state, HostStateView::Connected { .. });
        let identified = has_checkout_identity(view);
        (connected && identified).then(|| view.clone())
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

    /// Only the current existing destination can ask for setup. Keep its last answer while a save is rechecked.
    fn needs_root(&self, cx: &App) -> Option<bool> {
        self.checkout_host(cx)?;
        self.launch_history(cx)
            .map(|reply| reply.needs_checkout_root)
    }

    /// Release a pending focus handoff if the finished read still needs setup.
    pub(super) fn checkout_root_read(&mut self, needs_root: bool) {
        if needs_root {
            self.checkout.focus_after_setup = None;
        }
    }

    /// The `gh:` query the composer search holds, while it holds one.
    fn search_repository_query(&self, cx: &App) -> Option<String> {
        let text = self.search.text(cx);
        let (scope, value) = scoped(&text);
        (scope == Scope::Github).then(|| value.to_owned())
    }

    fn repository_history(&self) -> &[GithubRepo] {
        self.checkout.repositories.as_deref().unwrap_or_default()
    }

    fn repository_pending(&self) -> Option<bool> {
        self.checkout
            .repositories_asked
            .then_some(self.checkout.repositories.is_none())
    }

    /// A `gh:` query uses the same remembered repositories and typed choices as the Repository field.
    pub(super) fn search_repositories(&self, cx: &App) -> (Vec<GithubRepo>, Option<bool>) {
        let Some(query) = self.search_repository_query(cx) else {
            return (Vec::new(), None);
        };
        (
            repository_choices(&query, self.repository_history()),
            self.repository_pending(),
        )
    }

    /// Whether the search holds an unselected `gh:` query. It holds Launch until the user chooses the repository
    /// because the existing Folder draft is a destination the user is trying to replace.
    pub(super) fn search_holds_repository_query(&self, cx: &App) -> bool {
        self.search_repository_query(cx).is_some()
    }

    /// What a preview would answer for now, if a repository is chosen on a host that can be asked.
    fn preview_key(&self, cx: &App) -> Option<PreviewKey> {
        if self.checkout_root_refresh_pending() {
            return None;
        }
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

    /// A drawn launcher reads its repository history once and follows the preview as the destination changes.
    pub(super) fn follow_checkout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.follow_repository_history(cx);
        self.follow_preview(cx);
        // The name field follows a name the draft changed on its own: a numbered default moving on, or the copied title
        // ignored or shown again as the destination changes. Typing changes the draft to the field's text, so this
        // never fights the user.
        let shown = super::Input::Name.present(&self.draft, &self.opened).text;
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
        source: SourceConsent,
        mark: Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        // A quick launch's launcher is never drawn, so its preview is asked for here rather than while drawing.
        self.follow_preview(cx);
        // Compared with the launcher's own constant, never with helm words: a reworded reason must reword the constant.
        if matches!(self.refusal(cx), Some(super::Refusal(words)) if words == WAIT_FOR_PREVIEW)
            && self.checkout_preview(cx) == Preview::Waiting
            && let Some(key) = self.preview_key(cx)
        {
            self.checkout.armed = Some(Armed {
                source,
                mark,
                key,
                window: window.window_handle(),
                held_out_of_sight: self.quick_launch_unseen || super::super::launch::waking(cx),
            });
            cx.notify();
            return true;
        }
        self.submit(false, None, source, mark, window, cx);
        false
    }

    /// A preview answered: settle the launch waiting for one, if there is one. It waits for one preview only, the one
    /// due when it was armed; anything else that asked for a new preview meanwhile (the host reconnecting, a numbered
    /// name moving on after a taken folder, a newer configuration revision, a save in the inline setup) drops it, since
    /// its folder is one the user has not seen yet (SPEC.md: Launch "never submits automatically or silently chooses
    /// another directory"), as does a failed preview. A ready one starts it with its mark, if its launcher is still
    /// showing or was never shown. A failed preview for the held wake also brings back the launch refusal alert,
    /// using the failure from that preview. A superseded preview comes back for a new choice.
    ///
    /// A launch held out of sight follows its own numbered name. When the preview it waits for came back taken and
    /// the taken-name rule moved the name to its next candidate, with all other inputs intact, it waits for that
    /// candidate's preview. A shortcut or a press before waking the host already authorized that allocation. When
    /// the form stays shown after a guarded answer, the user can see the name change and press again.
    fn settle_waiting_launch(launcher: &WeakEntity<Launcher>, cx: &mut App) {
        let Some(entity) = launcher.upgrade() else {
            return;
        };
        let after_wake =
            super::super::launch::waking(cx) && super::super::launch::still_preparing(&entity, cx);
        let settled = entity.update(cx, |launcher, cx| {
            let armed = launcher.checkout.armed.as_ref()?;
            let renamed = launcher.preview_key(cx).filter(|key| {
                armed.held_out_of_sight
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
            let (start, refusal) = if launcher.preview_key(cx).as_ref() != Some(&armed.key) {
                (false, None)
            } else {
                match launcher.checkout.preview.answer_for(&armed.key) {
                    None => return None,
                    Some(Ok(_)) => (true, None),
                    Some(Err(words)) => (
                        false,
                        after_wake.then(|| {
                            HostSentence::new("launcher-checkout-wake-refusal").host(words)
                        }),
                    ),
                }
            };
            let armed = launcher.checkout.armed.take()?;
            if let Some(refusal) = refusal {
                launcher.wake_refused(refusal, cx);
            }
            let start = start && (armed.held_out_of_sight || launcher.layer.is_some());
            Some((start, armed))
        });
        let Some((start, armed)) = settled else {
            return;
        };
        if start {
            let _ = cx.update_window(armed.window, |_, window, cx| {
                entity.update(cx, |launcher, cx| {
                    launcher.submit(false, None, armed.source, armed.mark, window, cx)
                });
            });
        }
        // A launch that went out moved the quick launch's launcher to waiting for its reply; one that did not comes
        // back to be seen.
        if armed.held_out_of_sight {
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

    /// The list is small and global. Typing and switching destinations filter locally even if the read fails.
    fn follow_repository_history(&mut self, cx: &mut Context<Self>) {
        if self.checkout.repositories_asked {
            return;
        }
        let Ok(path) = ApiPath::api(&["github-repositories"]) else {
            return;
        };
        let launcher = cx.weak_entity();
        self.checkout.repositories_asked = operations::read::<Vec<GithubRepo>>(
            path,
            "GET /api/github-repositories",
            cx,
            move |answer, cx| {
                let _ = launcher.update(cx, |launcher, cx| {
                    launcher.checkout.repositories = Some(answer.unwrap_or_default());
                    // A typed match can fill the list before the reply. Re-evaluate its highlight once the rows change.
                    launcher.checkout.list.highlighted = None;
                    cx.notify();
                });
            },
        );
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
        cx.notify();
    }

    /// The preview as [`super::super::request::Preview`] puts it for the draft as it is now.
    pub(super) fn checkout_preview(&self, cx: &App) -> Preview<'_> {
        match self.preview_answer(cx) {
            Some(Ok(preview)) => Preview::Ready(preview),
            Some(Err(_)) => Preview::Failed,
            None => Preview::Waiting,
        }
    }

    /// Both the destination display and launch use the answer for the current draft and host connection.
    fn preview_answer(&self, cx: &App) -> Option<&PreviewAnswer> {
        self.checkout.preview.answer_for(&self.preview_key(cx)?)
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
    /// Filter the same history locally for each edit to the Repository field.
    fn repository_rows(&self, cx: &App) -> Vec<GithubRepo> {
        let query = self.checkout.repository.text(cx).to_string();
        repository_choices(&query, self.repository_history())
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
        let count = self.repository_rows(cx).len();
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
        let rows = self.repository_rows(cx);
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

    /// The Folder status for a resumable destination. A wake can promise an allocation only for an installation
    /// that can obtain its own preview after reconnecting.
    fn checkout_wake_notice(&self, cx: &App) -> Option<(&'static str, &'static str)> {
        self.draft.checkout.as_ref()?.repo.as_ref()?;
        let host = self.chosen_host(cx)?;
        if !self.draft.can_wake(host) {
            return None;
        }
        Some(if !has_checkout_identity(host) {
            ("unverified", UNVERIFIED_HOST)
        } else if self.draft.checkout_title().is_empty() {
            (
                "wake",
                "The folder is picked automatically when the host is ready",
            )
        } else {
            (
                "wake",
                "The folder is named after the session when the host is ready",
            )
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------------------------------------------------

impl Launcher {
    /// The switch in the Destination card's header. A held destination has no switch.
    pub(super) fn destination_switch(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
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
        Some(
            div()
                .id("launcher-where-row")
                .debug_selector(|| "launcher-where-row".into())
                .child(group)
                .into_any_element(),
        )
    }

    /// Use the live source row, as the Replace with footer does. Its current
    /// checkout membership matters here, regardless of how it first launched.
    fn replacement_checkout_source<'a>(
        &self,
        collections: &'a farhelm_helm_client::Collections,
    ) -> Option<(&'a SessionRow, SessionCheckout)> {
        let Mode::ReplaceWith { source } = &self.draft.mode else {
            return None;
        };
        let row = collections.get::<SessionRow>(source)?;
        Some((row, SessionCheckout::of(row, collections)?))
    }

    /// A Replace with keeps the source's checkout when the folder is still the
    /// source's working directory. The check can confirm a spelling that
    /// resolves to that directory or show that a symlink now leads elsewhere.
    pub(super) fn replacement_folder_hint(
        &self,
        palette: &theme::Palette,
        cx: &App,
    ) -> Option<Div> {
        let collections = self.store.read(cx).collections();
        let (row, checkout) = self.replacement_checkout_source(collections)?;
        if self.draft.checkout.is_some() {
            return None;
        }
        let recorded = row.info.canonical_cwd.as_deref();
        let expected = recorded.unwrap_or(&row.info.cwd);
        let same_directory = match self.current_folder_check(cx) {
            Some(Ok(resolved)) if recorded.is_some() => resolved == expected,
            Some(Err(_)) => false,
            // For an alias, wait for the current check when the session
            // supplied a canonical path. The spelling may have been repointed.
            _ => self.draft.folder.value() == expected,
        };
        if !same_directory {
            return None;
        }
        let sentence = HostSentence::new("launcher-replacement-checkout-folder")
            .text("This session's managed checkout of ")
            .host(&checkout.repo)
            .text(". The replacement keeps working in it, with its changes.");
        Some(
            help_line_container(palette)
                .flex()
                .items_start()
                .gap(px(6.))
                .child(checkout_mark(
                    "launcher-replacement-checkout-mark",
                    checkout,
                    palette,
                ))
                .child(sentence),
        )
    }

    /// Explain the source checkout's fate beside Where when making a fresh
    /// clone. The selected repository belongs to the replacement, so it
    /// cannot say which checkout the source can leave behind.
    pub(super) fn replacement_checkout_warning(
        &self,
        palette: &theme::Palette,
        cx: &App,
    ) -> Option<Div> {
        self.draft.checkout.as_ref()?;
        let (_, checkout) = self.replacement_checkout_source(self.store.read(cx).collections())?;
        let ending = if checkout.in_environment {
            " If this replaces its last session, the old checkout moves to the trash inside the sandbox or container. \
              Deleting the sandbox or container also deletes its trash."
        } else {
            " If this replaces its last session, the old checkout moves to the trash."
        };
        let sentence = HostSentence::new("launcher-replacement-checkout-warning")
            .text("Starts from a fresh clone without the changes in this session's checkout of ")
            .host(&checkout.repo)
            .text(".")
            .text(ending);
        Some(
            help_line_container(palette)
                .text_color(theme::color(palette.warn_bright))
                .child(sentence),
        )
    }

    /// Anchor suggestions to the width of the repository input, whether it shares the address border or has
    /// a row of its own. Follow dismissals by the layer manager so Escape leaves it closed until the next input.
    pub(super) fn repository_control(
        &mut self,
        embedded: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.checkout.list.follow(window, cx);
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
        let field = if embedded {
            field.embedded().height(ADDRESS_HEIGHT - px(2.))
        } else {
            field
        };
        div()
            .relative()
            .min_w_0()
            .child(field)
            .child(tracker)
            .into_any_element()
    }

    /// The root setup precedes the address because the repository on an installed host needs it. The history
    /// refresh can remove the field after a save, so leave a refusal in that same place even if the field goes away.
    pub(super) fn checkout_setup(
        &mut self,
        palette: &theme::Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let setup = self.setup_field(window, cx);
        if setup.is_none()
            && self.draft.target.is_none()
            && let Some((field, _)) = &self.checkout.setup
            && let Some(reason) = field.read(cx).refusal()
        {
            let id = "launcher-checkout-setup-refused";
            let message = HostSentence::new(id).host(reason);
            return Some(super::refusal_alert(id, message, palette));
        }
        setup
    }

    /// The inline checkout-folder field, while the chosen host needs a checkout folder set up: the shared explicit-save
    /// field over the default folder (SPEC.md: "When the selected installed host has no checkout folder in effect, the
    /// launcher's managed-checkout destination offers the same explicit-save field"). Made the first time it shows.
    /// A successful save or a value found changed after refusal refreshes the checkout-folder answer and preview.
    fn setup_field(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let needs_root = self.needs_root(cx);
        if needs_root != Some(true) {
            // The field also disappears when the placement changes. The handoff belongs to the host we rechecked.
            if let Some((host, mark)) = self.checkout.focus_after_setup.take()
                && needs_root == Some(false)
                && self.checkout_host_key(cx).as_ref() == Some(&host)
            {
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
            let updated = cx.subscribe(
                &field,
                |launcher: &mut Launcher, _, updated: &CheckoutUpdated, cx| {
                    // A save can settle after an accepted wake hid the launcher. Its fresh connected preview
                    // reads the effective folder itself, while this history refresh needs a visible launcher.
                    if launcher.layer.is_none()
                        && super::super::launch::wake_holds_launcher(&cx.entity(), cx)
                    {
                        return;
                    }
                    launcher.checkout.retry += 1;
                    launcher.checkout.focus_after_setup = updated
                        .saved_at
                        .filter(|_| launcher.needs_root(cx) == Some(true))
                        .and_then(|mark| launcher.checkout_host_key(cx).map(|host| (host, mark)));
                    launcher.refresh_checkout_root();
                    cx.notify();
                },
            );
            self.checkout.setup = Some((field, updated));
        }
        let (field, _) = self.checkout.setup.as_ref()?;
        // The same name as an id and as a debug selector, as `row` does: the in-process tests find the field by the
        // selector, and the test-automation bridge, which cannot see selectors, by the id.
        Some(
            div()
                .id("launcher-checkout-setup")
                .debug_selector(|| "launcher-checkout-setup".into())
                .child(field.clone())
                .into_any_element(),
        )
    }

    /// Keep the Folder line a single polite live region across selection, allocation, preview and refusal. The
    /// platform adapters announce the changing detail through its inherited priority. The region keeps its name.
    pub(super) fn checkout_folder_line(&self, palette: &theme::Palette, cx: &App) -> AnyElement {
        let (state, detail, ink, size) = self.checkout_folder_detail(palette, cx);
        div()
            .id("launcher-folder-row")
            .debug_selector(|| "launcher-folder-row".into())
            .child(
                div()
                    .id("launcher-preview")
                    .debug_selector(move || format!("launcher-preview-{state}"))
                    .role(Role::Status)
                    .aria_label("Folder")
                    .a11y_synthetic_children(|builder| {
                        builder
                            .parent_node()
                            .set_live(gpui::accesskit::Live::Polite)
                    })
                    .flex()
                    .items_baseline()
                    .gap(px(6.))
                    // Center the glyph on the label's first line even when the detail wraps below it.
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(field_label("Folder", palette))
                            .child(
                                gpui::svg()
                                    .path(Icon::Branch.path())
                                    .size(px(12.))
                                    .flex_none()
                                    .text_color(theme::color(palette.fg_1)),
                            ),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .text_size(size)
                            .text_color(theme::color(ink))
                            .child(detail.with_live_value()),
                    ),
            )
            .into_any_element()
    }

    /// A checkout needs a repository before any folder can be promised. New environments allocate after they connect,
    /// and resumable hosts allocate after wake. A preview from a previous connection supplies neither path.
    fn checkout_folder_detail(
        &self,
        palette: &theme::Palette,
        cx: &App,
    ) -> (&'static str, HostSentence, theme::Token, gpui::Pixels) {
        let (state, words) = if self
            .draft
            .checkout
            .as_ref()
            .is_none_or(|checkout| checkout.repo.is_none())
        {
            ("unselected", "Select a repository")
        } else if self.draft.target.is_some() {
            (
                "environment",
                "Cloned inside the new environment once it starts",
            )
        } else if let Some(notice) = self.checkout_wake_notice(cx) {
            notice
        } else {
            match self.preview_answer(cx) {
                Some(Ok(preview)) => {
                    return (
                        "ready",
                        HostSentence::new("launcher-preview-path")
                            .mono(&HostText::from_host(preview.binding.cwd.as_str())),
                        palette.fg_0,
                        metrics::TEXT_MD,
                    );
                }
                Some(Err(words)) => {
                    return (
                        "failed",
                        HostSentence::new("launcher-preview-error").host(words),
                        palette.warn_bright,
                        metrics::TEXT_SM,
                    );
                }
                None => ("pending", "Waiting for a current checkout preview"),
            }
        };
        (
            state,
            HostSentence::new("launcher-preview-message").text(words),
            palette.fg_1,
            metrics::TEXT_SM,
        )
    }

    /// Describe future Delete of the new checkout. Replace with's separate warning describes retiring the old checkout.
    pub(super) fn checkout_lifetime_note(&self, palette: &theme::Palette, cx: &App) -> AnyElement {
        use crate::session_list::environment::Environment;
        let words = if self.draft.target.is_some() {
            ENVIRONMENT_CHECKOUT_LIFETIME
        } else if self
            .chosen_host(cx)
            .and_then(Environment::of_host)
            .is_some()
        {
            EXISTING_ENVIRONMENT_CHECKOUT_LIFETIME
        } else {
            HOST_CHECKOUT_LIFETIME
        };
        help_line(words, palette).into_any_element()
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
            // Checking out on a host without a connection cannot name a path yet.
            (Some(repo), _)
                if self.draft.target.is_some() || self.checkout_wake_notice(cx).is_some() =>
            {
                checkout_label(repo)
            }
            (Some(_), Preview::Ready(preview)) => preview.binding.cwd.clone(),
            (Some(repo), Preview::Waiting | Preview::Failed) => {
                format!("{} (preview pending)", checkout_label(repo))
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

/// The open suggestions list's rows (`field_list`): a branch glyph and the `owner/repo` per suggestion, or, with none,
/// a status line saying what to type or whether history is still loading.
fn draw_repository_rows(
    launcher: &Launcher,
    list: Stateful<Div>,
    weak: &WeakEntity<Launcher>,
    _window: &Window,
    cx: &App,
) -> Drawn {
    let palette = theme::palette(cx);
    let list = list.aria_label("Repositories");
    let rows = launcher.repository_rows(cx);
    if rows.is_empty() {
        let query = launcher.checkout.repository.text(cx);
        let message = repository_empty_message(&query, launcher.repository_pending());
        let words = message.text;
        let color = if message.warning {
            palette.warn
        } else {
            palette.fg_1
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
            .in_popover()
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
