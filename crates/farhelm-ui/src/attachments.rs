//! Paste and drop interception (PLAN_M4.md item 7): the attachment domain
//! of the session view — the classification rule, the naming rule, the
//! shell-safety rule for what gets inserted, the upload endpoint, and
//! every word the user reads when a transfer is running or has failed.
//!
//! ## Why the policy lives here and the plumbing lives in terminal.js
//!
//! A paste or a drop is a DOM event carrying `File` objects. Nothing in
//! Rust can see one: the web build is wasm behind Dioxus's own event
//! system (which does not surface clipboard payloads), and the desktop
//! build cannot even ask, because wry's eval channel is dead on
//! WKWebView (MT-5, `api::mint_lease`'s docs). So the runtime path —
//! listen, classify, upload, insert — is terminal.js's, next to the
//! xterm.js island that owns the terminal it happened to.
//!
//! What is left for Rust is everything that is a DECISION rather than a
//! DOM call, and that is what this module holds. [`attachment_policy`]
//! serializes it into the page as part of each `farhelmTerm.sync()`, so
//! terminal.js reads answers instead of recomputing them: which flavor
//! wins for a payload ([`classify`], shipped as a lookup table over all
//! eight possible payloads), what a pasted image is called
//! ([`image_extension_for`] plus [`GENERATED_NAME_PREFIX`]), which paths
//! have to be quoted before they are typed into a shell
//! ([`SHELL_SAFE_PATH_CHARS`]), what separates two inserted paths, and
//! the exact prose of every message an upload can put on screen —
//! including the ones that used to be JS string literals, so that no
//! sentence a user can read is authored outside this file.
//!
//! The couplings that remain are named where they exist rather than
//! pretended away. terminal.js indexes [`classify`]'s table with the bit
//! order documented on [`payload_index`]; substitutes `{name}`-style
//! placeholders into the strings below, in one pass; normalizes a MIME
//! type and derives an extension from it the way [`image_extension_for`]
//! does, reading this module's alias table for the exceptions; and
//! applies POSIX single quoting to any path [`SHELL_SAFE_PATH_CHARS`]
//! rejects. Each is a few lines on the JS side, and each is pinned end to
//! end by the browser suite's interception tests — the same arrangement
//! terminal.js already documents for `TAKEOVER_DETACH_REASON`.
//!
//! ## What SPEC.md pins, and what this module chooses
//!
//! SPEC.md fixes the precedence (file references, then image data, then
//! plain text) and makes interception of files and images unconditional.
//! What it leaves open is settled here, and all of it is visible to
//! users:
//!
//! - Which payloads count as "image data" rather than a file reference,
//!   and therefore get a generated name. See [`classify`]; the rule is
//!   deliberately narrow, and what falls on each side is spelled out
//!   there.
//! - A path is inserted with a TRAILING separator rather than an infix
//!   one. See [`PATH_SEPARATOR`].
//! - A path whose own characters a shell would split or expand is
//!   inserted QUOTED. See [`SHELL_SAFE_PATH_CHARS`].
//!
//! ## The manual desktop pass, and exactly what it has to cover
//!
//! PLAN_M4.md acceptance 9 records one manual run on the DESKTOP build,
//! because it is a capability check that no browser run can stand in for:
//! the Playwright suite drives the web build with a synthesized event
//! object (its own interception section says so in detail), so a real OS
//! drag and a real clipboard image have never been exercised anywhere.
//! What the operator has to do in the self-contained desktop app (a release
//! bundle, or the development bundle started by `scripts/desktop-smoke.sh`):
//!
//! 1. Drag a real file from the file manager onto the AGENT terminal of an
//!    open session. Expect: an "attaching …" line, then the host path
//!    inserted at the cursor, and the file present under the session's
//!    attachments directory with its bytes intact. This is the one that
//!    fails if wry swallows the drop — see `main.rs`'s
//!    `with_disable_drag_drop_handler` call and the audit trail on it.
//! 2. Do the same onto a TAB's terminal, with the agent terminal visible
//!    beside it. Expect the path in the tab and nothing in the agent
//!    terminal.
//! 3. Take a real screenshot to the clipboard and paste it into a terminal.
//!    Expect the engine's real filename when it supplies one, or a
//!    `pasted-<n>.png` name for synthetic image data, and the same insertion.
//!    This is the SPEC.md acceptance walkthrough's own step, and the one with
//!    the most engine-specific clipboard behavior behind it. Expand the
//!    on-screen clipboard-facts dump, then check the other side of
//!    [`classify`]'s naming rule: copy an
//!    image FILE in the file manager, paste it, and expect its own name to
//!    survive rather than a generated one. Record what each engine
//!    actually put on the clipboard (the `File`'s name and `lastModified`,
//!    and the item order) — that observation is what this rule was
//!    designed against and what a future revisit needs.
//! 4. Drag a FOLDER onto a terminal. Expect a visible rejection and no
//!    upload. If a supported engine turns out to deliver directory drops
//!    as readable `File`s — neither detectable through the drag-entry API
//!    nor failing when its bytes are read — that is exactly the "real
//!    deficiency in our actual flows" SPEC_impl.md's clipboard risk
//!    reserves the native wry hooks for: the rejection promise stays
//!    unconditional, only the mechanism escalates. It is a finding to
//!    escalate under that clause, never a test to relax.
//! 5. Repeat step 3 against a REMOTE session (`--ssh`) with a
//!    representative screenshot, and write the observed paste-to-path
//!    latency into the record. SPEC.md promises "for a typical screenshot
//!    this is imperceptible", which no local run can vouch for; a measured
//!    number keeps the promise honest without inventing a CI gate for a
//!    subjective bar.
//!
//! The desktop build's own cross-origin problem — the page runs on wry's
//! custom scheme while the helm answers on loopback — is handled rather
//! than left for this pass to discover: the helm answers the attachments
//! route with CORS headers for exactly the custom-scheme origins its
//! loopback guard already trusts (farhelm-helm's `desktop_webview_cors`). Step
//! 1 failing with the file present on disk but the UI reporting an error
//! would be that mechanism regressing.

use crate::api::encode_path_segment;

/// The prefix of the name a pasted image is uploaded under (PLAN_M4.md
/// item 7's `pasted-<n>.png` shape; terminal.js appends a per-page counter
/// and the extension [`image_extension_for`] chooses).
///
/// Deliberately distinct from the supervisor's own `attachment-` fallback
/// prefix (farhelm-supervisor's `attachments::GENERATED_NAME_PREFIX`).
/// That one is for a proposal that carried no usable name at all and gets
/// no extension, because the supervisor knows the byte count and nothing
/// else. This one is for bytes the client KNOWS are an image of a
/// particular type, so it can say so in the name — which is the whole
/// point, since the agent reading the path is what decides what to do with
/// the file.
///
/// Uniqueness is not this prefix's job: two pastes in two views can both
/// mint `pasted-1.png`, and the supervisor's collision suffixing publishes
/// them under distinct paths. The counter exists so that several pastes in
/// ONE view read differently on screen, not as an identity.
pub(crate) const GENERATED_NAME_PREFIX: &str = "pasted-";

/// The stem an engine uses for the `File` it synthesizes around raw
/// clipboard image data — Chromium and WebKit both hand over
/// `image.<ext>`, matching the payload's own MIME type.
///
/// Load-bearing for [`classify`]'s naming rule: a payload whose only name
/// is this exact synthesized one carries no name the USER chose, which is
/// what makes a generated name an improvement rather than a loss. Any
/// other name is the user's (or their filesystem's) and is kept.
pub(crate) const PLACEHOLDER_NAME_STEM: &str = "image";

/// How recently a `File` must claim to have been modified for its
/// placeholder name to be believable as freshly synthesized clipboard
/// data, in milliseconds.
///
/// The second half of the narrowing above, and the reason it is safe to
/// treat a name as a placeholder at all: an engine synthesizing a `File`
/// for clipboard bytes stamps `lastModified` at the moment of the paste,
/// while a real file carries its own mtime. A user's genuine `image.png`
/// would have to have been written in the last few seconds to be
/// misclassified, and the cost even then is a name, never a byte.
///
/// Five seconds rather than a tighter bound because the clock being
/// compared is the PAGE's, against a value the engine stamped: a loaded
/// machine can put a second or two between them, and a bound that
/// occasionally misfires would send screenshots under `image.png` — the
/// exact outcome the rule exists to prevent.
pub(crate) const PLACEHOLDER_MAX_AGE_MS: u64 = 5_000;

/// What terminal.js inserts after each host path.
///
/// A TRAILING separator, not one placed between paths, and that is the
/// interesting half. Both readings satisfy PLAN_M4.md item 7's
/// "space-separated", but an infix separator only knows about the paths of
/// its OWN drop: drag one file in, then drag a second, and the two paths
/// arrive with nothing between them —
/// `/state/attachments/s/a.png/state/attachments/s/b.png` — which is one
/// unusable string rather than two paths. A trailing separator has no such
/// state to get wrong, and it leaves the cursor where the user's next word
/// goes.
///
/// A single space specifically, because the inserted text is terminal
/// input that a shell will word-split.
pub(crate) const PATH_SEPARATOR: &str = " ";

/// Every character a published path may contain and still be typed into a
/// shell unquoted.
///
/// The supervisor sanitizes the FILENAME to a shell-safe set (its
/// `attachments::sanitize`), and that is not enough: the published path is
/// `<state-dir>/attachments/<session-id>/<name>`, and `--state-dir` is the
/// user's own argument. A state directory under `~/Library/Application
/// Support/…`, or any path with a space, an apostrophe, or a `$` in it,
/// produces a perfectly valid published path that a shell would split into
/// several words or expand — so the agent would be handed a filename that
/// does not exist, or worse, run whatever a `$(…)` in the path expanded
/// to.
///
/// So terminal.js checks the WHOLE path against this set and, when
/// anything falls outside it, wraps the path in POSIX single quotes
/// (embedded quotes closed and re-opened the usual `'\''` way). The SET is
/// the rule — shipped as data rather than as a pattern or a function,
/// because a character set is the one form both languages can read the
/// same way, and because the alternative (a regular expression, or a Rust
/// predicate the page cannot call) would be a second transcription of it
/// to keep in step. What the test module checks is this data against real
/// path shapes; what the browser suite checks is that a quoted path
/// reaches a real shell as one word.
///
/// Conservative on purpose: `~`, `#`, and `!` are all safe in the middle
/// of a word and are excluded anyway, because their safety depends on
/// position and on which shell is reading. An unnecessary pair of quotes
/// costs nothing; a missing pair costs the user their attachment.
pub(crate) const SHELL_SAFE_PATH_CHARS: &str = concat!(
    "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
    "abcdefghijklmnopqrstuvwxyz",
    "0123456789",
    "_@%+=:,./-",
);

/// The extension used when a MIME type yields nothing usable — anything
/// that is not an image type at all, or an image subtype with no
/// alphanumeric characters in it.
///
/// `bin` rather than a plausible-looking `png`: the name is a convenience
/// for whoever reads the path, and naming unknown bytes after a format
/// they are probably not is a small lie that costs someone a confusing
/// debugging session. The bytes are published unchanged either way.
pub(crate) const FALLBACK_EXTENSION: &str = "bin";

/// The longest extension [`image_extension_for`] will derive from a
/// subtype before giving up on it.
///
/// A vendor subtype can be arbitrarily long, and a forty-character
/// extension is not one — past this bound the derived string stops being a
/// recognizable file type and starts being noise glued to the user's
/// filename.
const MAX_DERIVED_EXTENSION: usize = 12;

/// Image subtypes whose conventional file extension is not the subtype
/// token itself, as `(subtype token, extension)` pairs.
///
/// The token is what [`image_extension_for`]'s derivation produces BEFORE
/// this table is consulted (parameters and `+suffix` dropped, vendor
/// prefix reduced to its last segment, `x-` stripped, non-alphanumerics
/// removed), which is why `image/x-icon` and `image/vnd.microsoft.icon`
/// both arrive here as `icon`.
///
/// Shipped to the page, because terminal.js has to derive extensions for
/// types no list can enumerate and this is the only part of the rule that
/// is data rather than algorithm.
const EXTENSION_ALIASES: &[(&str, &str)] = &[("jpeg", "jpg"), ("icon", "ico")];

/// One payload's winning interpretation — SPEC.md's flavors, plus the
/// "nothing to do" case a payload with no usable content lands in.
///
/// Exactly one of these applies to a payload, which is the property that
/// matters: a drag carrying a file AND the text of its own path must
/// produce ONE upload and ONE insertion, never both an upload and a
/// pasted path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Flavor {
    /// One or more actual file-system objects — dragged files, or file
    /// references on the clipboard. Every one of them is uploaded, each
    /// under its own name.
    File,
    /// Raw image bytes with no file behind them: a screenshot on the
    /// clipboard. Uploaded under a generated name.
    Image,
    /// Plain text, including text that merely looks like a path. Passes
    /// through as ordinary terminal input.
    Text,
    /// Nothing this UI knows how to act on. Not an error and not reported
    /// as one — an empty drag or a clipboard holding only, say, a custom
    /// application format is a no-op, not a failure.
    None,
}

impl Flavor {
    /// The spelling terminal.js matches on. Kept as an explicit method
    /// rather than a serde rename so the wire vocabulary is visible at the
    /// one place that defines it.
    fn wire_name(self) -> &'static str {
        match self {
            Flavor::File => "file",
            Flavor::Image => "image",
            Flavor::Text => "text",
            Flavor::None => "none",
        }
    }
}

/// SPEC.md's precedence order, applied to what a payload carries.
///
/// The rule is a strict order — file references beat image data beats
/// plain text — and the reason it needs stating at all is that real
/// payloads carry several at once. Dragging a file out of a file manager
/// hands over the file AND its path as `text/plain`; copying an image file
/// hands over the file AND a rendering of it as raw image bytes. Without a
/// precedence rule those would upload a file and paste its path, or upload
/// the same picture twice under two different names.
///
/// ## What counts as image DATA rather than a file reference
///
/// This is the one judgment call, and it decides only one thing: whether
/// the upload keeps a name or gets a generated one. Both are intercepted
/// and both are published either way.
///
/// An entry is image data when all of the following hold, and a file
/// reference otherwise:
///
/// - it arrived on the CLIPBOARD (a drag always carries real filesystem
///   entries, so a dragged image is a file, full stop);
/// - its MIME type is an image type; and
/// - it carries no name the user chose — either none at all, or exactly
///   the placeholder the engine synthesizes for clipboard bytes
///   ([`PLACEHOLDER_NAME_STEM`] plus this type's own extension) on a
///   `File` stamped within [`PLACEHOLDER_MAX_AGE_MS`] of now.
///
/// So: a screenshot on the clipboard is image data and becomes
/// `pasted-1.png`. An image FILE copied in a file manager is a file
/// reference and keeps `holiday.png`, because its name is not the
/// placeholder and its `lastModified` is its own. A user's genuine file
/// that happens to be called `image.png` AND was written seconds ago is
/// the one case the rule gets wrong, and it costs a name rather than a
/// byte. An earlier version of this rule used the clipboard alone and got
/// that case backwards for every copied image file, which is why the
/// narrowing exists; the manual desktop pass (this module's header, step
/// 3) is what records what the engines actually put on the clipboard for
/// each.
///
/// ## Why image data loses to a file rather than uploading alongside it
///
/// When a payload carries both, they are two REPRESENTATIONS of one thing
/// — copying an image file puts the file reference and a rendering of its
/// contents on the clipboard together — so uploading both would publish
/// the same picture twice under two names for one paste. Precedence exists
/// to pick one. That is not the same as dropping a distinct file: several
/// real files in one payload all count toward `has_file` and are ALL
/// uploaded, which is what the narrowing above makes possible (an image
/// file is a file reference, so it is one of them).
///
/// Directories are folded into `has_file` by terminal.js rather than given
/// a flavor of their own: a dragged folder IS a file-system object, so it
/// must outrank the `text/plain` copy of its path that the same drag
/// carries. It is then rejected instead of uploaded — SPEC.md's
/// unconditional rejection — which is a decision about what to DO with the
/// winning flavor, not about which flavor wins.
pub(crate) fn classify(has_file: bool, has_image: bool, has_text: bool) -> Flavor {
    if has_file {
        Flavor::File
    } else if has_image {
        Flavor::Image
    } else if has_text {
        Flavor::Text
    } else {
        Flavor::None
    }
}

/// Where a payload carrying these three flavors sits in the table
/// [`classification_table`] ships.
///
/// The bit order is a cross-language contract: terminal.js computes the
/// same index from its own booleans and reads the answer out of the array,
/// which is what keeps the precedence rule from being implemented twice.
/// File is the high bit, text the low one — the same order the rule itself
/// reads in.
fn payload_index(has_file: bool, has_image: bool, has_text: bool) -> usize {
    usize::from(has_file) << 2 | usize::from(has_image) << 1 | usize::from(has_text)
}

/// [`classify`]'s answer for every payload there is, in [`payload_index`]
/// order.
///
/// Eight entries is the whole domain, so shipping the table rather than
/// the rule is not a compression trick — it means the page never has to
/// re-derive anything, and a change to the precedence order is a change to
/// `classify` alone.
fn classification_table() -> Vec<&'static str> {
    let mut table = vec![Flavor::None.wire_name(); 8];
    for has_file in [false, true] {
        for has_image in [false, true] {
            for has_text in [false, true] {
                table[payload_index(has_file, has_image, has_text)] =
                    classify(has_file, has_image, has_text).wire_name();
            }
        }
    }
    table
}

/// A MIME type reduced to the form the rules below match on: lowercased,
/// with any parameters (`image/png; foo=bar`) and surrounding whitespace
/// dropped.
///
/// terminal.js normalizes the same way before deriving an extension.
fn normalized_mime(mime: &str) -> String {
    mime.split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
}

/// The file extension a pasted image of this MIME type is named with.
///
/// PLAN_M4.md item 7 says the extension comes FROM THE MIME TYPE, so this
/// derives one for any `image/*` rather than consulting a list of types
/// somebody remembered to add: the subtype is reduced to a plausible
/// extension token (`+xml` and other structured suffixes dropped, a vendor
/// tree reduced to its last segment, a leading `x-` stripped, anything
/// left that is not alphanumeric removed), and [`EXTENSION_ALIASES`]
/// corrects the handful whose conventional extension differs from that
/// token. `image/x-icon` becomes `ico`, `image/jpeg` becomes `jpg`,
/// `image/avif` becomes `avif` with no list involved.
///
/// [`FALLBACK_EXTENSION`] is reserved for types that genuinely yield
/// nothing: a non-image type, or a subtype with no alphanumerics in it. A
/// derived token longer than [`MAX_DERIVED_EXTENSION`] falls back too —
/// past that it is not an extension, it is a vendor string glued to a
/// filename.
pub(crate) fn image_extension_for(mime: &str) -> String {
    let normalized = normalized_mime(mime);
    let Some(subtype) = normalized.strip_prefix("image/") else {
        return FALLBACK_EXTENSION.to_string();
    };
    // `image/svg+xml` is an SVG; the structured-syntax suffix says how it
    // is encoded, not what it is.
    let subtype = subtype.split('+').next().unwrap_or("");
    // `image/vnd.microsoft.icon` is an icon: the vendor tree is
    // registration bookkeeping, and the last segment is the format.
    let subtype = subtype.rsplit('.').next().unwrap_or("");
    let subtype = subtype.strip_prefix("x-").unwrap_or(subtype);
    let token: String = subtype
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    if token.is_empty() || token.len() > MAX_DERIVED_EXTENSION {
        return FALLBACK_EXTENSION.to_string();
    }
    EXTENSION_ALIASES
        .iter()
        .find(|(from, _)| *from == token)
        .map(|(_, to)| (*to).to_string())
        .unwrap_or(token)
}

/// The helm endpoint one session's uploads POST to (the pinned attachment
/// REST contract: raw body, `?filename=` proposal, `{"path"}` back).
///
/// The session id is percent-encoded for the same reason every other id in
/// this UI is: it came from a supervisor, which under `--ssh` is a
/// different machine, and an id carrying path syntax must not be able to
/// choose which endpoint the browser posts a file to (see
/// `api::encode_path_segment`).
///
/// The `?filename=` value is NOT built here. It is a name from the user's
/// own filesystem or a generated one, known only at paste time, so
/// terminal.js appends it with `encodeURIComponent` — which is the correct
/// encoder for a query value and is not something a Rust-side string could
/// have prepared in advance.
pub(crate) fn attachment_upload_path(session_id: &str) -> String {
    format!(
        "/api/sessions/{}/attachments",
        encode_path_segment(session_id)
    )
}

/// The DOM id of one terminal's attachment status line — where an upload
/// in flight and a failed one are shown.
///
/// Derived from the terminal's own mount-point id (`tabs`'
/// `AGENT_TERMINAL_ELEMENT_ID` or `tab_terminal_element_id`) so that a
/// status line belongs to exactly one island: the element is rendered by
/// `SessionView` inside that terminal's own pane, and terminal.js writes
/// into it and clears it on unmount. It lives as long as the pane does,
/// which for a tab is until that tab leaves the strip and for the agent
/// terminal is the life of the view.
///
/// Separate from the detach BANNER on purpose. The banner is sticky for
/// the life of an attachment (its first message wins, so a takeover is
/// never overwritten by the close that follows it), which is exactly wrong
/// for a line that has to change as uploads start, finish, and fail.
pub(crate) fn attachment_status_element_id(terminal_element_id: &str) -> String {
    format!("attach-{terminal_element_id}")
}

/// While a transfer is running. `{name}` is the file being sent.
///
/// Deliberately unobtrusive (PLAN_M4.md item 7 asks for an indicator, not
/// a meter) and deliberately naming the file: with several terminals on
/// screen and a transfer that outlives a tab switch, "uploading" alone
/// would not tell the user what is going where.
const BUSY_ONE_TEXT: &str = "attaching {name}…";

/// While several transfers are running at once — a multi-file drop, or a
/// second drop landing on a terminal that is still sending the first.
/// `{count}` is how many, so three files in flight read "attaching 3
/// files…". The per-file form above takes over as soon as one is left.
const BUSY_MANY_TEXT: &str = "attaching {count} files…";

/// A transfer that failed. `{name}` is the file, `{reason}` the helm's own
/// message — the supervisor's words, modulo the surrounding whitespace
/// terminal.js trims before displaying them.
///
/// It says outright that nothing was inserted, because that is the
/// question the user is about to have: the path they were waiting for is
/// not there, and SPEC.md's "an attachment must never disappear silently"
/// is only honored if the failure explains the absence.
const FAILED_TEXT: &str = "attaching {name} failed: {reason} — no path was inserted";

/// A dropped directory, detected up front through the drag-entry API.
/// `{name}` is the directory.
const DIRECTORY_TEXT: &str =
    "{name} is a directory — this version attaches files only, so nothing was uploaded";

/// A dropped item whose bytes could not be read, which is how a directory
/// arrives on an engine with no drag-entry API (PLAN_M4.md item 7's
/// fallback).
///
/// Worded as what was actually observed plus the likely cause, rather than
/// asserting "this was a directory": the read failed, and an unreadable
/// file — a permissions change between the drag and the drop, a removable
/// disk pulled mid-drag — reaches this same message. Claiming to know
/// which would be a guess dressed as a diagnosis.
const UNREADABLE_TEXT: &str = "{name} could not be read, so nothing was uploaded — a dropped directory fails exactly this way";

/// A payload offered to a terminal whose socket is not open.
///
/// Refused up front rather than uploaded, because the whole point of an
/// attachment is the path that gets typed into the terminal, and a
/// detached terminal has nowhere to type it: `term.paste()` on an island
/// whose socket is closed drops the text silently, which is exactly the
/// "an attachment must never disappear silently" failure SPEC.md forbids.
/// No `{name}`: a payload can carry several files, and the reason has
/// nothing to do with which.
const DETACHED_TEXT: &str =
    "this terminal is not connected, so nothing was attached — reconnect and try again";

/// An upload that finished after its terminal lost its socket.
///
/// The file is real and published, so the honest thing is to hand the user
/// the path rather than either pretending it failed or pasting it into a
/// terminal that will drop it. `{name}` and `{path}` are both there
/// because the path is the part they need and the name is how they know
/// which upload it was.
const LANDED_TEXT: &str =
    "{name} landed at {path}; terminal detached — path not inserted, copy it from here";

/// An upload the helm refused without saying why — a status code and an
/// empty body. `{status}` is that code.
///
/// Authored here rather than as a JS literal for the same reason every
/// other sentence in this module is: what the user reads when their
/// attachment fails is reviewable prose, not an implementation detail of
/// the fetch wrapper.
const HTTP_STATUS_TEXT: &str = "the helm answered {status} with no message";

/// A 200 whose body was not the pinned REST contract's `{"path": …}` —
/// unparsable, or parsed with no usable path in it.
///
/// Treated as a failure rather than a success with nothing to insert:
/// something published or did not, and this client cannot tell which, so
/// the only honest report is that no path arrived.
const NO_PATH_TEXT: &str = "the upload reply carried no usable path";

/// What an upload's reply says when it came from a DIFFERENT build of the
/// helm than this page was loaded from (PLAN_M6.md item 6).
///
/// Reported on the terminal's own status line rather than through the
/// page-level banner, because this file's messages are the only surface
/// terminal.js can reach on both targets — see `noteUploadBuild` there.
/// It names the consequence for the thing the user was actually doing
/// (this transfer) before the remedy, since a line about versions on an
/// attachment error would otherwise read as a non-sequitur. `{helm}` is
/// what the helm reported.
const SKEW_TEXT: &str = "this page was built against a different farhelm than the helm now running ({helm}), so this \
     upload may not behave as expected — reload to pick up the matching interface";

/// What an upload's reply says when it carried NO build stamp at all.
///
/// Its own sentence rather than a blank in the one above, because there is
/// no version to name: a helm that reports nothing predates the reporting,
/// and — since a conforming helm both sends the header and exposes it to
/// this cross-origin read — a reply without one is a peer that is not this
/// build, which is the same conclusion by a different route.
const SKEW_SILENT_TEXT: &str = "this page was built against a different farhelm than the helm now running (it reports no \
     build at all), so this upload may not behave as expected — reload to pick up the matching \
     interface";

/// Everything terminal.js needs to intercept, upload, name, quote, and
/// report for one session's terminals, as the JSON handed to
/// `farhelmTerm.sync()`.
///
/// One object per SESSION VIEW rather than one per island: only the status
/// element id differs between terminals, and that rides on each terminal's
/// own spec. A view with the maximum number of tabs would otherwise carry
/// thirty-odd identical copies of this through the eval channel on every
/// desired-set change. `SessionView` memoizes the serialized result by
/// session id, so a feed-driven detail read does not rebuild it.
///
/// Every string here is authored in this module (see the constants above)
/// so that the wording a user reads when an upload fails is reviewable in
/// Rust alongside the rest of this UI's prose, rather than buried in a
/// script file. The `{name}`/`{count}`/`{reason}`/`{status}`/`{path}`
/// placeholders are substituted by terminal.js, in one pass, so a value
/// that itself contains a placeholder is never re-scanned.
pub(crate) fn attachment_policy(session_id: &str) -> serde_json::Value {
    // Each alias's shipped VALUE comes from `image_extension_for` rather
    // than from the table directly: the page runs the derivation, so what
    // it is handed has to be what the derivation actually answers for that
    // token, not a second copy of the table that could drift from it.
    let aliases: serde_json::Map<String, serde_json::Value> = EXTENSION_ALIASES
        .iter()
        .map(|(token, _)| {
            (
                (*token).to_string(),
                serde_json::Value::String(image_extension_for(&format!("image/{token}"))),
            )
        })
        .collect();
    serde_json::json!({
        "upload": attachment_upload_path(session_id),
        // The skew pair (PLAN_M6.md item 6). The upload is the one request
        // this UI makes with `fetch` instead of through `api::send`, so it
        // is the one reply whose build stamp nothing else would read — see
        // terminal.js's `noteUploadBuild`. Both halves ride the policy
        // rather than being spelled out in JS: the header name is a
        // contract with the helm, and the stamp is what THIS bundle was
        // compiled from, which only Rust knows.
        "build": crate::skew::CLIENT_BUILD,
        "buildHeader": crate::skew::BUILD_HEADER,
        "classify": classification_table(),
        "extensionAliases": aliases,
        "maxExtensionLength": MAX_DERIVED_EXTENSION,
        "fallbackExtension": FALLBACK_EXTENSION,
        "namePrefix": GENERATED_NAME_PREFIX,
        "placeholderStem": PLACEHOLDER_NAME_STEM,
        "placeholderMaxAgeMs": PLACEHOLDER_MAX_AGE_MS,
        "separator": PATH_SEPARATOR,
        "safePathChars": SHELL_SAFE_PATH_CHARS,
        "text": {
            "busyOne": BUSY_ONE_TEXT,
            "busyMany": BUSY_MANY_TEXT,
            "failed": FAILED_TEXT,
            "directory": DIRECTORY_TEXT,
            "unreadable": UNREADABLE_TEXT,
            "detached": DETACHED_TEXT,
            "landed": LANDED_TEXT,
            "httpStatus": HTTP_STATUS_TEXT,
            "noPath": NO_PATH_TEXT,
            "skew": SKEW_TEXT,
            "skewSilent": SKEW_SILENT_TEXT,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SPEC.md's precedence order, over the payloads that make it matter.
    ///
    /// The two mixed cases are the ones real payloads produce and the ones
    /// PLAN_M4.md item 7 calls out: a drag carrying a file plus the
    /// `text/plain` copy of its path must upload the file and paste
    /// nothing, and a clipboard carrying image bytes plus an HTML or text
    /// wrapper must upload the image and paste nothing. Text-only is the
    /// other half of the same promise — pasted text that merely looks like
    /// a path is still text, and the classifier has no path-shaped special
    /// case anywhere in it.
    ///
    /// Exactly ONE flavor comes back for every payload, which is what
    /// makes "one winning interpretation, one insertion" structural rather
    /// than a discipline the caller has to keep.
    #[test]
    fn precedence_is_file_then_image_then_text() {
        assert_eq!(classify(true, true, true), Flavor::File);
        assert_eq!(classify(true, false, true), Flavor::File);
        assert_eq!(classify(true, true, false), Flavor::File);
        assert_eq!(classify(true, false, false), Flavor::File);
        assert_eq!(classify(false, true, true), Flavor::Image);
        assert_eq!(classify(false, true, false), Flavor::Image);
        assert_eq!(
            classify(false, false, true),
            Flavor::Text,
            "text that looks like a path is still text — there is no path heuristic to get wrong"
        );
        assert_eq!(
            classify(false, false, false),
            Flavor::None,
            "an empty payload is a no-op, not a failure to report"
        );
    }

    /// The exact array terminal.js indexes, written out as a literal.
    ///
    /// Deliberately NOT derived from `classify`/`payload_index`: a test
    /// that recomputes the table from the same functions that built it
    /// passes for any consistent pair of bugs, including a transposed bit
    /// order. This is the wire format, so it is asserted the way a wire
    /// format has to be — by writing down what must appear on the wire.
    #[test]
    fn the_shipped_table_is_the_exact_eight_entry_wire_array() {
        assert_eq!(
            classification_table(),
            vec![
                "none",  // 0: nothing at all
                "text",  // 1: text only
                "image", // 2: image data only
                "image", // 3: image data + text
                "file",  // 4: file only
                "file",  // 5: file + text
                "file",  // 6: file + image data
                "file",  // 7: file + image data + text
            ]
        );
    }

    /// The generated name a pasted image gets is prefix + counter +
    /// extension, and both authored halves have to hold up: the prefix
    /// says where the file came from, and the extension says what it is so
    /// the agent reading the path can act on it.
    ///
    /// The expectations are a LITERAL table, not derived from the function
    /// under test: an extension rule checked by re-running its own
    /// derivation would accept any self-consistent mistake. Every row is a
    /// MIME type a clipboard actually produces, plus the three shapes the
    /// derivation exists to handle (structured suffix, vendor tree, `x-`
    /// prefix) and the two that genuinely yield nothing.
    #[test]
    fn extensions_come_from_the_mime_type_for_any_image_subtype() {
        assert_eq!(GENERATED_NAME_PREFIX, "pasted-");
        let expected: &[(&str, &str)] = &[
            ("image/png", "png"),
            ("image/jpeg", "jpg"),
            ("image/gif", "gif"),
            ("image/webp", "webp"),
            ("image/bmp", "bmp"),
            ("image/tiff", "tiff"),
            ("image/avif", "avif"),
            ("image/heic", "heic"),
            ("image/heif", "heif"),
            ("image/apng", "apng"),
            ("image/jxl", "jxl"),
            ("image/svg+xml", "svg"),
            ("image/x-icon", "ico"),
            ("image/vnd.microsoft.icon", "ico"),
            ("image/vnd.adobe.photoshop", "photoshop"),
            ("IMAGE/PNG; charset=binary", "png"),
            ("  image/gif  ", "gif"),
            ("application/octet-stream", "bin"),
            ("text/plain", "bin"),
            ("image/", "bin"),
            ("image/+++", "bin"),
            ("", "bin"),
        ];
        for (mime, extension) in expected {
            assert_eq!(
                image_extension_for(mime),
                *extension,
                "{mime} must be named .{extension}"
            );
        }
    }

    /// The narrowing that decides whether an upload keeps its name.
    ///
    /// The placeholder stem plus the type's own extension is exactly the
    /// name Chromium and WebKit synthesize for raw clipboard image data,
    /// and it is the ONLY name treated as "no name the user chose" — the
    /// difference between a screenshot becoming `pasted-1.png` and a
    /// copied `holiday.png` keeping its own name (see `classify`). The age
    /// bound is the second half of that narrowing and must be generous
    /// enough to survive a loaded machine's clock skew between the page
    /// and the engine's stamp.
    #[test]
    fn the_placeholder_rule_is_the_engine_synthesized_name_and_nothing_else() {
        assert_eq!(PLACEHOLDER_NAME_STEM, "image");
        let placeholder = format!(
            "{PLACEHOLDER_NAME_STEM}.{}",
            image_extension_for("image/png")
        );
        assert_eq!(placeholder, "image.png");
        assert_ne!(
            placeholder, "holiday.png",
            "a name the user chose is not a placeholder, whatever its type"
        );
        assert!(
            (1_000..=30_000).contains(&PLACEHOLDER_MAX_AGE_MS),
            "too tight and screenshots lose their generated name on a loaded machine; too loose \
             and the age check stops narrowing anything"
        );
    }

    /// The alias table is the only part of the extension rule the page
    /// cannot derive for itself, so what is shipped has to be exactly the
    /// corrections this module applies — and each entry has to actually
    /// correct something, or it is a row that says nothing.
    #[test]
    fn the_shipped_aliases_are_the_corrections_the_rule_applies() {
        let policy = attachment_policy("s1");
        let aliases = policy["extensionAliases"].as_object().expect("an object");
        assert_eq!(aliases.len(), EXTENSION_ALIASES.len());
        for (token, extension) in EXTENSION_ALIASES {
            assert_eq!(aliases[*token], *extension);
            assert_ne!(token, extension, "an alias to itself corrects nothing");
        }
        assert_eq!(policy["fallbackExtension"], FALLBACK_EXTENSION);
        assert_eq!(policy["maxExtensionLength"], MAX_DERIVED_EXTENSION);
    }

    /// Which published paths have to be quoted before they are typed at a
    /// shell.
    ///
    /// The threat is not the filename — the supervisor already sanitizes
    /// that — but the PARENT directories, which come from the user's own
    /// `--state-dir` and can contain anything a filesystem allows. Each
    /// case below is a real path shape: a macOS-style directory with
    /// spaces, an apostrophe in a user's name, and a `$` that a shell
    /// would expand into something else entirely.
    #[test]
    fn paths_a_shell_would_mangle_are_the_ones_that_need_quoting() {
        // The rule terminal.js applies, spelled out here against the same
        // shipped data it reads: a path is bare only if every character of
        // it is in the safe set, and the empty path is not a bare word at
        // all.
        fn path_needs_quoting(path: &str) -> bool {
            path.is_empty() || path.chars().any(|c| !SHELL_SAFE_PATH_CHARS.contains(c))
        }

        assert!(!path_needs_quoting(
            "/home/dev/.local/state/farhelm/attachments/s-1/screenshot.png"
        ));
        assert!(!path_needs_quoting("/tmp/a-b_c.d/e@f%g+h=i:j,k/l.png"));
        assert!(path_needs_quoting(
            "/Users/dev/Library/Application Support/farhelm/a.png"
        ));
        assert!(path_needs_quoting("/home/o'brien/state/a.png"));
        assert!(path_needs_quoting("/tmp/$(rm -rf ~)/a.png"));
        assert!(path_needs_quoting("/tmp/back\\slash/a.png"));
        assert!(path_needs_quoting("/tmp/star*/a.png"));
        assert!(
            path_needs_quoting(""),
            "nothing is not a bare word, and an empty path must not vanish into the command line"
        );
        assert!(
            SHELL_SAFE_PATH_CHARS.contains('/') && !SHELL_SAFE_PATH_CHARS.contains(' '),
            "the set the page reads must agree with the function above on the two characters \
             every path contains a lot of"
        );
    }

    /// The upload endpoint is the pinned REST contract's, and the session
    /// id in it is encoded: the id comes from a supervisor that under
    /// `--ssh` is a different machine, and an id carrying path syntax must
    /// not be able to redirect a file upload at another session's endpoint
    /// (the same attack `api::path_segments_cannot_escape_their_segment`
    /// pins for the tab routes).
    #[test]
    fn the_upload_endpoint_encodes_the_session_id() {
        assert_eq!(attachment_upload_path("s1"), "/api/sessions/s1/attachments");
        assert_eq!(
            attachment_upload_path("../victim"),
            "/api/sessions/%2E%2E%2Fvictim/attachments"
        );
    }

    /// A status line belongs to exactly one terminal, so its id must be
    /// derived from that terminal's own mount point and must collide with
    /// nothing else the session view renders.
    #[test]
    fn status_element_ids_are_per_terminal() {
        assert_eq!(attachment_status_element_id("terminal"), "attach-terminal");
        assert_ne!(
            attachment_status_element_id("terminal"),
            attachment_status_element_id("terminal-t1")
        );
    }

    /// The messages an upload can put on screen are the user's only
    /// account of what happened to their file, so each has to carry the
    /// thing it is about and each failure has to say what became of the
    /// path. SPEC.md's rule is that an attachment never disappears
    /// silently; wording that omits the name, or that reports a failure
    /// without accounting for the missing path, breaks that while looking
    /// fine.
    #[test]
    fn every_message_accounts_for_the_file_and_the_path() {
        assert!(BUSY_ONE_TEXT.contains("{name}"));
        assert!(BUSY_MANY_TEXT.contains("{count}"));
        for failure in [FAILED_TEXT, DIRECTORY_TEXT, UNREADABLE_TEXT, LANDED_TEXT] {
            assert!(
                failure.contains("{name}"),
                "a message about one file must name it: {failure}"
            );
        }
        assert!(
            FAILED_TEXT.contains("{reason}") && HTTP_STATUS_TEXT.contains("{status}"),
            "the helm's own words, or failing that its status, are what make an error actionable"
        );
        assert!(
            LANDED_TEXT.contains("{path}"),
            "an upload that outlived its terminal must hand the user the path it could not insert"
        );
        assert!(
            FAILED_TEXT.contains("no path was inserted")
                && DIRECTORY_TEXT.contains("nothing was uploaded")
                && UNREADABLE_TEXT.contains("nothing was uploaded")
                && DETACHED_TEXT.contains("nothing was attached")
                && LANDED_TEXT.contains("not inserted")
                && NO_PATH_TEXT.contains("no usable path"),
            "each outcome must account for the absence of the path the user was waiting for"
        );
    }

    /// The policy is what the page runs on, so its keys are a contract
    /// with terminal.js: a rename here that misses the JS side would leave
    /// interception silently disabled (a missing `upload` makes the hooks
    /// no-op) with every Rust test still green. The browser suite is what
    /// would catch it; this test makes the shape itself explicit.
    #[test]
    fn the_policy_carries_every_key_the_page_reads() {
        let policy = attachment_policy("s1");
        assert_eq!(policy["upload"], "/api/sessions/s1/attachments");
        assert_eq!(policy["namePrefix"], GENERATED_NAME_PREFIX);
        assert_eq!(policy["placeholderStem"], PLACEHOLDER_NAME_STEM);
        assert_eq!(policy["placeholderMaxAgeMs"], PLACEHOLDER_MAX_AGE_MS);
        assert_eq!(policy["separator"], PATH_SEPARATOR);
        assert_eq!(policy["safePathChars"], SHELL_SAFE_PATH_CHARS);
        assert!(policy["classify"].is_array());
        assert!(policy["extensionAliases"].is_object());
        // The skew pair the upload path reads (PLAN_M6.md item 6). Pinned
        // here because an absent `build` silently disables the check on the
        // ONE request that does not go through `api::send` — the failure
        // mode is a path that observes nothing, with every other test still
        // green.
        assert_eq!(policy["build"], crate::skew::CLIENT_BUILD);
        assert_eq!(policy["buildHeader"], crate::skew::BUILD_HEADER);
        for key in [
            "busyOne",
            "busyMany",
            "failed",
            "directory",
            "unreadable",
            "detached",
            "landed",
            "httpStatus",
            "noPath",
            "skew",
            "skewSilent",
        ] {
            assert!(
                policy["text"][key].is_string(),
                "terminal.js reads text.{key}"
            );
        }
    }
}
