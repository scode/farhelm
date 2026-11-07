# Host-only lifecycle operations still ask unrelated tmux

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

Keeping one old session on a host can make Restart or Delete of a completely new session fail when the old terminal
server cannot answer, even though the new session has no old tabs.

## Details

Source coordinates from the original records:
`crates/farhelm-supervisor/src/tmux.rs:2951-3036; service/core.rs:13165-13318,13807-13825`;
`crates/farhelm-supervisor/src/service/core.rs:13807,14924-14944,15004-15020; SPEC_impl.md:2826-2840`;
`crates/farhelm-supervisor/src/service/core.rs:14924-14944,15004-15019`;
`crates/farhelm-supervisor/src/service/core.rs:13807,14924-14944,15004-15021`. References below use these paths'
basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

The assigned spec change promises that a session created in a terminal host never asks tmux, and that a host-agent
session asks about old tabs only while the driver's adopted-name set includes it (SPEC_impl.md Terminal substrate:
private tmux server:2826-2828). The correctly implemented gate is terminals.rs:431-451; details and fleet listing use it
(handlers.rs:522-547; listing.rs:174-194). Two management helpers do not: core.rs:14924-14944 session_tabs and
:15004-15021 session_tabs_including_dead call the global pane_states unconditionally for a host agent. Restart reaches
the first helper before its into_host branch (core.rs:13788-13835; caller relaunch at :13501), and Delete reaches the
second whenever entry.terminal is Some (teardown.rs:493-500). Driver pane_states_checked (tmux.rs:4882-4943) knows only
whether ANY private server is running, so a retained unrelated old session causes list-panes to spawn and errors to
propagate. At the introducing commit the same helper bodies are at core.rs:14830-14938. This is a source-proven unwanted
subprocess/dependency; no runtime experiment performed. A concrete source-test reproduction could adopt a usable server
containing an old session, create an independent host session, make tmux list-panes fail through the existing stand-in
driver mechanism, and assert host-only Restart/Delete never invoke it.

Apply may_have_tmux_terminals around these per-session tab reads (and audit guarded Delete's NothingAlive scan at
core.rs:14850-14873, which has the same unconditional query). When skipped, retain the host tab list, rather than
returning all tabs empty. SPEC_impl.md's earlier Terminal host paragraph:2113-2116 permits a failed tmux query to fail
host listings/details when migrated sessions might retain old tabs; it does not authorize probing a known host-only
session, and the newly specified gate makes that distinction explicit.

Suggested bucket: other: established impact is unnecessary lifecycle dependency and possible refusal, with no
security/data-loss route demonstrated. Confidence: confirmed by both introducing and pinned source. proposed_drop:
false. possible_cover: none searched; source peers g008/g010/g011 may have independently found the missing management
gates. The initial details/GetSession suspicion was withdrawn after finding its correct gate; only Restart/Delete helper
paths remain.

Synthesis revision; reviewed pinned tip 407a49435278844248d573083fdea6e23fd96c02. Read the explicitly named peer g010 c3
and its synthesis refinement. Its exact original potential consequence was 'Potential consequence: unlisted or
uncloseable old tabs/processes. Bucket not finalized; potential high, or highest if source establishes orphaned user
work.' Its exclusion is expressly limited to wrong derived names: terminals.rs:416-425 still derives fh-<id>, migration
rejects mismatches, and filtering by that name retains mixed tabs when the query succeeds. That does not refute g009-l4:
global pane_states can fail before any name filtering. Peer g010 i1 independently corroborates the same helper route and
corrects the Delete caller citation to teardown.rs:479-486. Preserve g009's prior refutation of normal
GetSession/details: handlers.rs:522-547 does apply the gate, and that claim remains withdrawn. Current suggested bucket
remains other; no new security/lost-work consequence asserted. proposed_drop: false; possible_cover: explicitly named
g010 i1 corroboration, consolidate with g009-l4. Next synthesis checks: exact ordinary-query-error propagation versus an
indefinitely hung tmux command, and the Restart/Delete mutation boundaries; no separate discovery pass.

Synthesis revision at pinned 407a49435278844248d573083fdea6e23fd96c02. Exact Delete call-site correction:
service/teardown.rs:479-486, not the earlier :493-500 citation. session_tabs_including_dead errors map to
TeardownError::TabRediscovery before upload cancellation, the portable sweep, host Shutdown, attachment extraction or
row removal. For an ordinary query error that returns, this is a visible refusal, not an indefinite stall or a
demonstrated orphan-process route. Restart's helper error is RelaunchFailure::definitive (core.rs:13807-13811); relaunch
handles it by abort_relaunch, restoring prior outcome when that succeeds, restoring the retained terminal/entry, then
returning failure.error (:13501-13756). Its agent stop/detach occurred earlier and is not undone; a failed relaunch does
not automatically start the old agent again. This narrows the established consequence to unnecessary query work and
lifecycle refusal, with Restart potentially already stopped/detached. Do not describe an ordinary nonzero/failed query
as a hung tmux process, lost session, or lost user work. Suggested bucket remains other, no newly established highest
consequence. proposed_drop false: the adopted-name gate is still missing, even though errors fail visibly.
possible_cover: g010 i1 is the same dependency finding. SPEC.md Slow hosts:3987-3999 explicitly accepts ordinary
short-on-healthy-host work taking as long as a slow host needs; unsupported tmux wrappers/custom behavior are excluded
by Supported user environments:4488-4495. Neither is needed to establish an ordinary returning query failure. Next: read
the real command runner's deadline/error classification and the Restart pre-helper stop boundary to finish the requested
distinction.

Final synthesis revision, pinned 407a49435278844248d573083fdea6e23fd96c02. Exact runner trace: pane_states_checked ->
run(list-panes -a) (:4882-4905) -> run_bytes (:2655-2657) -> run_bytes_within(args, None) (:2951-2953) ->
command.output().await with no timeout on the None branch (:3002-3006). A genuinely hung private tmux command can
therefore leave that helper waiting; this is conditional on a command that really never completes, not what an ordinary
nonzero exit does. Nonzero exits become TmuxCommandFailure (:3017-3035), and pane_states_checked converts only
is_definitively_empty errors to an empty map. All other returned failures propagate. An absent server (including a
last-old-session exit) is therefore not the failure reproduction. A usable server adopted at startup plus a later
ordinary spawn failure/non-definitive query failure is enough to show the unnecessary dependency; no indefinitely hung
server or unsupported wrapper is required. If startup classified tmux as Unusable, known_absent short-circuits its query
as an empty result instead, so do not conflate that accepted g009-l1 case with l4.

Returning-error effect: Restart already runs stop_live_agent or the prior-run sweep at core.rs:13165-13218 and detaches
the old viewer before calling session_tabs (:13495-13518). The helper wraps its returned error as a definitive relaunch
failure (:13807-13811), so relaunch restores the prior outcome/terminal entry and returns the error (:13570-13756;
republished_terminal :3496-3505), without automatically relaunching the stopped agent. Delete's
session_tabs_including_dead error at teardown.rs:479-486 is before destructive sweep/host teardown/row removal;
handlers.rs:1274-1277 renders 'could not determine this session’s terminal tabs, so nothing was deleted'. Nothing here
establishes lost session ownership, user work or credentials.

Contract reconciliation: SPEC_impl.md Terminal substrate: private tmux server:2826-2828 says a host-agent session asks
about old tabs only while the adopted set contains its tmux session; a freshly host-created session never asks. Both
helpers call global pane_states without the terminals.rs:443-451 membership predicate, so both an unnecessary successful
query and the returning-error refusal violate that specific rule. Mixed sessions that actually remain in the adopted set
must still query; the g010 c3 derived-name exclusion correctly protects their lookup identity and is a separate issue.
The earlier broad host-listing paragraph does not override this later explicit no-query gate.

Suggested bucket remains other; confidence confirmed for the unwanted query/ordinary refusal, conditional only for the
additional genuine-hang effect. proposed_drop false for the missing gate. Do not elevate a hypothetical hung-tmux
consequence into a separate high finding: SPEC.md Slow hosts:3987-3999 accepts healthy-host-brief work waiting on a slow
host, and no in-scope shipped input that permanently hangs supported private tmux was demonstrated in this review. This
is an explicit limitation/lowered consequence from any possible broad reading of 'cannot answer', not a loss-of-work
claim. possible_cover: same g010 i1 corroboration; g024/g025 source ownership to consolidate through parent. No new
independent defect discovered in synthesis.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g009/g009-l4`,
`g010/i1`, `g024/s1`, `g025/i7`. The retained append-only histories preserve all earlier claims and corrections.
Reviewed source references name their own revisions; the inclusive reviewed tip is the hash above. This review used
source inspection only, with no product runtime experiments or live-install changes.
