# Old-page caller guards permit the interactive unknown-kind mount in g196/l1

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: a browser page left open through the grid-only update can still open an interactive legacy
terminal against the new grid endpoint. This supplies the caller/mount premise for parent-supplied g196/l1: a pasted
JSON document can be interpreted as input instructions instead of literal text, potentially executing inner control
bytes. Details within this group's caller scope: parent lib.rs:520-527 defaults missing agent_terminal to None and
missing supervisor_creates_host_terminals to false. parent session_view.rs:1771-1776 calls agent_spec_kind; :3839-3849
preserves null outside an active Restart; :1809 and :1840 send that unknown kind to terminal.js. For a live ordinary
nonretired Running session, parent keep_terminal_surface at :3793-3814 returns true; :1786 sets unavailable only from
session.stale, so a fresh live response does not suppress mounting. The lease mount at :906-913 accepts a successfully
minted lease without a local skew veto. Parent AppBody lib.rs:2042-2098 renders the BuildSkewNotice and retains the
authenticated tree; :2272-2282 still mounts SessionView by selected session ID. skew.rs:30-35 explicitly documents no
render blocking, and its notice at :286-293 is a PeerLine, not a modal/input cover. Thus neither actual App rendering
nor the shared skew signal blocks this fresh mount. The session view only consults helm_is_current for reconnect_policy,
not keep_terminal_surface. That policy disables automatic attempts and heartbeat, leaving manual connection available
(reconnect.rs:186-224). Parent terminal.js:4650-4665 calls buildXtermSurface for unknown kind; :5

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g088/l2`.

### g088/l2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 13 (finding, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e, comparing the previous page at its parent. TLDR: a browser
page left open through the grid-only update can still open an interactive legacy terminal against the new grid endpoint.
This supplies the caller/mount premise for parent-supplied g196/l1: a pasted JSON document can be interpreted as input
instructions instead of literal text, potentially executing inner control bytes. Details within this group's caller
scope: parent lib.rs:520-527 defaults missing agent_terminal to None and missing supervisor_creates_host_terminals to
false. parent session_view.rs:1771-1776 calls agent_spec_kind; :3839-3849 preserves null outside an active Restart;
:1809 and :1840 send that unknown kind to terminal.js. For a live ordinary nonretired Running session, parent
keep_terminal_surface at :3793-3814 returns true; :1786 sets unavailable only from session.stale, so a fresh live
response does not suppress mounting. The lease mount at :906-913 accepts a successfully minted lease without a local
skew veto. Parent AppBody lib.rs:2042-2098 renders the BuildSkewNotice and retains the authenticated tree; :2272-2282
still mounts SessionView by selected session ID. skew.rs:30-35 explicitly documents no render blocking, and its notice
at :286-293 is a PeerLine, not a modal/input cover. Thus neither actual App rendering nor the shared skew signal blocks
this fresh mount. The session view only consults helm_is_current for reconnect_policy, not keep_terminal_surface. That
policy disables automatic attempts and heartbeat, leaving manual connection available (reconnect.rs:186-224). Parent
terminal.js:4650-4665 calls buildXtermSurface for unknown kind; :5721-5724 and :5891-5915 interpret missing stream
notice as bytes. Its old input path at :4158-4159 and :4191-4195 sends UTF-8 raw data whenever the socket is OPEN,
without checking skew or negotiated input type; :4628-4634 installs term.paste with that socket-open liveness.
:5631-5650 accepts replay_complete without a stream notice and ends catch-up; :5725-5763 writes the binary frames as
bytes rather than refusing. No guard in these caller paths disproves g196's interactive-mount premise. An ordinary shell
tab in a previously open page provides a running-shell target without assuming an agent is itself a shell. Unknown
tab.kind is passed directly at :1840. The source permits this stale page use; no runtime schedule or paste was executed.
Suggested bucket: preserve highest/security or loss-of-work concern supplied by g196, since the established caller can
send raw pasted text to the JSON-decoding endpoint; full input reinterpretation route and control-byte consequence are
documented by owner g196/l1, not independently re-reviewed beyond this caller scope. Confidence: confirmed caller/mount
and lack-of-guard premises; full consequence remains likely with g196's stated small valid-Text-JSON paste and fresh
default paste-mode premises. proposed_drop: false; broad stale-page acceptance is not accepted here as permission to
reinterpret document contents as commands. Exact potentially applicable specification text at reviewed
SPEC_impl.md:3022-3027: 'A page loaded from the previous helm and left open across the update must be reloaded; nothing
keeps it working.' It then describes that 'what is typed into it ends the attachment as malformed input' and 'a reload
is the whole remedy.' These accept broken drawing/attachment and require independent judgment for the different
successful input-execution consequence, rather than source-refuting it. Could-be-highest: yes, conditional on g196's
exact shipped paste/decoder route; this review establishes its previously missing UI premise. possible_cover: supplied
g196/l1 is the same concern, retained for parent consolidation, not a separate independent discovery. Caveats: automatic
reconnection is withdrawn after build skew; existing known-host grid mounts need not take the raw branch; a fresh
unknown-kind mount and valid grid-shaped paste before other invalid input are needed. No open caller/guard premise
remains; decoder provenance is intentionally left with assigned owner g196. No tests, builds or reproductions.

Returned proposal (claims-058, event 13): **retain-unverified**, suggested bucket **highest**.

The caller/mount premise is source-established. Keep this as supporting evidence for the parent-supplied g196/l1
concern, preserving its conditional command-execution consequence until the decoder owner supplies the remaining route.
Do not create a second independent discovery from this contribution.

Original stronger consequence: A document pasted into a stale legacy surface may become input instructions containing
executable control bytes against a live shell, with potential security or user-work loss. The earlier unresolved lead
and latest caller confirmation retain that consequence conditionally; no withdrawal or lowering occurred.

Unresolved premise or limitation: The current grid decoder's acceptance of the supplied small Text JSON paste, its
default paste-mode handling, exact control-byte delivery and command execution remain with g196/l1. They were not
independently established by this caller-only consultation. Parent must consolidate that evidence before finalizing
confidence/severity; preserve highest meanwhile.

Proposed basis: Retain the named contribution with confirmed mount premises and explicit missing decoder premises. No
proposed dismissal, upgrade filter or consequence lowering.

Existing parent disposition (event 13): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g088 record](records/review-through-00de0b6c/groups/g088.json). Record ID: `l2`.
