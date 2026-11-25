//! The bodies the desktop app sends to the helm's launch routes: create
//! (`POST /api/sessions`), a create in a new disposable environment
//! (`POST /api/environment-sessions`), Restart with
//! (`POST /api/sessions/{id}/restart`) and Replace and Replace with
//! (`POST /api/sessions/{id}/replace`), plus the
//! launcher's reads and their replies: the folder read
//! (`POST /api/browse-directory`), the launch history
//! (`GET /api/launch-history`), and the managed-checkout destination's
//! repository history (`GET /api/github-repositories`) and checkout
//! preview (`POST /api/github-checkout-preview`).
//!
//! The helm decodes each route into a request type of its own (`CreateReq`,
//! `EnvironmentCreateRequest`, `RestartReq` and `ReplaceReq` in its sessions
//! module), which also carries
//! fields only helm-internal callers set. These are the client's half: the
//! fields the GUI sends, built from the same launch types the helm reads
//! ([`LaunchSelection`], [`CommandLaunch`], [`GithubCheckoutRequest`]). The
//! launcher's bodies are large, and a hand-built JSON value would drift from
//! the helm's decoder without anything noticing, which is the copy-drift bug
//! class the UI channel's no-mirror rule exists to stop (SPEC_impl.md, "What is
//! pushed, and how changes are found"). The helm's tests serialize each body
//! here and decode it into its own type, so a field renamed on one side fails
//! there.
//!
//! What the GUI deliberately never sends: launch request keys
//! (`intent_key`). SPEC.md, "Creation": the command line's and agents'
//! idempotency keys are the create's retry keys, and "the GUI sends none";
//! duplicates around a lost connection are accepted instead. Nor a terminal
//! size: the first attach resizes the terminal to its viewer, so the helm's
//! 80x24 default only decides how the agent's first lines wrap, as in the old
//! app.

use serde::{Deserialize, Serialize};

#[cfg(doc)]
use crate::AcceptedGithubPreview;
use crate::launcher::TemplateNameKind;
use crate::{
    CommandLaunch, GithubCheckoutRequest, GithubRepo, LaunchSelection, LaunchTarget, Place,
};

/// `POST /api/sessions`: one create.
///
/// Exactly one of `launch` (an agent launch's choices, which the helm composes
/// into commands) and `command` (a command launch, sent as written) is set;
/// the helm refuses a body with both or neither rather than guessing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CreateSessionBody {
    /// The registry id of the host to create on. Absent means the helm's own
    /// host, which is also SPEC.md's fallback when no session is open.
    pub host: Option<i64>,
    /// The connection the launcher was looking at (`HostView::incarnation`),
    /// so a host retargeted or adopted onto another installation after the
    /// user chose it refuses the create rather than launching elsewhere
    /// (SPEC.md, "Host").
    pub expected_incarnation: Option<u64>,
    /// The folder, as typed: absolute, `~` or `~/path`. For a managed
    /// checkout, empty or exactly the accepted preview's `binding.cwd`; the
    /// helm refuses anything else.
    pub cwd: String,
    /// The session name; absent means auto-generated.
    pub title: Option<String>,
    pub launch: Option<LaunchSelection>,
    pub command: Option<CommandLaunch>,
    /// The initial prompt, sent once with this create and never stored.
    pub prompt: Option<String>,
    /// The one-time conversation selector, passed to the agent literally.
    pub conversation: Option<String>,
    /// The user answered the host's YOLO question for this launch (SPEC.md,
    /// "Creation"); without it the helm refuses a YOLO launch on a host that
    /// asks first.
    pub confirm_yolo: bool,
    /// A fresh managed checkout of a GitHub repository, with the preview the
    /// user accepted.
    pub github_checkout: Option<GithubCheckoutRequest>,
}

/// `POST /api/environment-sessions`: a create in a new disposable environment
/// (a sandbox or a container) that the helm makes from `recipe` at `place` for
/// this launch alone (SPEC.md, "Sandbox sessions" and "Container sessions").
///
/// A route of its own, not a target on [`CreateSessionBody`]: an older helm's
/// ordinary create ignores fields it does not know and defaults an absent host
/// to its own, so a recipe launch sent there would run on the wrong machine
/// instead of being refused (SPEC_impl.md, "Tensorlake sandbox sessions").
///
/// Only for `POST /api/environment-sessions`: the legacy
/// `POST /api/sandbox-sessions` takes no `place` and refuses unknown fields,
/// so it refuses this body.
///
/// The helm answers 202 with the launch's `{host_id, run_id}` once it has
/// admitted it, and the outcome arrives in the pushed host progress
/// (SPEC_impl.md, "The UI channel"). A refusal before admission (an
/// unconfirmed billed create, a missing recipe or place) is an ordinary error
/// reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnvironmentCreateBody {
    /// The recipe the environment is made from, by name.
    pub recipe: String,
    /// Where the environment is made: the Tensorlake account, or a container
    /// host.
    pub place: Place,
    /// The user answered "Create a sandbox for this session?" with Create and
    /// launch. Without it the helm refuses a place that asks before creating
    /// (a billed sandbox), unless the Tensorlake setting "Create Tensorlake
    /// sandboxes without asking" is on.
    pub confirmed: bool,
    /// Whether a template's name is a base the managed checkout numbers or an
    /// exact name; absent for a launch that no template named.
    pub name_kind: Option<TemplateNameKind>,
    /// The create itself. `host` and `expected_incarnation` stay absent, since
    /// the environment does not exist yet (the helm refuses either), and a
    /// managed checkout carries no preview, which the helm obtains once the new
    /// environment's Farhelm connects.
    pub request: CreateSessionBody,
}

/// `POST /api/sessions/{id}/restart`: a plain Restart, or Restart with.
///
/// Restart with sets one of `with` (an agent launch's edited choices) and
/// `with_command` (a command launch's edited command, resume command and YOLO
/// assertion); a plain Restart sets neither.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RestartSessionBody {
    /// A one-time conversation to resume instead of the captured one, offered
    /// only with an agent launch.
    pub conversation: Option<String>,
    /// The user confirmed stopping an agent that reads working. Without it
    /// the supervisor refuses rather than stopping a working agent unasked.
    pub stop_if_running: bool,
    pub with: Option<LaunchSelection>,
    pub with_command: Option<CommandLaunch>,
    /// As [`CreateSessionBody::confirm_yolo`].
    pub confirm_yolo: bool,
}

/// `POST /api/sessions/{id}/replace`: a plain Replace, or Replace with.
///
/// Replace creates a new session and then deletes the source. A plain
/// Replace copies the source's stored launch (`with` absent); Replace with
/// sends the launcher's edited create in `with`, on the source's own host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReplaceSessionBody {
    /// As [`CreateSessionBody::confirm_yolo`]. For Replace with the helm also
    /// counts `with.confirm_yolo` (either place confirms), so a builder sets
    /// this one and leaves the inner one false.
    pub confirm_yolo: bool,
    /// Replace with's create. Its `host` is absent or the source's own host:
    /// the helm refuses another host, since Clone is the way to move hosts.
    pub with: Option<CreateSessionBody>,
    /// The source delete that ends the replace may proceed only while nothing
    /// is alive: set when the question (or, with confirmations skipped, the
    /// click) showed nothing alive (SPEC.md, "Lifecycle operations", the
    /// confirmation rule). Set from the question's [`crate::DeleteGuard`]
    /// (`only_if_nothing_alive()`), never chosen independently of
    /// `only_if_agent_ended`.
    pub only_if_nothing_alive: bool,
    /// The source delete may proceed only while its agent stays ended: set
    /// when what was shown warned only about open terminal tabs. Set from the
    /// same [`crate::DeleteGuard`] (`only_if_agent_ended()`).
    pub only_if_agent_ended: bool,
}

/// `POST /api/browse-directory`: one bounded level of a folder on a host, for the launcher's folder check and its Browse
/// picker. The helm decodes it into its own `BrowseDirectoryReq`.
///
/// `expected_incarnation` is the host connection the launcher is looking at, so a reply never describes a connection
/// the user did not see. `membership_only` asks only whether the path is a usable directory, without listing it or
/// refining the folder history: the automatic folder check sets it; an ordinary Browse does not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BrowseDirectoryBody {
    pub host: i64,
    pub cwd: String,
    pub expected_incarnation: Option<u64>,
    pub membership_only: bool,
}

/// The helm's answer to [`BrowseDirectoryBody`]: the directory as the host resolved it, its parent, and its children,
/// at most a bounded number of them (`truncated` says when more were left out). A refusal (a missing folder, a path that
/// is not a directory) is an error reply in the supervisor's words, not a body.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BrowseDirectoryReply {
    pub cwd: String,
    pub parent: Option<String>,
    pub children: Vec<String>,
    pub truncated: bool,
}

/// `POST /api/github-checkout-preview`: the exact folder a managed checkout of `repo` would get on a host, before
/// anything is created. The helm decodes it into its own `GithubCheckoutPreviewReq` and answers an
/// [`AcceptedGithubPreview`], which the create then carries back unchanged in its [`GithubCheckoutRequest`].
///
/// `repo` is the `owner/repo` pair as selected and `title` the name the checkout's folder is derived from: the
/// session name the create will send, or empty for an unnamed checkout, which gets the lowest free `<repo>-N`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GithubCheckoutPreviewBody {
    pub host: i64,
    pub expected_incarnation: Option<u64>,
    pub repo: String,
    pub title: Option<String>,
}

/// The helm's answer to `GET /api/launch-history?host=<id>`: what launched successfully before on that host's current
/// installation, which the launcher offers again as recent folders and recent setups, and the checkout configuration's
/// revision when it was read.
///
/// The helm answers only for a connected host, from the history its live connection claims (never a registry row
/// alone), so a retargeted row never offers the replaced installation's folders; a host that reports no identity has no
/// history. Both lists come newest first.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LaunchHistoryReply {
    pub launches: Vec<LaunchHistoryEntry>,
    pub folders: Vec<FolderHistoryEntry>,
    /// The checkout configuration's revision as of this read. A higher revision than any seen means the checkout
    /// folders may have moved, so a checkout preview taken before it is stale.
    pub checkout_config_revision: i64,
    /// Whether this verified host lacks an effective checkout folder. Session-owned hosts have a home fallback.
    #[serde(default)]
    pub needs_checkout_root: bool,
}

/// One earlier successful agent launch: its destination and the choices it was made with, without its title or
/// conversation, which belong to that session only.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LaunchHistoryEntry {
    pub host: i64,
    /// A disposable environment's recipe and place, for a launch that created one.
    pub target: Option<LaunchTarget>,
    /// The repository of a launch into a fresh managed checkout; reusing it asks for a new checkout preview.
    pub github_repo: Option<GithubRepo>,
    /// The folder's identity as the launch accepted it; `None` for an older entry, whose `cwd` spelling is all there is.
    pub canonical_cwd: Option<String>,
    pub cwd: String,
    pub selection: LaunchSelection,
    pub created_at: i64,
    pub creation_seq: Option<u64>,
}

/// One folder a launch on this host started in, as the user typed it (`display_cwd`), with the identity the host
/// resolved it to for telling two spellings of one folder apart.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FolderHistoryEntry {
    pub host: i64,
    pub canonical_cwd: String,
    /// Whether a successful launch proved `canonical_cwd`; `false` for an older spelling the helm may refine later.
    pub canonical_proven: bool,
    pub display_cwd: String,
    pub created_at: i64,
    pub creation_seq: Option<u64>,
}

/// One explicit destination for a session move or a pin drop.
/// The ungrouped area has its own wire value so an omitted destination
/// cannot accidentally remove membership.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionGroupTarget {
    Ungrouped,
    Group(i64),
}

impl SessionGroupTarget {
    /// Membership as stored on the session row. The global pins have no group.
    pub fn group(self) -> Option<i64> {
        match self {
            Self::Ungrouped => None,
            Self::Group(id) => Some(id),
        }
    }
}

impl From<Option<i64>> for SessionGroupTarget {
    fn from(group: Option<i64>) -> Self {
        group.map_or(Self::Ungrouped, Self::Group)
    }
}

/// Before a neighbor move the helm checks both areas under the write lock.
/// This distinguishes a deliberate cross-area drag from a stale menu move.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct PinMoveAreas {
    pub source: SessionGroupTarget,
    pub destination: SessionGroupTarget,
}

/// A membership edit can also serve a pin drop that has no visible
/// neighbor. Such a drop verifies the pin and its source before appending.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SessionGroupMembership {
    pub target: SessionGroupTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin_source: Option<SessionGroupTarget>,
}

/// Create one group for all named session references, including a marked set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSessionGroup {
    pub name: String,
    pub sessions: Vec<String>,
}

/// Change either attribute of a group. An omitted value keeps its stored choice.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateSessionGroup {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collapsed: Option<bool>,
}
