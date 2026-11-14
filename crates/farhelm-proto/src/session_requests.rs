//! The bodies the desktop app sends to the helm's launch routes: create
//! (`POST /api/sessions`), Restart with (`POST /api/sessions/{id}/restart`) and
//! Replace and Replace with (`POST /api/sessions/{id}/replace`).
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

use serde::Serialize;

use crate::{CommandLaunch, GithubCheckoutRequest, LaunchSelection};

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
