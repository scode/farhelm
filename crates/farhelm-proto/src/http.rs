//! Machine-read tokens the helm's HTTP API hands the browser UI.
//!
//! The helm's error bodies are prose shown verbatim, and its replies are JSON
//! the UI decodes into its own types, so some of the places where the UI
//! BRANCHES on something the helm sends are bare strings: a marker in an
//! error body, a header, a code in an error envelope, one label in the host
//! list. Each one used to be spelled separately in both crates with nothing
//! tying the two together: the helm's tests pinned its side, the UI's unit tests pinned its
//! own copy, and the browser suite that would notice a mismatch is not part of
//! CI. A rename on one side would therefore have compiled, passed, and quietly
//! switched the UI's decision to its fallback. Defining them once here makes
//! that rename a single edit.
//!
//! Not covered: whole enums the UI mirrors by hand from the helm's JSON (host
//! phases, provisioning step statuses and the like). Those are typed fields
//! of reply shapes the UI declares on its own, a wider arrangement than a
//! handful of tokens.
//!
//! These are wire contracts, not implementation details: changing one here is
//! a change to what the API promises. That is why tests that pin the exact
//! bytes keep spelling them out rather than importing these constants, so an
//! accidental edit here fails a test instead of silently moving both sides.

/// Response header marking a refused request as a failed precondition the
/// client should answer by re-reading, not by showing a permanent error.
///
/// The helm sets it on a 409 whose request named a connection the host is no
/// longer on (farhelm-helm's `precondition` module has the full contract), with
/// the value [`PRECONDITION_INCARNATION`]. A 409 WITHOUT it is some other
/// conflict and must not be answered by re-reading.
///
/// A header rather than a marker in the body because the body can carry a
/// remote supervisor's text verbatim, and a supervisor must not be able to
/// make the client act. Only the helm's own response code sets headers, so
/// this signal is the helm's alone.
pub const PRECONDITION_HEADER: &str = "x-farhelm-precondition";

/// The one value of [`PRECONDITION_HEADER`]: the request was prepared against
/// a connection that has since been replaced.
pub const PRECONDITION_INCARNATION: &str = "incarnation";

/// Response header that marks a failed create as provably never accepted.
///
/// Carried only when the helm can prove the create was not accepted: it failed
/// locally before any create frame was dispatched, or the supervisor refused
/// it durably ([`crate::ErrorKind::CheckoutConflict`]). That settles the
/// request's intent key, so the client may drop the attempt; a new one takes
/// another explicit submission. Its absence means "unresolved": the create may
/// have landed, and a retry must replay the same key. The only value it ever
/// carries is [`CREATE_OUTCOME_DEFINITELY_UNACCEPTED`].
pub const CREATE_OUTCOME_HEADER: &str = "x-farhelm-create-outcome";

/// The one value of [`CREATE_OUTCOME_HEADER`].
pub const CREATE_OUTCOME_DEFINITELY_UNACCEPTED: &str = "definitely-unaccepted";

/// The host-list `cause` for a local row whose supervisor is not running.
///
/// The one unreachable cause a user can fix with a command on the machine they
/// are already at, so the UI keys both its diagnosis and its manual-start
/// remedy off exactly this string. Every other unreachable host reports
/// `"transport-failure"`.
pub const LOCAL_SUPERVISOR_NOT_RUNNING: &str = "local-supervisor-not-running";

/// The `code` of the JSON body the helm's device-authentication boundary
/// answers an unauthenticated request with.
///
/// Only that middleware emits it. A remote supervisor's error text reaches the
/// client verbatim and could spell this JSON, but the helm never answers a
/// supervisor's refusal with 401 (farhelm-helm's `http_error` translates its
/// `Unauthorized` to 502), and the UI only reads this code off a 401.
pub const AUTH_REQUIRED_CODE: &str = "device_auth_required";

/// Response header carrying the helm's build version on every reply.
///
/// The UI compares it with its own build to notice that it is running
/// against a different helm build (version skew). Lowercase because the helm
/// builds the header name with `HeaderName::from_static`, which rejects
/// uppercase.
pub const BUILD_STAMP_HEADER: &str = "x-farhelm-build";
