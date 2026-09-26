# Protocol version notes through version 30

This is `PROTOCOL_VERSION`'s doc comment in `crates/farhelm-proto/src/lib.rs` as it stood at protocol version 30,
relocated verbatim on 2026-09-25 when the doc comment was cut back to the rules still in force. It continues the
history in `2026-08-20-protocol-version-changelog.md` (versions 2 through 11) and, like that entry, is preserved as
written rather than maintained: the duplicated "Version 20" paragraph below is how the comment read at the time.
Version rationale for later bumps is recorded where `PROTOCOL_VERSION`'s current doc comment says.

Protocol version exchanged in the hello. Bumped only for incompatible
frame or message changes; the receiving side refuses a mismatch with a
clear error per SPEC.md's version-skew rule. Build versions travel
alongside for diagnostics only and never gate anything.

Within version 21 the additive discipline of every prior version
continues to apply, with version 9's sharper reading intact: new
optional fields with decode defaults are fine WHEN ignoring one is
harmless; a field whose omission changes behavior, a new tagged variant,
a new REQUIRED field, or a field REMOVAL earns the next bump.

A field whose whole purpose is to CHANGE what the receiver does is not
additive, whatever serde makes of it. SPEC.md's version rule
("Incompatible versions refuse to connect with a clear, actionable
error; there is no silent degradation") is the standard being met
here, and the hello refusal is the machinery that meets it — a helm on
9 and a supervisor on 8 refuse each other at connect, and the host
surfaces `version-skew` with both builds named and the helm's own
remediation sentence, instead of quietly running a fleet where
automatic reconnects steal sessions.

The browser edge cannot use the hello, having none: it is gated on the
helm's build stamp instead (farhelm-ui's `skew` module), which refuses
unattended attaches whenever the helm answering is not the build this
bundle was made for. Same rule, same milestone, different handshake.

What version 10 deliberately does NOT carry, decided in PLAN_M6_75.md item
3 so it is not re-litigated per PR: any supervisor-edge PUSH channel. The
helm keeps its 3-second drain plus the existing post-write wake,
accepting one drain interval of status staleness; the push problem M6.75
solves is the CLIENT edge, where the helm coalesces revisions it would
have had to build regardless. Also absent: remembered profile defaults,
which the helm owns in helm.db and resolves into a concrete launch bundle
before it ever sends a create — there is nothing for this protocol to
carry.

Version 13 narrows that "no supervisor-edge push channel" statement
rather than reversing it, and the distinction is worth keeping exact.
It adds exactly ONE request shape that travels supervisor→helm
([`ControlMsg::AgentRequest`], answered by
[`ControlMsg::AgentResponse`]), and nothing else changes direction: the
helm still learns about session and terminal state by drain and by the
existing post-write wake, so the staleness trade version 10 accepted is
untouched. What forced an upward request at all is that an agent inside
a session has no route, address, or credential back to the machine
running the helm, so the supervisor it CAN reach has to carry the
question the rest of the way — see [`ControlMsg::AgentRequest`].

Version 14 REMOVES session-list pagination from this wire.
[`ControlMsg::ListSessions`] lost its `cursor` and `limit`, and
[`ControlMsg::SessionList`] lost `total` and `next_cursor` and gained
`truncated`: a supervisor now answers with its whole session set in one
reply, cut only at [`LIST_SESSIONS_CAP`]. Field removals are the
non-additive case by this constant's own rule, hence the bump. The
contract behind the change is SPEC.md's Session list section: the fleet
this product is for is tens of sessions, and no layer is to paginate,
cursor, stream, or index the list on the server's side.

Version 15 removes the supervisor-owned profile CRUD vocabulary and the
`CreateSession::profile_id` selector. Creates now carry the helm-resolved
launch bundle, including a snapshot of the profile identity when one was
selected. It also adds [`ProfileExistence::Unresolved`]: supervisors emit
that placeholder because only a helm has the catalog needed to derive the
browser-facing state. [`AgentVerb::ResolveProfile`] and
[`AgentReply::ResolvedProfile`] add the upward relay used when
`farhelm spawn --agent` asks an attached helm to resolve a name.

Version 17 adds host-directory browsing. A helm asks the selected
supervisor rather than reading its own filesystem, so a remote composer
never receives paths from the wrong machine.

Version 16 adds the structured launch snapshot carried by
[`ControlMsg::CreateSession`] and [`SessionInfo`]. A helm depends on the
supervisor retaining it through retry, clone, and restart, so a peer that
would silently discard this lifecycle data is not protocol-compatible.

Version 18 carries the supervisor's accepted canonical working-directory
fact in [`SessionInfo::canonical_cwd`]. Although the JSON field is
optional for old durable rows, a helm uses it to decide folder-history
identity. An older helm silently ignoring it would retain and merge a
different history, so this is not the harmless optional-field case.

Version 19 adds OpenCode to [`LaunchHarness`]. The harness lives in a
structured create and session snapshot, so an older supervisor could not
decode its new enum tag. Refusing this version mismatch is preferable to
accepting a create whose durable launch provenance has been lost.

Version 20 makes every consequential agent selector explicit, adds
profile discovery and stable host ids, carries arbitrary clone sources,
and adds the expected-title precondition used by agent rename. It also
Version 20 makes lifecycle targets, create and clone hosts, clone sources,
and create selectors explicit; adds conditional agent rename and discovery
envelopes; and carries an explicit inheritance selector for local spawn.
Older peers would silently apply the defaults these fields remove, so the
exact-match handshake must refuse mixed versions.

Version 21 adds Goose and Pi structured harnesses, their native effort and
permission vocabulary, and their dedicated agent kinds. Those enum tags are
durable launch provenance and capture policy; an older peer cannot safely
decode or retain them, so mixed versions must refuse the hello rather than
silently compiling another harness or dropping exact-resume behavior.

Version 22 adds the OMP agent kind (`omp`) to the closed [`AgentKind`]
vocabulary. The kind decides which conversation reports a supervisor
accepts (OMP reports a typed locator under its own `omp:` prefix) and
which hook injection its launches receive; an older peer cannot decode
the new enum tag at all, so mixed versions must refuse the hello rather
than silently dropping exact-resume behavior or accepting a report
against the wrong integration.

Version 23 adds the OMP structured harness (`omp`) to the closed
[`LaunchHarness`] vocabulary. The harness tag is durable launch
provenance — it is stored beside the resolved invocation, replayed by
clone and history, and its strict stored-row decoding pairs it with
`AgentKind::Omp` — so an older peer that could not decode the tag could
neither retain a stored launch nor validate a new create, and mixed
versions must refuse the hello rather than silently losing the recorded
launch selection. (The split from version 22 is deliberate: each closed
wire vocabulary grows in its own reviewed step.)

Version 24 adds the owned-GitHub-checkout wire vocabulary:
[`ControlMsg::GithubCheckoutPreview`]/[`ControlMsg::GithubCheckoutPreviewed`]
and [`ControlMsg::GithubRepoSearch`]/[`ControlMsg::GithubRepoResults`], the
optional [`ControlMsg::CreateSession::github_checkout`] create payload, and
the [`SessionInfo::github_repo`]/[`SessionInfo::working_copy`] provenance
fields. [`ErrorKind::CheckoutConflict`] distinguishes a durably refused
fresh allocation from an ambiguous or previously accepted intent. These
additions share this feature's version bump. The create payload is why
this is a bump and not an additive drift:
a fresh-checkout create names an intent (clone a repository the user has
never materialized) that an older supervisor cannot see in its
create-pipeline at all. Silent tolerance would mean an old peer dropping
the intent while reporting an ordinary create — exactly the outcome the
handshake exists to make impossible — so an older peer must refuse the
hello rather than half-serve the request.

The published v0.10.0-rc.3 used 22 for the checkout vocabulary from a
stale base without OMP. Its number does not identify the OMP vocabulary
described above; version 24 refuses both that build and OMP-only peers.

Version 25 adds the tagged `AgentVerb::Restart` request and the
non-secret `AgentSession::restart_offer` discovery field. Both change
what an attached-session caller can safely request, so serde tolerance
is not compatibility: the exact-version handshake refuses an older peer
before it can ignore the capability or reject the new verb mid-relay.

Version 26 removes session-archive state and operations from every wire
shape. Exact-version negotiation keeps an older peer from presenting or
accepting that removed lifecycle vocabulary.

Version 27 adds Cursor to the structured launch vocabulary. Older peers
cannot decode that harness, even though it uses the existing Generic runtime.

Version 28 requires a vendor discriminator on conversation reports and
adds optional subagent identity evidence.
Exact-version negotiation prevents older peers from bypassing that contract.

Version 29 adds Grok to the structured launch and durable agent-kind
vocabularies. Older peers cannot retain its tracked launch policy.

Version 30 adds optional compiled structured launch fields to
`RestartSession`; older peers must refuse the handshake rather than
silently restarting with stale settings.

`protocol_version_is_pinned_at_30` (renamed at every bump since `_at_4`)
and `unknown_control_message_tag_fails_decode` below, plus the loop-level
teardown test in the farhelm crate's e2e suite, pin both the number and
the reasoning so the next milestone cannot re-assume tolerance that was
never there.

The entry-by-entry history for versions 2 through 11 is preserved in
`lore/2026-08-20-protocol-version-changelog.md`; that file is frozen at
the moment it was written (`lore/AGENTS.md`) and is not extended for
version 12 or later — see [`ControlMsg::ReportConversation`] for what
version 12 added, [`ControlMsg::AgentRequest`] for version 13,
[`ControlMsg::SessionList`] for version 14, and
[`ControlMsg::ReportConversation`]'s required fields for version 28.
