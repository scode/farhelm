# Protocol version changelog

This is the per-version history for protocol versions 2 through 11, relocated verbatim from `PROTOCOL_VERSION`'s doc
comment in `crates/farhelm-proto/src/lib.rs` on 2026-08-20. It is preserved as written rather than maintained as the
protocol evolves, per the convention for entries in this directory.

Bumped to 2 when `ControlMsg::Error` gained its required `kind` field.
That is a wire-format change, not a compatible addition: an old peer's
decoder has no default for a missing field and errors on the first
`Error` message it receives — which, unlike a refused handshake, tears
down an already-established, possibly multi-session connection instead
of failing cleanly before anything was shared. Version skew must be
caught at the hello, not discovered mid-connection.

Bumped to 3 for `StopSession`/`SessionStopped` and
`DeleteSession`/`SessionDeleted` (PLAN_M2.md step 4). This is the ONE
version bump M2 gets: PLAN_M2.md commits every later M2 wire change to
being strictly additive and tolerant on decode within version 3 (new
optional fields with defaults, new message variants nothing yet sends)
so that mixed M2-era builds keep interoperating without a bump each
time. Anything that cannot be made additive earns its own bump instead
of retroactively stretching this one's meaning.

Bumped to 4 for `ControlMsg::PauseOutput`/`ResumeOutput` (PLAN_M2_5.md
step 2). M2's "new message variants are additive" premise above turned
out to be false: `io::parse_control` decodes `ControlMsg` through a
single `#[serde(tag = "type")]` enum, so an unrecognized tag is a decode
error, not a defaulted no-op, and both the helm and the supervisor
connection loops treat that error as fatal — an unknown variant tears
down an already-established connection instead of being ignored. A new
variant is exactly the "cannot be additive" case that earns its own
bump, per version 3's own docs above; `protocol_version_is_pinned_at_11`
(renamed at every bump since `_at_4`) and
`unknown_control_message_tag_fails_decode` below (plus the
loop-level teardown test in the farhelm crate's e2e suite) pin both the
number and the reasoning so the next milestone cannot re-assume
tolerance that was never there. Within version 4, the same additive
discipline applies: later M2.5 wire changes must be new optional fields
with decode defaults, not new variants, or they earn their own bump too.

Bumped to 5 for the complete M3 wire vocabulary (PLAN_M3.md item 1):
`SessionStatus::Interrupted`/`Error`, `SessionInfo`'s stop annotation
and restart-offer fields, `CreateSession`'s intent key and snapshot
overrides (`agent_kind`, `resume_template`), `ErrorKind::Conflict` for
an intent key reused with a different fingerprint, and
`RestartSession`/`SessionRestarted`. All of it lands in ONE bump rather
than several: each of the FIVE new tagged-enum variants above (two on
`SessionStatus` — `Error` and `Interrupted`; one on `ErrorKind` —
`Conflict`; two on `ControlMsg` — `RestartSession` and
`SessionRestarted`) independently earns its own bump by version 4's own
argument above — an unrecognized tag is connection-fatal, never a
tolerated no-op — so spreading M3 across a version per variant would
only multiply the number of protocol versions a mixed fleet has to
reason about, for a milestone that ships all of them together anyway.
(`AgentKind`'s three variants, including `Generic`, are not part of
this count: `AgentKind` itself is new in version 5, so there is no
prior decoder for it to break — the count above is specifically about
variants added to enums that already existed at version 4.) Within
version 5, new FIELDS on an existing message keep the same additive
discipline version 4 established: default on absence, ignored when
unrecognized. Only a genuinely new tagged variant, OR a field change
that cannot be made additive — a new REQUIRED field being the version 2
precedent documented above, where an old decoder has no default to
fall back on — needs another bump; neither is anticipated before M4.

Bumped to 6 for the complete M4 wire vocabulary (PLAN_M4.md item 1):
terminal tabs and attachment uploads. The new tagged `ControlMsg`
variants — `OpenTab`/`TabOpened`, `CloseTab`/`TabClosed`, and the
upload family (`BeginUpload`/`UploadStarted`/`UploadAck`/
`CommitUpload`/`UploadCommitted`/`AbortUpload`/`UploadAborted`) — each
independently earn the bump by version 4's argument above (an
unrecognized tag is connection-fatal, never a tolerated no-op), so as
with 5, all of M4 lands in ONE bump rather than a version per
variant. The additions that ride along are `SessionInfo::tabs` and
`Attach`'s `terminal`/`lease` fields, all defaulted on absence — but
only `tabs` and `lease` are honestly additive. `terminal`'s default
protects one direction alone (an old-shaped request decoded by a new
peer); the other direction is unsafe, because a field-ignorant
decoder receiving `TerminalSelector::Tab` would silently attach the
agent terminal — exactly the wrong-terminal fallback that type's own
contract forbids. Non-default terminal selection is safe ONLY because
the version-6 handshake guarantees no such decoder is on the other
end; the default expresses compatibility of meaning, not a license to
send tab selectors at older peers. Shipping all three inside the bump
means no mixed-fleet state ever exists where a version-6 peer lacks
them. (`TerminalSelector` and `TabInfo` are new types with no
prior decoder to break — outside the variant count for the same
reason `AgentKind` was at version 5.) Within version 6 the additive
discipline of versions 4 and 5 continues to apply: new optional
fields with decode defaults are fine; a new tagged variant, or a new
REQUIRED field, earns the next bump.

Bumped to 7 for the complete M5 wire vocabulary (PLAN_M5.md item 1):
the replay-complete marker and session rename. `ReplayComplete` is a
new tagged `ControlMsg` variant, independently earning the bump by
version 4's argument above (an unrecognized tag is connection-fatal,
never a tolerated no-op); `RenameSession` and `SessionRenamed` are two
more — so, as with 5 and 6, all three land in ONE bump rather than a
version per variant. Nothing else on the wire changes: no new
`SessionInfo` field, no new `ErrorKind`, no new frame kind. The
marker's alternative shapes were both considered and rejected, not
merely unconsidered — an in-band byte sentinel spliced into terminal
data (rejected because replay bytes are arbitrary history any
sentinel could collide with, and this protocol never interprets or
rewrites terminal bytes) and a new frame kind (rejected as a
structural change to the framing layer for no gain over a control
message) — see `ControlMsg::ReplayComplete`'s own docs for the full
argument. `RenameSession`'s validation contract (control-character
refusal, field cap, last-write-wins) and `SessionRenamed`'s built-not-
echoed reply are handler-side behavior a later PR implements
(PLAN_M5.md item 3); this bump only fixes what travels on the wire.
Within version 7 the additive discipline of every prior version
continues to apply: new optional fields with decode defaults are
fine; a new tagged variant, or a new REQUIRED field, earns the next
bump.

Bumped to 8 for the complete M6 wire vocabulary (PLAN_M6.md item 1): the
supervisor's host identity, cursor-paginated `ListSessions`, and each
`SessionInfo`'s creation timestamp. Unlike every prior headline reason,
none of the three is a new tagged variant — but all three still earn the
bump:

- `Hello::host_identity` (`Option<String>`, `None` on every hello THIS PR
  sends — minting and filling it in is a later PR's serving work, see
  below) is additive on its own; it only rides this bump because one was
  already needed for the other two, and shipping it separately would
  just be another version for a mixed fleet to reason about, per version
  5's argument for bundling all of one milestone's vocabulary into a
  single bump.
- `ListSessions` gains `cursor: Option<String>` and `limit: Option<u32>`,
  themselves additive — but the reply they page through is not (next
  point).
- `SessionList` DROPS `truncated` and gains `next_cursor: Option<String>`
  (PLAN_M6.md's "Pagination shape": absence means exhaustion).
  Truncation-as-error is replaced by next-cursor-as-continuation; `total`
  keeps meaning what it always has, the full count before any cut. A
  field REMOVAL is the version-2 precedent this doc keeps citing: an old
  decoder has no way to notice a field vanished, so a v8 sender must
  never reach a v7 decoder, exactly as if `next_cursor` had instead been
  a new tagged variant.
- `SessionInfo` gains `created_at: i64` (seconds since the Unix epoch,
  matching `StoredSession::created_at`'s column — see that field's docs),
  additive via `#[serde(default)]` like every prior `SessionInfo` field
  addition, and folded into this same bump for the bundling reason above.

`created_at` IS sourced from storage by this same PR (the supervisor's
store has held it since M2 — see `StoredSession::created_at`'s docs),
since doing so is a mechanical fill-in at every existing `SessionInfo`
construction site rather than new serving logic. Minting a host identity
and serving real cursor-paginated pages were, at THIS bump, deliberately
deferred to a later PR (PLAN_M6.md items 1 and 2) — this bump shipped
only the vocabulary those two need to travel on the wire. Both have
since landed: `SessionStore::ensure_host_identity` mints and
`service::handle_list_sessions`/`build_list_reply` serve real pages,
under the SAME version 8 — the wire shape was already right, so serving
it for real needed no further bump. Left here as the version's own
history, not corrected in place, per this doc's own vocabulary-first
framing: the bump is what PROTOCOL_VERSION 8 shipped, in the order it
shipped in. Within version 8 the additive discipline of every prior
version continues to apply: new optional fields with decode defaults are
fine; a new tagged variant, a new REQUIRED field, or a field REMOVAL
earns the next bump.

Bumped to 9 for ONE field: [`ControlMsg::Attach::if_unowned`], the
non-displacing attach auto-reconnect needs (PLAN_M6.md item 7). It is
decode-additive — `#[serde(default)]`, an older decoder drops it
without complaint — and that is precisely why it earns a bump instead
of riding version 8. Every additive field this doc has waved through
was additive in MEANING as well as in decoding: ignoring it produced
the same outcome the sender would have accepted anyway. This one
inverts that. It says "refuse rather than displace", and a peer that
ignores it does the opposite of what was asked — it takes a session
from a client that still holds it, on behalf of a browser that was not
even there to ask. The failure is silent on both ends: the sender sees
a successful attach, the displaced client sees its terminal die, and
nothing anywhere reports a version problem.

So the rule this doc has stated since version 4 gains its sharper
form: a field whose whole purpose is to CHANGE what the receiver does
is not additive, whatever serde makes of it. SPEC.md's version rule
("Incompatible versions refuse to connect with a clear, actionable
error; there is no silent degradation") is the standard being met
here, and the hello refusal is the machinery that meets it — a helm on
9 and a supervisor on 8 now refuse each other at connect, and the host
surfaces `version-skew` with both builds named and the helm's own
remediation sentence, instead of quietly running a fleet where
automatic reconnects steal sessions.

The browser edge cannot use the hello, having none: it is gated on the
helm's build stamp instead (farhelm-ui's `skew` module), which refuses
unattended attaches whenever the helm answering is not the build this
bundle was made for. Same rule, same milestone, different handshake.

Within version 9 the additive discipline of every prior version
continues to apply, now with that sharper reading: new optional fields
with decode defaults are fine WHEN ignoring one is harmless; a field
whose omission changes behavior, a new tagged variant, a new REQUIRED
field, or a field REMOVAL earns the next bump.

Bumped to 10 for the complete M6.75 wire vocabulary (PLAN_M6_75.md item
3): the live-status split, agent profiles, and the source-profile
snapshot. As with 5, 6 and 7, all of one milestone's vocabulary rides
ONE bump rather than a version per change — "wire shapes never trickle"
is that item's own standing rule, and every piece below independently
earns a bump anyway:

- [`SessionStatus`] REPLACES `Alive` with `Running`/`Waiting`/`Idle`.
  That is a removal AND three new tagged variants at once, the two
  hardest cases this doc names: a v9 decoder fails outright on
  `{"state":"running"}` (no fallback for an unrecognized tag), and a v10
  decoder fed `{"state":"alive"}` fails the same way in reverse. Both
  directions are pinned below, and the hello refusal is what keeps
  either from ever happening in the field.
- [`ControlMsg::CreateSession`] gains its profile-backed MODE:
  `invocation` becomes `Option<String>` and `profile_id` joins it,
  exactly one of the two per request. Making a REQUIRED field optional
  is decode-tolerant in only one direction: a v9 request still decodes
  here, while a v10 profile-mode request fails under a v9 decoder — and
  fails on an INVALID VALUE, not a missing key. This crate's encoder
  always emits an `Option` field, so what a v9 peer actually receives is
  `"invocation": null`, which its required `String` refuses. The
  distinction matters because absence is the case serde is lenient about
  and a null is not; the refusal here is the reliable half.
- The profile CRUD family (`ListProfiles`/`ProfileList`,
  `CreateProfile`/`ProfileCreated`, `UpdateProfile`/`ProfileUpdated`,
  `DeleteProfile`/`ProfileDeleted`) is eight new tagged `ControlMsg`
  variants, each of which earns the bump on version 4's argument alone.
- [`SessionInfo::source_profile`] is a new OPTIONAL field (absent =
  raw-created), additive on its own and riding this bump for the
  bundling reason above. [`Profile`], [`SourceProfile`] and
  [`ProfileExistence`] are new TYPES with no prior decoder to break,
  outside the variant count for the same reason `AgentKind` was at 5
  and `TerminalSelector` at 6.

What this bump deliberately does NOT carry, decided in PLAN_M6_75.md
item 3 so it is not re-litigated per PR: any supervisor-edge PUSH
channel. The helm keeps its 3-second drain plus the existing post-write
wake, accepting one drain interval of status staleness; the push problem
M6.75 solves is the CLIENT edge, where the helm coalesces revisions it
would have had to build regardless. Also absent: remembered profile
defaults, which the helm owns in helm.db and resolves into a concrete
`profile_id` before it ever sends a create — there is nothing for this
protocol to carry.

Bumped to 11 for all M7 wire vocabulary in one move (PLAN_M7.md item
2). Ignoring [`ControlMsg::Hello`] authentication would widen a
restricted peer to full authority, while [`ControlMsg::ArchiveSession`],
[`ControlMsg::SessionArchived`], and [`ErrorKind::Unauthorized`] are new
tagged variants an older decoder refuses. The remaining additions ride
the same milestone bump: spawn parent/profile-name fields and archive
metadata. `SessionArchived` has carried its required post-teardown
`session` from version 11's first published shape; it was never an
additive field within an already-established version.

Within version 11 the additive discipline of every prior version
continues to apply, with version 9's sharper reading intact: new
optional fields with decode defaults are fine WHEN ignoring one is
harmless; a field whose omission changes behavior, a new tagged variant,
a new REQUIRED field, or a field REMOVAL earns the next bump.
