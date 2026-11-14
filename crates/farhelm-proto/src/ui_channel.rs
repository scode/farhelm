//! The UI channel's wire types: one ordered WebSocket per UI, carrying the
//! GUI's commands, the helm's replies, and the state the helm pushes.
//!
//! SPEC_impl.md's "GUI architecture" section is the rulebook this module
//! implements the wire for; SPEC.md's "Losing the connection to the helm" is
//! the product behavior it exists to make simple. The short version:
//!
//! - The GUI sends [`ClientMessage::Command`]s, each an `{id, method, path,
//!   body}` envelope that the helm runs through its ordinary HTTP router in
//!   process. There is no second, typed command API: a command is the HTTP
//!   request it names, with the same handlers, validation and authentication.
//! - The helm answers every command with exactly one [`ServerMessage::Reply`],
//!   in the order the commands arrived, and pushes every change the command
//!   caused BEFORE that reply. A GUI that sees "done" already shows the result.
//! - State arrives as collections keyed by id: a [`ServerMessage::Snapshot`]
//!   when the channel opens, then [`ServerMessage::Changes`] per collection,
//!   each entry replaced whole. Every entry travels with its key
//!   ([`KeyedEntry`]), the one the helm diffs by, and the GUI applies both
//!   with one generic piece of code under that key, never per kind.
//! - A command's path is an [`ApiPath`], never a plain string: the one type
//!   that both builds canonical paths for the GUI and refuses non-canonical
//!   ones on the helm (SPEC_impl.md, "Paths are a trust boundary").
//!
//! Entries travel as JSON values inside the envelope and are decoded with the
//! entry types below ([`SessionRow`], [`HostView`], [`Preferences`]) and the
//! domain modules' ([`crate::approvals::PendingApproval`],
//! [`crate::launcher::LaunchTemplate`], [`crate::notices::Notice`],
//! [`crate::announcements::AnnouncementQueue`],
//! [`crate::provisioning::ProvisioningView`]). Those are the SAME types the helm's
//! GET handlers serialize, so the helm and the GUI cannot drift into two
//! copies of one shape (the old GUI hand-mirrored the helm's private types,
//! and the copies drifted).
//!
//! What this module deliberately does not carry: terminal output and file
//! transfers. Each terminal keeps its own stream and each transfer its own
//! HTTP request, so bulk data never delays a click.

use crate::SessionInfo;
use serde::{Deserialize, Serialize};

mod path;
pub use path::{ApiPath, PathError};

/// The path of the UI channel's WebSocket upgrade.
pub const UI_CHANNEL_PATH: &str = "/api/ui";

/// The channel-local command that sets this channel's session-list query.
///
/// Outside `/api/` on purpose: the helm handles it in the channel itself
/// rather than routing it, because the query is state of THIS channel (the
/// helm keeps serving the filtered, sorted view and its counts after every
/// change), not something any HTTP route owns. It is still an ordinary
/// command, so it is admitted in order and answered like every other one.
pub const SESSION_QUERY_PATH: &str = "/ui/session-query";

/// A message from the GUI to the helm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Run one command; answered by exactly one [`ServerMessage::Reply`].
    Command(Command),
    /// A heartbeat, answered at once by [`ServerMessage::Pong`] with the same
    /// number. It bypasses the command queue, so a long command never makes a
    /// live channel look dead to the GUI's watchdog.
    Ping { n: u64 },
}

/// One command: the HTTP request the helm should run on the GUI's behalf.
///
/// `path` is an absolute `/api/...` path, optionally with a `?query`, or the
/// channel-local [`SESSION_QUERY_PATH`]. Its type is the trust boundary: the
/// GUI can only build it by percent-encoding every segment, and a command
/// whose path is not canonical (empty, `.` or `..` segments, also
/// percent-encoded, a fragment) does not decode on the helm, so it never
/// reaches the router. A segment holding an encoded `/` or `\` does decode
/// (a host's id may contain one); the helm refuses that command with an
/// ordinary reply instead, and the channel continues.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Command {
    /// Chosen by the GUI, echoed in the reply. Unique within one channel.
    pub id: u64,
    pub method: Method,
    pub path: ApiPath,
    /// The JSON request body, if the route takes one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
}

/// A command's HTTP method: the closed set of methods the helm's routes
/// take, so a typo in the GUI cannot reach the wire.
///
/// On the wire it is the method's usual uppercase spelling (`"GET"`,
/// `"POST"`, ...), the same strings the field carried when it was a plain
/// string. Decoding is exact: `"get"` or `"OPTIONS"` is not a method, so a
/// command carrying one does not decode as a [`ClientMessage`], and the helm
/// closes the channel as for any message it cannot read, the same as for a
/// non-canonical [`ApiPath`]. No GUI built from this tree can send one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

impl Method {
    /// The wire spelling, for messages and logs. Always equal to what serde
    /// writes for this method.
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Delete => "DELETE",
            Method::Patch => "PATCH",
        }
    }
}

/// A message from the helm to the GUI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Every pushed collection as it stands, sent once, first, when the
    /// channel opens. A collection whose read failed is listed as
    /// unavailable with no entries, never as empty.
    Snapshot { collections: Vec<CollectionState> },
    /// What changed in one collection since the last message about it.
    Changes(CollectionChanges),
    /// The answer to one command, in command order.
    Reply(Reply),
    /// The answer to a [`ClientMessage::Ping`].
    Pong { n: u64 },
    /// Another UI took this helm's seat (SPEC.md, "One GUI at a time"). The
    /// helm closes the channel right after sending it. Only this message
    /// counts as a takeover; a channel that merely drops is a reset.
    Displaced,
}

/// The helm's answer to one command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reply {
    /// The [`Command::id`] this answers.
    pub id: u64,
    /// The HTTP status the routed request answered with. 202 means the helm
    /// accepted long work: the watchdog is settled, the outcome arrives as
    /// pushed state, and success is not implied.
    pub status: u16,
    /// The response body: its JSON when it was JSON, its text as a JSON
    /// string otherwise, absent when it was empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
}

/// The pushed collections, by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Collection {
    /// The session list, as this channel's [`SessionQuery`] selects it.
    Sessions,
    /// The registered hosts, as `GET /api/hosts` serves them.
    Hosts,
    /// The helm's shared preferences, as one entry keyed
    /// [`PREFERENCES_KEY`].
    Preferences,
    /// Agent requests waiting for the user's answer.
    Approvals,
    /// Launch templates, as `GET /api/templates` serves them, keyed by name.
    Templates,
    /// Shared notices about hosts, as `GET /api/notices` serves them, keyed
    /// by notice id.
    Notices,
    /// The announcement queue (pending and shown), as one entry keyed
    /// [`ANNOUNCEMENTS_KEY`]: the queue is small and its two lists are one
    /// consistent read, so it travels whole, like the preferences row.
    Announcements,
    /// Provisioning progress for every host with a run this helm retains, as
    /// `GET /api/provisioning` serves it, keyed by host id. A host without a
    /// run is absent, not listed as idle.
    HostProgress,
}

/// The one key the [`Collection::Preferences`] collection uses: the helm has
/// one preferences row, and modelling it as a one-entry collection keeps it
/// inside the generic apply rather than giving it a code path of its own.
pub const PREFERENCES_KEY: &str = "preferences";

/// The one key the [`Collection::Announcements`] collection uses.
pub const ANNOUNCEMENTS_KEY: &str = "announcements";

/// One entry of a pushed collection, with the key it is filed under.
///
/// The key travels beside the entry rather than being derived from it on
/// each side: the helm diffs by this key and the GUI applies by it, so there
/// is exactly one definition of what identifies an entry, and a GUI never has
/// to know which field of which type carries it. Keys are what
/// [`CollectionMeta::order`] and [`CollectionChanges::remove`] name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyedEntry {
    pub key: String,
    /// The entry as the collection's GET route serves it; it decodes as the
    /// collection's entry type (see [`Collection`]'s variants).
    pub value: serde_json::Value,
}

/// One collection as a whole, in a [`ServerMessage::Snapshot`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectionState {
    pub collection: Collection,
    pub meta: CollectionMeta,
    /// The entries, in the order the helm serves them.
    pub entries: Vec<KeyedEntry>,
}

/// A change to one collection.
///
/// `upsert` entries replace any entry with the same key whole; `remove` names
/// keys that are gone. Both are empty when only the metadata moved (an order
/// change, a count, or the collection becoming unavailable). While a
/// collection is unavailable the GUI keeps the entries it has: a failed read
/// is never evidence that something was deleted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectionChanges {
    pub collection: Collection,
    pub meta: CollectionMeta,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub upsert: Vec<KeyedEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove: Vec<String>,
}

/// What a collection says about itself beside its entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionMeta {
    pub availability: Availability,
    /// Every key, in the order the helm serves the collection. The GUI draws
    /// in this order; entries arrive keyed, not positioned.
    pub order: Vec<String>,
    /// The session list's counts, which describe more than the entries do
    /// (the whole view, the filter's matches, whether a cap cut the list).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counts: Option<ListCounts>,
}

/// Whether the helm could read a collection the last time it tried.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Availability {
    Available,
    /// The read failed. `error` is the helm's own description, for display.
    /// The order and counts are the last ones that were available.
    Unavailable {
        error: String,
    },
}

/// The session list's counts, as `GET /api/sessions` reports them beside its
/// rows ([`SessionList`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListCounts {
    pub total: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matching: Option<u64>,
    pub truncated: bool,
}

/// The body of the channel-local [`SESSION_QUERY_PATH`] command: which
/// sessions this channel's [`Collection::Sessions`] holds, and in what order.
///
/// The same filters and orders `GET /api/sessions` accepts, with the same
/// spellings; the helm validates them by serving the list with them, so an
/// unknown status or sort word is refused rather than silently ignored. The
/// list's cap is the helm's own fixed listing cap. Every field absent means
/// every session, in creation order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
}

// ---------------------------------------------------------------------------
// Entry types the helm serves over HTTP too. Moved here from the helm so that
// one definition serializes on one side and deserializes on the other.
// ---------------------------------------------------------------------------

/// One row of `GET /api/sessions` and of the UI channel's sessions collection: a
/// session as its supervisor reports it, plus the helm's facts about its host.
///
/// `SessionInfo` is FLATTENED rather than nested, which is the whole
/// compatibility story for the UI that predates this PR: every field it
/// already reads (`id`, `title`, `status`, `tabs`, …) stays at the top
/// level of each row, and the host fields are additive siblings. A nested
/// `{"session": {...}, "host": ...}` would have been tidier and would have
/// broken every existing reader.
///
/// The flattening is also why new `SessionInfo` timestamps need nothing here
/// to reach a client: activity age and stable work ordering arrive as raw
/// sibling fields without a second request or a row shape of their own.
///
/// Session references stay bare while filtering and sorting; REST and agent
/// projections add this row's captured entry prefix only when building output.
/// Other fields retain what the supervisor reported, zeroes included.
/// `?sort=activity` groups by connected Running/Waiting first and then
/// applies `effective_work_started_at`, whose zero fallback is creation in
/// milliseconds; row age applies `effective_activity`, whose zero fallback
/// is creation in seconds. Neither derived value is written back, because a
/// synthesized timestamp would be indistinguishable from an authoritative
/// observation at the next monotonic cache merge. The displayed age and the
/// seen/unseen comparison play no part in the grouping or the work-start
/// order — they describe output recency, a separate fact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionRow {
    #[serde(flatten)]
    pub info: SessionInfo,
    /// The registry id of the host this session lives on — the value a
    /// client hands back to `POST /api/sessions` to create alongside it,
    /// and the join key into `GET /api/hosts`.
    pub host: i64,
    /// Boundary projections use the entry from this same snapshot. Keeping it
    /// private to serialization avoids a second lookup after removal or retarget.
    ///
    /// Helm-side only and never on the wire: the GUI sees an empty string.
    #[serde(skip)]
    pub entry_uuid: String,
    /// The install identity the REGISTRY held for that host when the reply
    /// was being built (`HostView::identity`'s value), denormalized onto
    /// the row so a client can bind "the session the user is looking at"
    /// to the INSTALL behind it rather than to the row id alone. "Was
    /// being built" is deliberate: the identity is sampled beside the
    /// session content, not atomically with it, and can be one write stale
    /// by the time the reply lands — the assembly orders its reads so that
    /// such skew mismatches (and the client falls back safely) rather than
    /// falsely matching; see the identity join in `session_list_staged`. A
    /// `HostId` outlives retargets and adoptions while the machine behind
    /// it changes; this field is what lets the create dialog's host default
    /// notice that the install it was derived from is no longer the one
    /// the row id now names (the #156-review residual).
    ///
    /// `None` means the registry records no identity for the host (never
    /// reached, or it reports none) — and the key is ALWAYS serialized, so
    /// a client can tell "this helm says no identity" (`null`) from "this
    /// helm predates the field" (key absent) and degrade to the old
    /// row-id-only behavior only in the latter case.
    pub host_identity: Option<String>,
    /// The helm's display-name rendering of that host, denormalized onto
    /// the row so a list can be drawn without a second request.
    pub host_name: String,
    /// Whether this row is last-known knowledge rather than a live report:
    /// true for every row of a host that is not currently connected, in
    /// ANY of the non-connected states. SPEC.md requires such rows to stay
    /// listed and be clearly marked, and requires operations against them
    /// to be refused — this flag is the first half, and
    /// `crate::route_session` is the second.
    pub stale: bool,
    /// The activity stamp that was current the last time some client had
    /// this session open (`store::HelmStore::seen_activity`'s per-session
    /// answer), or `None` when nothing is currently recorded — either
    /// because no client has ever opened the session, or because a client
    /// manually marked it unread (`store::HelmStore::clear_seen` DELETES
    /// the row rather than writing a sentinel, so the two cases are
    /// indistinguishable here on purpose: SPEC.md, Status defines "unseen"
    /// from the absence of a stamp either way). Compared against
    /// `SessionInfo::effective_activity` on the CLIENT to decide whether
    /// the idle dot draws grey (seen) or blue (unseen output).
    ///
    /// ALWAYS serialized, `host_identity`'s shape and reason exactly: a
    /// client must be able to tell "this helm says never seen" (`null`) from
    /// "this helm predates the field" (key absent), because only the former
    /// may draw the unseen-blue dot and offer the read/unread toggle — an
    /// old helm's idle rows draw the SAME grey every other idle row does;
    /// what they lose is only the toggle and the possibility of the blue
    /// variant, never a distinct legacy colour.
    pub seen_activity_at: Option<i64>,
    /// Shared helm pin membership. The sidebar partitions on this flag;
    /// the served sort stays intact for clients such as the quick switcher.
    pub pinned: bool,
    /// Shared manual position, absent for an unpinned row. Positions can tie
    /// while Replace keeps both source and replacement; clients break ties
    /// by host number, then bare session ID. The served sort ignores it.
    pub pin_position: Option<i64>,
    /// How far through this session's notifications every client of this
    /// helm has read (`store::HelmStore::notification_marks`): a
    /// notification whose sequence number is above it is unread, which
    /// decides whether the row's bell shouts and which entries its list sets
    /// apart (SPEC.md, Status). Cleared notifications never reach the row at
    /// all: the helm's row builder drops them from `info.notifications`, so a row with
    /// none left shows no bell.
    #[serde(default)]
    pub notifications_read_through: u64,
}

/// The whole merged list, in the JSON shape `GET /api/sessions` answers
/// with: one array, its two counts, and whether a cap cut it.
///
/// `sessions`/`total`/`truncated` are the keys the UI has read since the
/// list first existed; `matching` arrived with filtering. There is no
/// cursor and no page size, by contract (SPEC.md's Session list section).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionList {
    /// Every row of the view that satisfies the filter, in the requested
    /// order — cut at [`crate::LIST_SESSIONS_CAP`] only when `truncated` says so.
    pub sessions: Vec<SessionRow>,
    /// How many sessions the VIEW holds, filter or no filter — the
    /// denominator the UI's "N matching of M sessions" prints.
    ///
    /// Deliberately does not move when the user types: a denominator that
    /// tracked the filter would compare a number against itself. A cached row whose
    /// payload no longer decodes is in neither the rows nor this count: it
    /// is dropped at the read with a warning (`store::CachedRow`), so the
    /// counts always describe rows a client can see.
    pub total: u64,
    /// How many rows of the view satisfy the caller's filter, present
    /// exactly when a predicate is active (`!filter.is_empty()`).
    ///
    /// Equal to `sessions.len()` unless the cap cut the reply — the count
    /// is over the whole view, the array is what fits — so a client that
    /// prints "showing N of M matching" has both numbers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matching: Option<u64>,
    /// Whether the client is NOT looking at the whole view: some host's
    /// list was cut at the wire's cap (a fact kept with that host's cache,
    /// so it holds for a host that has since gone down and across a helm
    /// restart), or the merged, filtered, sorted array hit the cap. This is
    /// the one signal behind SPEC.md's "could not read to the end" notice;
    /// nothing else produces it.
    pub truncated: bool,
}

/// The client preference this helm remembers for every client at once
/// (SPEC.md, Session list): the chosen list order, session the user last
/// selected, compact-row choice, and structured-launch defaults. One row, one shape — the stored row and
/// the `GET` reply.
///
/// Choice fields are `Option` because "never set" is a real state: the
/// client's default applies. `list_sort` is the bare `?sort=` word
/// (the helm's `parse_sort_key`), stored as the word rather than the enum so an
/// unset value has an honest representation; `last_selected` is a full
/// entry-bound session reference, valid for this helm's fleet only, which is
/// why the helm's own database and not client storage is where it belongs. A `PUT` sends a
/// `PreferencePatch` (helm), not this type: a patch has to tell "leave alone"
/// from "clear", and a plain `Option` cannot.
///
/// The remembered permission is a server-observed fact, unlike the
/// client-declared list fields. The helm writes them as side effects of
/// successful structured launches
/// (`record_create_history_with_destination`) — see that function's doc for why
/// origin-gating them to user-initiated launches matters. The wire route still
/// accepts them on `PUT` like the others (kept uniform with the rest of this
/// type rather than carved into a read-only exception), but no shipped
/// client sends it. Workspace-trust memory is retired: old wire writes are
/// accepted and ignored, and its retained database column is never read.
/// Host setup and removal confirmation choices are
/// helm-wide opt-outs: an explicit permanent answer is shared by every
/// client, while an unset field keeps asking. Feedback contact is a client
/// choice recorded only after a successful send, and explicit null forgets it.
/// Terminal link ownership is also helm-wide; an unset or unknown word selects
/// interception without changing file transfers or program URL forwarding.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    /// The place from the last admitted container create, including raw commands.
    /// Maintained from captured launch intent. A missing host stays a stale ID
    /// until another container launch replaces it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_container_host: Option<i64>,
    /// Only true authorizes the helm to update eligible remote hosts without a window.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_update_remote_hosts: Option<bool>,
    /// Only true authorizes desktop loopback tunnels; unset and false both disable them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tunnel_consent: Option<bool>,
    /// Shared opt-out for the managed-checkout Claude trust hint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude_trust_hint_dismissed: Option<bool>,
    /// Shared opt-outs for each agent's managed-checkout trust hint. Unset
    /// and false both allow the hint, independently of the trust switch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codex_trust_hint_dismissed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub muse_trust_hint_dismissed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pi_trust_hint_dismissed: Option<bool>,
    /// Dismissal shared by every client of this helm. Unset means unseen,
    /// including existing helms upgraded from before the welcome card.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub welcome_seen: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_sort: Option<String>,
    /// Full entry-bound reference. A deleted entry or session may leave this
    /// stale; readers choose a fallback rather than treating it as routing authority.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_selected: Option<String>,
    /// Most recently visited full references, newest first and without duplicates.
    /// The helm maintains this from selection writes; clients cannot replace it.
    /// An upgrade starts empty even when a legacy last selection survives.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub visit_history: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compact: Option<bool>,
    /// The wire word for the permissions mode of the last successful
    /// structured launch (a `LaunchPermission` wire word, see
    /// the helm's `is_known_remembered_permissions_word`), or `None` when nothing is
    /// remembered. A word this build does not recognize reads back as
    /// `None` (see the helm store's `preferences` read) rather than being forwarded
    /// to a client that could not act on it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remembered_permissions: Option<String>,
    /// Last explicitly launched prefix for each agent type. An absent key
    /// means New should start without a wrapper for that type.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub remembered_wrapper_prefixes: std::collections::BTreeMap<crate::LaunchHarness, Vec<String>>,
    /// Retired wire field. Reads are always unset; the historical database
    /// column is retained, but trust now comes from managed-checkout Settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remembered_workspace_trust: Option<bool>,
    /// Whether host removal skips its confirmation dialog for this helm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_host_remove_confirmation: Option<bool>,
    /// Whether host setup skips its confirmation dialog for this helm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_host_setup_confirmation: Option<bool>,
    /// Everyday session prompts may be skipped, but their guards still apply.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_session_confirmations: Option<bool>,
    /// Whether quick launches offer naming while the agent starts. Unset is enabled,
    /// so upgrading an existing helm has the same default as a fresh installation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_session_on_quick_launch: Option<bool>,
    /// Color each session's terminal frame with its host identity color.
    /// Unset stays off for fresh and upgraded helms.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_terminal_frames: Option<bool>,
    /// Final extensions the desktop helm may open after saving a download.
    /// NULL uses the default policy; an empty string disables opening.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_extensions: Option<String>,
    /// One shared template name, independent of any editor's saved fields.
    /// A missing or deleted name is handled as no quick launch by clients.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quick_launch_template: Option<String>,
    /// Shared physical digit bindings, independent of the quick-launch mark.
    /// Missing names are allowed so a stale binding can be repaired in Templates.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub template_shortcuts: std::collections::BTreeMap<String, String>,
    /// Contact from the last successful feedback send that opted into reuse.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feedback_contact: Option<String>,
    /// Which terminal web-link clicks the client owns. None means intercept;
    /// file downloads and program-initiated URL forwarding are independent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_link_click: Option<String>,
    // The seven fields below hold the settings the old app kept per device.
    // The maintainer decided on 2026-11-13 that they become shared helm
    // preferences with the new desktop app, starting unset: nothing is
    // carried over from the old app's per-device storage.
    /// The window's appearance word: `system`, `dark` or `light`, the old
    /// app's words. Unset (and any word the helm does not know, which it
    /// reads back as unset) means `system`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub appearance: Option<String>,
    /// The sidebar's resize width in whole logical pixels, within
    /// [`SIDEBAR_WIDTH_RANGE`] (SPEC.md, "Session list"): the width with the
    /// 56px collapsed rail included, which keeps its meaning whether or not
    /// the rail is expanded (expanding adds 164px on screen, not here). Unset
    /// means the 396px default; the helm reads a stored value outside the
    /// range back as unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sidebar_width: Option<u32>,
    /// Whether the host rail is expanded. Unset means collapsed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_rail_open: Option<bool>,
    /// The waiting sound switch (SPEC.md, "Status"). Unset means on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sound_waiting: Option<bool>,
    /// The approval-request sound switch. Unset means on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sound_approval: Option<bool>,
    /// The turn-finished sound switch. Unset means off.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sound_turn_finished: Option<bool>,
    /// Terminal text size in points, within [`TERMINAL_TEXT_SIZE_RANGE`].
    /// Unset means 14; the helm reads a
    /// stored value outside the range back as unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_text_size: Option<u32>,
}

/// The widths [`Preferences::sidebar_width`] may hold, in logical pixels
/// (SPEC.md, "Session list": the drag range of the sidebar's right edge).
/// The helm refuses a write outside it rather than clamping, so a client bug
/// shows up as a refused save instead of a silently different width.
pub const SIDEBAR_WIDTH_RANGE: std::ops::RangeInclusive<u32> = 296..=656;

/// The sizes [`Preferences::terminal_text_size`] may hold, in points: the
/// existing steps, every whole point size from 9 to 28 (SPEC_impl.md, "GPUI
/// cookbook: traps", the glyph atlas; SPEC.md says only "a fixed range").
/// Refused outside, like [`SIDEBAR_WIDTH_RANGE`].
pub const TERMINAL_TEXT_SIZE_RANGE: std::ops::RangeInclusive<u32> = 9..=28;

// Host entries.

/// One registered host as `GET /api/hosts` reports it.
///
/// Flat rather than nested under a `registry`/`connection` split: every
/// consumer of this shape renders one row per host, and a split would only
/// make the common case — "name it and chip its state" — reach through two
/// objects.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostView {
    pub id: i64,
    /// Agent discovery uses this captured entry identity; ordinary host JSON keeps
    /// its existing integer IDs and display labels.
    ///
    /// Helm-side only and never on the wire: the GUI sees an empty string.
    #[serde(skip)]
    pub entry_uuid: String,
    /// `"local"` for the one reserved row, `"ssh"` for a registered
    /// destination, `"tensorlake"` for a sandbox, or `"podman"` for a
    /// session-owned container Farhelm created. The
    /// distinction is not cosmetic: it is what tells a client which rows
    /// offer edit and remove at all, and what removing one does.
    pub kind: String,
    /// Session launch ownership changes the resource's lifecycle, not its
    /// inclusion in Hosts or its ordinary diagnostics and update eligibility.
    #[serde(default)]
    pub is_session_owned: bool,
    /// The sandbox behind a `"tensorlake"` host, for its details. Left out of
    /// the JSON for every other host, so their shape is unchanged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tensorlake: Option<TensorlakeHostView>,
    /// Public ownership/placement facts, without execution coordinates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<crate::EnvironmentHostView>,
    /// `None` for the local row, always `Some` for an ssh row.
    pub destination: Option<String>,
    /// The stored optional alias, kept distinct from `name` so clients can
    /// edit it without trying to reverse the derived display label.
    pub alias: Option<String>,
    /// The helm's display-name rendering — the same string session rows
    /// carry in `host_name`, so a row and its host chip never disagree.
    pub name: String,
    /// The identity the REGISTRY holds for this host: the one recorded at
    /// first contact, or accepted by an adoption. `None` if the host has
    /// never been reached, or reports none at all.
    ///
    /// Deliberately not "what the host is reporting". During an
    /// identity-mismatch those are two different values, and the difference
    /// is the whole decision the user is being asked to make — the reported
    /// one rides `state` (as `identity-mismatch`'s `reported`), where it is
    /// paired with the `recorded` value it contradicts. A renderer that
    /// took this field for "what is out there" would show the dead install's
    /// identity beside a prompt about the new one.
    pub identity: Option<String>,
    pub remote_farhelm: Option<String>,
    pub remote_state_dir: Option<String>,
    /// Whether the host starts YOLO sessions without asking; `false` (ask first) until the
    /// user changes it. A row missing from the registry read reports that confirmation is
    /// required.
    pub yolo_without_asking: bool,
    /// Whether the helm carries out acting `farhelm` commands from this host's sessions
    /// without asking the user; `false` (ask first) until the user changes it. A row missing
    /// from the registry read reports that asking is required, like the YOLO setting.
    pub commands_without_asking: bool,
    /// Default-off authority for session actions confined to the requesting host.
    pub own_sessions_without_asking: bool,
    /// Browser permissions are independent of command authority and each other.
    pub links_without_asking: bool,
    pub local_links_without_asking: bool,
    /// Helm-owned identity choices, independent of the actor's connection.
    pub icon: crate::host_appearance::HostIcon,
    /// Identity tint only; connection and permission colors stay separate.
    pub color: crate::host_appearance::HostColor,
    pub state: HostStateView,
    /// Which CONNECTION this host is on — an opaque, monotonic token that
    /// changes whenever the host's client does, including when it goes away
    /// (see `manager::ActorStatus::incarnation`).
    ///
    /// Served so a client can tell one connection to a host from the next
    /// without interpreting the state payload — the UI's provisioning panel
    /// binds a displayed plan to it, so a plan prepared against one
    /// connection is not left actionable on another. Opaque as a USAGE
    /// convention: compare it, never interpret it — the number is a counter
    /// over this helm's own connections and means nothing across a restart,
    /// which is exactly why a client must not persist one.
    ///
    /// `0` is "never connected".
    pub incarnation: u64,
    /// This helm's last observed Trash change, independent of session status.
    pub trash_epoch: u64,
}

impl HostView {
    /// The key the UI channel files the host with registry id `id` under in its hosts collection: the id in decimal,
    /// as the helm spells a numeric key. A session row names its host by that id (`SessionRow::host`), so this is how a
    /// client finds the row's host without parsing keys itself.
    pub fn key(id: i64) -> String {
        id.to_string()
    }

    /// Whether this is the helm's own machine: the one reserved row whose
    /// `kind` is `"local"`. One place for that spelling, so a client does not
    /// compare the wire's kind words itself.
    pub fn is_local(&self) -> bool {
        self.kind == "local"
    }
}

/// A Tensorlake host's sandbox as `GET /api/hosts` reports it: its name in
/// Tensorlake, the recipe it was made from, and the size it was created
/// with (`None` when the stored size cannot be read).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TensorlakeHostView {
    pub sandbox: String,
    pub recipe: String,
    pub size: Option<crate::tensorlake::recipes::RecipeSize>,
    /// The connected phase's forwards, absent without a lease. Their
    /// failure never claims the supervisor itself is disconnected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tunnels: Option<crate::tensorlake::TunnelView>,
    /// Delivery is independent of connectivity and can fail during setup too.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_warning: Option<String>,
}

/// How one host's `HostState` (the helm's connection state) crosses the REST boundary.
///
/// Internally tagged by `phase`, whose values are exactly
/// the helm's `HostState::phase`'s stable labels — the same strings the diagnostic
/// trail logs. That equality is deliberate and load-bearing: a UI chip, a
/// log line, and this JSON must key off ONE vocabulary, or a state that
/// gains a payload field starts reading as a different phase somewhere.
///
/// Every variant carries its own evidence rather than a shared error
/// string, because SPEC.md requires errors to be actionable: "unreachable"
/// and "the host runs an older farhelm" call for opposite responses, and a
/// renderer cannot invent the difference from prose.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "phase")]
pub enum HostStateView {
    /// Inside the active-retry window, including the wait between attempts.
    #[serde(rename = "connecting")]
    Connecting {
        attempt: u32,
        /// Closed vocabulary for the currently observed boundary; no live SSH output.
        stage: ConnectionStage,
        last_error: Option<String>,
    },
    /// The active window is spent; background re-probing continues
    /// forever, so no user action is required for the host to come back.
    #[serde(rename = "unreachable-reprobing")]
    Unreachable {
        /// `"local-supervisor-not-running"` is the one cause the user can
        /// fix with a command on the machine they are already at, and the
        /// UI's manual-start hint keys off it. Everything else is
        /// `"transport-failure"` with the transport's own text beside it.
        cause: String,
        last_error: String,
    },
    #[serde(rename = "connected")]
    Connected {
        identity: Option<String>,
        build_version: String,
        /// Whether the peer's parseable build is older than this helm's
        /// build. `false` also represents an unknown age when either side
        /// does not publish a valid semantic version.
        old_version: bool,
        /// Whether the peer's parseable build is NEWER than this helm's: the
        /// host list labels such a host "too new" (SPEC.md, host version
        /// advisories) and Update refuses it, since an Update would install
        /// this helm's older build over it. `false` also represents an
        /// unknown age, as for `old_version`.
        newer_version: bool,
        refresh: RefreshView,
    },
    /// Refused at the hello: both versions are named so the user can see
    /// WHICH side is behind, and `remediation` is the sentence to act on.
    #[serde(rename = "version-skew")]
    VersionSkew {
        peer_protocol: u32,
        peer_build: String,
        our_protocol: u32,
        our_build: String,
        remediation: String,
    },
    /// Frozen awaiting a user decision: adopt the reported identity
    /// (`POST /api/hosts/{id}/adopt`) or fix the destination. Nothing is
    /// connected and nothing is written while this holds.
    #[serde(rename = "identity-mismatch")]
    IdentityMismatch { recorded: String, reported: String },
    /// Frozen because the destination answered without an identity at all
    /// while this entry has one on record — so there is nothing to compare
    /// and, deliberately, nothing to adopt.
    ///
    /// A renderer must NOT offer the adopt verb here (it would be refused,
    /// and the offer itself would be a lie about what is on the table): the
    /// remedies are fixing the host so it reports its identity, retargeting
    /// the entry, or removing it. Re-probed automatically, so a host that
    /// starts identifying itself again recovers unaided.
    #[serde(rename = "identity-unverified")]
    IdentityUnverified { recorded: String },
    /// This ENTRY reaches a machine another entry already holds. Nothing is
    /// connected through it; the user removes the other entry or changes
    /// this one's destination, then presses Retry.
    #[serde(rename = "duplicate")]
    Duplicate { twin: i64, identity: String },
    /// Farhelm is handling an explicit lifecycle request under a hold. The
    /// direction records intent, not a confirmed suspension. The unix-ms
    /// request time is display metadata; reconnection after Resume uses the
    /// ordinary Connecting phase.
    #[serde(rename = "tensorlake-changing")]
    TensorlakeChanging {
        starting: bool,
        direction: crate::tensorlake::LifecycleDirection,
        requested_at: i64,
    },
    /// A Tensorlake host whose sandbox Tensorlake reports suspended (or
    /// failed to resume); nothing is dialed until it runs again. Times are
    /// unix ms: `observed_at` is when this helm first saw the suspension,
    /// `suspended_by_farhelm_at` when Farhelm's own Suspend finished (carried
    /// by the confirmed state and persisted on the host row for recovery;
    /// `None` when Farhelm did not suspend it, which is how the UI tells
    /// "suspended from Farhelm"
    /// from an idle timeout). `resume_failed` is Tensorlake's diagnosis of a
    /// failed resume.
    #[serde(rename = "suspended")]
    Suspended {
        observed_at: i64,
        suspended_by_farhelm_at: Option<i64>,
        resume_failed: Option<String>,
    },
    /// Stopped retains files without pretending paused processes survive.
    #[serde(rename = "stopped")]
    Stopped {
        start_failed: Option<String>,
        waiting_for_place: bool,
    },
    /// No actor is running for this row — it retired with its registry row,
    /// or its task panicked. Reported rather than hidden so an operation
    /// refused against it has something honest to name.
    #[serde(rename = "retired")]
    Retired { reason: String },
    /// The host's sandbox or container is being destroyed; `failure` is the
    /// provider's reason when that failed and waits for Retry.
    #[serde(rename = "deleting")]
    Deleting { failure: Option<String> },
}

impl HostStateView {
    /// This state's stable phase label — the same strings
    /// the helm's `HostState::phase` produces and the same strings the `#[serde(
    /// rename)]`s above put on the wire.
    ///
    /// A third spelling of one vocabulary, which this type's own docs warn
    /// against, and it earns the exception by being the only way a
    /// non-serializing consumer can reach the word. The agent relay needs
    /// the label as a Rust string (`crate::agent_requests`), and the
    /// alternatives were worse: re-deriving it from the manager's snapshot
    /// would bypass the view every other surface renders from, and
    /// round-tripping through `serde_json` to read a tag back out is a
    /// vocabulary coupling expressed as an allocation. What keeps the copy
    /// honest is `phase_matches_the_serialized_tag`, which compares this
    /// function's answer to the encoder's for every variant.
    pub fn phase(&self) -> &'static str {
        match self {
            HostStateView::Connecting { .. } => "connecting",
            HostStateView::Unreachable { .. } => "unreachable-reprobing",
            HostStateView::Connected { .. } => "connected",
            HostStateView::VersionSkew { .. } => "version-skew",
            HostStateView::IdentityMismatch { .. } => "identity-mismatch",
            HostStateView::IdentityUnverified { .. } => "identity-unverified",
            HostStateView::Duplicate { .. } => "duplicate",
            HostStateView::TensorlakeChanging { .. } => "tensorlake-changing",
            HostStateView::Suspended { .. } => "suspended",
            HostStateView::Stopped { .. } => "stopped",
            HostStateView::Retired { .. } => "retired",
            HostStateView::Deleting { .. } => "deleting",
        }
    }
}

/// How a connected host's most recent cache refresh went.
///
/// Beside the connection rather than inside it, mirroring
/// the manager's `RefreshHealth`: a failed refresh does not disconnect a host, and
/// collapsing the two would make a host that is answering perfectly well
/// read as unreachable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status")]
pub enum RefreshView {
    /// Connected, first refresh still in flight.
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "ok")]
    Ok { sessions: usize },
    /// The last refresh failed; the PREVIOUS cache is still in place, so
    /// this host's rows are still listed.
    #[serde(rename = "failed")]
    Failed { error: String },
}

/// The last connection boundary actually observed during one attempt.
///
/// These are stable REST vocabulary, not inferred diagnoses: opening SSH's
/// pipes alone cannot establish that a remote supervisor is listening.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConnectionStage {
    /// No attempt is running yet, or the retry's backoff has not elapsed.
    WaitingToConnect,
    /// SSH has started, but no evidence yet establishes a remote supervisor connection.
    ConnectingToHost,
    /// The local supervisor socket has not accepted the connection yet.
    ConnectingToSupervisor,
    /// A supervisor socket accepted the connection; its hello is still outstanding.
    WaitingForHello,
    /// Hello succeeded; identity recording and browser-link setup share this final stage.
    SettingUp,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The method's JSON spelling is wire format: the helm shipped before it
    /// was an enum and decoded these exact strings, so a renamed variant or a
    /// different `rename_all` would silently break every command. Pins each
    /// method's spelling both ways, that `as_str` agrees with serde, and that
    /// decoding is case-sensitive and closed: anything else must fail to
    /// decode, which is what makes the helm close the channel on it.
    #[test]
    fn each_method_round_trips_through_its_uppercase_spelling() {
        for (method, spelling) in [
            (Method::Get, "GET"),
            (Method::Post, "POST"),
            (Method::Put, "PUT"),
            (Method::Delete, "DELETE"),
            (Method::Patch, "PATCH"),
        ] {
            let json = serde_json::Value::String(spelling.to_string());
            assert_eq!(serde_json::to_value(method).unwrap(), json);
            assert_eq!(serde_json::from_value::<Method>(json).unwrap(), method);
            assert_eq!(method.as_str(), spelling);
        }
        for foreign in ["get", "Put", "OPTIONS", ""] {
            assert!(
                serde_json::from_value::<Method>(serde_json::Value::String(foreign.into()))
                    .is_err(),
                "{foreign:?} is not a method"
            );
        }
    }
}
