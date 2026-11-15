//! The bodies the desktop app sends to the helm's launch routes: create
//! (`POST /api/sessions`), Restart with (`POST /api/sessions/{id}/restart`) and
//! Replace and Replace with (`POST /api/sessions/{id}/replace`), plus the
//! launcher's reads and their replies: the folder read
//! (`POST /api/browse-directory`), and the managed-checkout destination's
//! repository suggestions (`POST /api/github-repositories`) and checkout
//! preview (`POST /api/github-checkout-preview`).
//!
//! The helm decodes each route into a request type of its own (`CreateReq`,
//! `RestartReq` and `ReplaceReq` in its sessions module), which also carries
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
use crate::{CommandLaunch, GithubCheckoutRequest, GithubRepo, LaunchSelection};

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

/// `POST /api/github-repositories`: repository suggestions for the launcher's managed-checkout destination on one host,
/// filtered by `query` (the repository field's text). The helm decodes it into its own `GithubRepositoriesReq`.
///
/// `expected_incarnation` is the host connection the launcher is looking at, as for [`BrowseDirectoryBody`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GithubRepositoriesBody {
    pub host: i64,
    pub expected_incarnation: Option<u64>,
    pub query: String,
}

/// The helm's answer to [`GithubRepositoriesBody`]: suggestions, recent repositories first, with discovery's own
/// status beside them, since an incomplete scan must never stop the user from choosing a valid pair they type
/// (SPEC.md, "Managed checkouts").
///
/// `host`, `incarnation` and `installation_identity` say which host installation answered (`host` is the registry id
/// in decimal), so the launcher can drop an answer about a host that was retargeted or reconnected meanwhile.
/// `scan_error` is one of the helm's own sentences, never the host's text. `needs_checkout_root` is the host's
/// effective configuration (its own folder, a command-line override, or the default), which decides whether the
/// launcher offers its inline checkout-folder setup; an older helm that leaves it out means no setup is offered.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GithubRepositoriesReply {
    pub host: String,
    pub incarnation: u64,
    pub installation_identity: String,
    pub repos: Vec<GithubRepo>,
    pub truncated: bool,
    pub scan_error: Option<String>,
    #[serde(default)]
    pub needs_checkout_root: bool,
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
