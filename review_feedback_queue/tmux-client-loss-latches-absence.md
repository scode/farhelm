# A failed tmux query can destroy live old terminals

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

A temporary allocation failure in one tmux query process can make Farhelm treat a still-running old server as empty.
During startup it can then destroy that server's terminal sessions without a Delete request. During later listings it
can hide live work and let guarded Delete kill the live agent without its required warning. These are source-confirmed
conditional routes, with no measured allocation failure or incident.

## Details

Source coordinates from the original records: `crates/farhelm-supervisor/src/tmux.rs:3099-3126,4901-4903;g031/r5`;
`crates/farhelm-supervisor/src/tmux.rs:3099-3126; g032/d6`; `crates/farhelm-supervisor/src/service/core.rs:6566`;
`crates/farhelm-supervisor/src/tmux.rs:4901-4903; crates/farhelm-supervisor/src/service/core.rs:14796-14895`;
`crates/farhelm-supervisor/src/tmux.rs:4901-4903;service/core.rs:14796-14874`;
`crates/farhelm-supervisor/src/service/teardown.rs:344-353,478,570-582`;
`crates/farhelm-supervisor/src/tmux.rs:4887-4903,5130-5137; tmux/presence.rs:259-291`;
`crates/farhelm-supervisor/src/service/core.rs:6566; crates/farhelm-supervisor/src/tmux.rs:3099-3126`;
`crates/farhelm-supervisor/src/tmux.rs:3099-3126;service/core.rs:6566`;
`crates/farhelm-supervisor/src/tmux.rs:4901-4903,5163-5164`;
`crates/farhelm-supervisor/src/tmux.rs:3099-3126; service/core.rs:6566`. References below use these paths' basenames
unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **highest**. Source-confirmed conditional behavior does not imply
an observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the
finding.

The assigned pane_states_checked change treats every is_definitively_empty error as an authoritative empty inventory and
calls presence.listed(started,empty) at tmux.rs4901-4903. means_no_panes includes ServerExited (5126-5137), classified
from exact raw 'server exited unexpectedly' at5163-5164. With ordinary adopted Settled(0) names and no concurrent old
create, listed removes every name and latches Absent (presence.rs259-270). Future commands are answered with synthetic
NoServer without spawning (tmux.rs2535-2552), and ordinary host-default activity cannot repair this cache; prior l2
history records the qualified exceptional keyed legacy retry and general supervisor-reconstruction repair.

The diagnostic does NOT prove that the server died. Official pinned tmux3.7c client.c584-592 sets
CLIENT_EXIT_LOST_SERVER and exitval1 whenever its IPC callback receives NULL, and210-211 maps it to the exact message
printed on stderr at443-444 for ordinary nonattached commands. proc.c84-117 sends that callback for IPC read/get/write
errors as well as EOF. A concrete supported client-local error exists in tmux's portability implementation:
configure.ac753-760 selects compat/imsg on platforms without libutil imsg_add; proc_add_peer enables fd passing at
proc.c325-327; imsgbuf_read delegates to msgbuf_read (compat/imsg.c75-80), which calls ibuf_read_process
(compat/imsg-buffer.c955-982). A valid message header is decoded with imsg_parse_hdr; ibuf_open(len) returning NULL at
compat/imsg.c446-447 propagates via imsg-buffer.c841-843,870-874. ibuf_open uses ordinary calloc at74-79 and returns
NULL on allocation failure. This allocation occurs in the separate short-lived tmux command client, not the server or
supervisor. No malformed message, unsupported private-server interaction, custom binary or server death is needed. The
server's EOF callback removes only that ordinary client (server-client.c433-512,2215-2217), not its sessions. A
transient client-local failure while the server/agent remain healthy is therefore a concrete source condition; actual
calloc timing/incidence remains unobserved.

The destructive continuation was traced for an already loaded old terminal, separately from l2's startup terminal-less
route. A normal list calls pane_states once (listing.rs177-202). An empty map makes terminal_evidence Absent
(status.rs210,223-234); a Running outcome with no live evidence becomes Exited (318-380), and entry_info builds an empty
tab list from that map (529-547). The ordinary UI's ended/zero-tab click skips confirmation and sends NothingAlive (UI
status.rs403-427; list/shared.rs63-74; list/view.rs2611-2642). The prior synthesis directly verified unchanged UI
API/helm/wire propagation (api.rs4088-4093; helm sessions.rs3078-3110,client.rs3847-3862; supervisor
handlers.rs3059-3074). At action time still_alive_for_delete DOES try a new process probe (core.rs14796-14808), but
Terminal::Tmux dispatches to pane_process (terminals.rs159-162), whose runner now synthesizes NoServer from the false
Absent cache; pane_process turns that into Gone (tmux.rs4192-4197). That fresh guard therefore cannot correct the false
listing. Legacy sessions with no host records/tabs pass the remaining checks (core.rs14824-14895). handlers.rs1245-1268
proceeds to teardown. Its own pane probe likewise returns Gone (teardown.rs344-353), other_pane_roots skips the falsely
absent name (199-200), and tab rediscovery is empty. It nevertheless retains entry.scope at478 and reaps units
at570-582. sweep.rs1399-1443 calls kill_scope for the recorded available Linux unit, whose1695-1717 path sends SIGTERM
to the still-existing launch scope. Thus this is stopping an unconfirmed live agent, not merely losing terminal access
or cleaning descendants of an agent already dead. Choose a normal scoped legacy session, no host records/tabs or
unrelated preflight failure, and no concurrent legacy create; these are ordinary supported premises.

SPEC.md Lifecycle operations1666-1680 requires NothingAlive refusal before stopping anything live; the partial-delete
allowance1631-1636 and leftover-descendant cleanup1683-1685 do not extend consent to the live agent.
SPEC_impl.md2828-2831 allows shrinking from a complete listing and suppressing traffic after actual server absence; a
generic lost-IPC diagnostic does not supply either fact. The stale prose calling ServerExited mid-teardown is an
inference, not evidence that only server teardown produces that message. Treat this diagnostic as unknown for permanent
presence publication, or verify absence independently without freezing the model from the client error. A regression
should preserve a live-server premise while returning this exact native diagnostic, then require the later guard/query
to reach the server rather than synthetic absence. No test/runtime experiment was run.

Suggested bucket: highest, because the complete shipped NothingAlive-to-recorded-scope SIGTERM route is established
conditionally. Confidence: source-confirmed conditional producer, cache transition and destructive continuation; runtime
allocation failure/timing/frequency unmeasured. No duplicate-agent, credential leak or additional file/data-loss claim.
proposed_drop false. possible_cover none in authorized latest peer records: l2=g031:f1 shares the false-Absent
downstream harm but originates in a startup socket probe; this new assigned listing update trusts a generic client
IPC-loss message during ordinary operation. Known g031:e2 instead misses absence publication after a real gone-server
capture and has bounded extra-spawn consequence. d3's pre-base potentially destructive socket replacement remains
separately excluded with its original claim intact.

Post-finding ancestry/check continuation for d6, not a stopping point. At exclusive
base8a8c9579e04299410652f0e4681975a7198dfc7b tmux.rs4699-4742, the same tolerated diagnostic returned one empty reply
at4716-4717 but did not record absence; the subsequent guarded Delete's process probe could still reach the live server.
At introduction f972458 tmux.rs5027-5029 the assigned hunk adds presence.listed(empty), surviving at tip4901-4903. The
permanent lie that defeats the later fresh guard is therefore introduced in range even though interpreting one query as
empty predates base. No claim that every inherited consequence of ServerExited is new. Tip status.rs556-588 confirms
computed Exited is actually assigned to the reply; a no-sentinel Running legacy entry with an empty snapshot follows
that branch. session_hosts.rs240-263 treats Terminal::Tmux as no held host and enumerates only recorded host launch
files, so an ordinary pre-host session with no such records supplies no independent host liveness check. All original
highest claims/exclusions remain preserved. Bucket highest/source-confirmed conditional unchanged; runtime
timing/unmeasured allocation and later successful cleanup remain explicit conditions. proposed_drop false;
possible_cover none for d6 root.

Continued original-scope search after recording d6. The marker-query tolerance at tmux.rs4928-4938 and4954-4960 does not
call listed(empty); the permanent cache publication being reported is the first fact-list query at4901-4903, not every
tolerated marker error. The classifier test6998-7079 pins ServerExited into means_no_panes but does not establish that
the peer server died; its string fixture cannot distinguish a client-local decoder allocation error from server death.
This is supporting test limitation, not a separate test-quality finding. proc_loop/proc_exit at pinned tmux
proc.c220-239 shows callback exit leaves the client event loop and returns to the exact stderr printer, rather than
invoking a server kill. Pinned server.c281-291 keeps a server with sessions running under ordinary exit-unattached-off
behavior. The private Linux build script uses musl targets/static config (scripts/build-tmux-assets.sh49-51;
build-private-tmux.sh84-96,142-160) and introduces no imsg override; the allocation producer is in stock portability
source already traced. No blanket claim across every libc/platform or every IPC error is required: the specific bundled
compatibility decoder's NULL result is the retained producer. An allocation failure in imsgbuf_init would instead fatal
with a different diagnostic and is NOT claimed as this route; d6 requires the later per-message ibuf_open failure after
client initialization. Actual allocation failure, server survival and later Delete sequence were not measured.
Source-confirmed conditional highest, proposed_drop false, possible_cover none. No additional discovered finding or
unresolved source premise from these checks; d3 and all older proposed exclusions remain preserved unchanged.

Additional premise check for already-known d6, not new discovery. Pinned server_client_check_exit sends ordinary
CLIENT_EXIT_RETURN as MSG_EXIT with the command retval at1932-1944; CLIENT_EXITED is a flag on that command client, not
evidence that the server or sessions died. On the receiving side, even this valid final reply passes imsg_parse_hdr's
ibuf_open allocation at446-447 before dispatch. client_dispatch_wait sets the client's own client_exitflag only after
decoding/dispatching MSG_EXIT (655-656). Thus failure of the per-message allocation for an otherwise normal return reply
reaches client_dispatch(NULL) while client_exitflag is still false and selects CLIENT_EXIT_LOST_SERVER/nonzero1
(587-589), rather than preserving a successful command result. No malformed protocol or initial imsgbuf_init failure is
needed. This check preserves the exact per-message NULL premise and independent server survival; it does not assert that
such an allocation failure was observed or broaden to every IPC closure. Existing source-confirmed conditional highest
and full NothingAlive route unchanged; proposed_drop false; possible_cover none for the root. d3 and other prior
proposed exclusions remain intact. Continue fresh search beyond all known roots.

Defensive interface synthesis only at reviewed tip407a49435278844248d573083fdea6e23fd96c02. Read existing g031:r5 full
history as requested. The parent retains its startup inventory variant under this same generic
ServerExited-as-authoritative-empty-inventory root. Preserve BOTH already documented consequences: incorrect startup
empty-server reconciliation without a Delete request, and the separately established later
cached-absence/guarded-cleanup consequence in this d6 history. The startup variant is documented in g031:r5 with source
ancestry and terminal-state teardown evidence; this entry incorporates that known evidence by reference without a
reproduction, operational example, new harmful technique or independent root count. The governing defensive distinction
is a conclusively empty server versus an unavailable inventory query. The existing per-message compatibility-decoder
allocation-after-initialization premise remains conditional, source-confirmed and unobserved; do not broaden to
arbitrary IPC error/platform behavior or describe a measured incident. Preserve highest, proposed_drop false,
possible_cover g031:r5 as the same root/additional known consequence. No prior exclusion or accepted-survival
distinction is weakened; d3 and original l1/l3/l4 histories remain intact. Latest user restrictions narrow subsequent
work to safe source/contracts only; no pending interface question. Continue fresh bounded correctness search beyond
known roots.

### Additional consequence: Do not kill an adopted server from a lost query client's empty-inventory inference

The known lost-query-client diagnostic has an additional startup consequence: Farhelm can mistake a still-running old
server for an empty one and issue kill-server itself. This destroys its old terminal sessions without any Delete
request. This is a newly verified destructive route of g032/d6's known classification root, not a new diagnostic root or
a measured incident.

The real constructor passes ownership.is_some to adopt_running_server (core.rs6566), so ordinary claimed startup
reconciles. After successful socket connect and floor checks, list-sessions at3099 can fail with the exact raw
ServerExited diagnostic. classify_tmux_refusal recognizes it at5163-5164; means_no_panes includes it5130-5137;
adoption's is_definitively_empty arm at3105 converts it to an empty name vector. The code has not yet published Absent;
its Unprobed presence permits the ordinary runner. names.empty then sends kill-server at3112-3119, and only afterward
sets Absent at3126. This is a direct destructive command, independent of the later cache-driven GUI NothingAlive route
in g032/d6.

The live-server producer is already established by g032/d6's full primary-source investigation, read in full: an
ordinary command client's per-message imsg buffer allocation can fail locally, trigger generic IPC callback NULL, and
print nonzero server exited unexpectedly while the independent server and its sessions survive. That decoder is common
to command clients, not specific to list-panes format parsing, so list-sessions has the same supported conditional
producer. Preserve exact compatibility-decoder allocation-after-initialization premise; no blanket platform/error claim.
This one client failure followed by an ordinary succeeding kill command is not a supervisor crash or two coinciding rare
faults.

Official pinned tmux3.7c cmd-kill-server.c49-56 sends SIGTERM to the server process itself
(https://raw.githubusercontent.com/tmux/tmux/3.7c/cmd-kill-server.c). server.c406-417 handles SIGTERM by setting
server_exit and calling server_send_exit;291-311 marks clients shutdown and destroys every session
(https://raw.githubusercontent.com/tmux/tmux/3.7c/server.c). Thus the server that still holds old terminals is
deliberately torn down, not merely hidden. No claim that every daemonized descendant must die is needed or made. Old
agent/tab terminal processes and terminal state are put through server teardown; follow-up source checks below will pin
window/pty cleanup.

Ancestry: f972 introduces this empty-inventory kill-server continuation at3238-3252. Base ensure_server3001-3117
performs start/adopt, floor checks and options, with no adoption name-list->kill-server branch. The new harm is
therefore in range, not just an inherited generic diagnostic. SPEC_impl.md Terminal substrate2818-2831 permits ending an
adopted server with no sessions; it does not authorize destroying sessions merely because one client's IPC read failed.
SPEC.md Lifecycle operations1666-1680 permits destructive operations only within their user-authorized action, and this
path has no such action. The exception1715-1727 for exact-moment supervisor crash/two independent faults does not apply
to this single local query failure.

Suggested bucket highest for deliberate destruction of live user terminal sessions/work. Confidence source-confirmed
conditional producer inherited from peer and locally traced command/admission/destructive server route; no runtime
allocation failure, timing, incidence or killed-process count measured. proposed_drop false for the additional route;
possible_cover g032/d6 same false-ServerExited-as-authoritative-inventory root. Consolidate this variant with that
finding while preserving r5 and d6 IDs, rather than count an independent root. A repair must audit startup empty-server
kill as well as pane_states permanent publication; fixing only listed(empty) leaves this route. No runtime experiment or
checkout edit.

Further r5 proof and coordinate precision, not a new independent issue. Reviewed
tip407a49435278844248d573083fdea6e23fd96c02. is_definitively_empty at4974-4975 delegates exactly to means_no_panes; the
ServerExited branch is therefore reachable in adoption. Raw upstream source line numbers differ from web-extracted line
numbers in preceding event; raw bounded reads now give cmd-kill-server.c54-60 ->server SIGTERM, server.c431-441
->server_send_exit, server.c310-329 ->session_destroy for every session. session.c197-229 removes every window link;
window.c196-207 removes each reference;396-403 destroys the window when its last link disappears;345-370 calls
window_destroy_panes;954-969 destroys each pane;1090-1112 frees its PTY bufferevent, closes its fd and frees the base
screen/history. Thus all old server terminal state is actually removed, not merely a new status display. Ordinary pane
programs lose their controlling PTYs; no universal claim about daemonized descendants or every program's signal
behavior. The confirmed terminal-session/state destruction suffices for highest, with actual query allocation
failure/timing unmeasured. Bounded raw sources: https://raw.githubusercontent.com/tmux/tmux/3.7c/session.c and
https://raw.githubusercontent.com/tmux/tmux/3.7c/window.c plus preceding cmd/server URLs. First web find result returned
broad server text, so exact needed functions were re-read with bounded source commands before this final proof. Base
ensure_server source read through3122 returns after its hook, completing absence of the new empty-adoption kill branch.
Suggested highest/source-confirmed conditional unchanged; proposed_drop false; possible_cover g032/d6 same root with
additional startup destructive continuation, preserve IDs. No unresolved r5 premise remains; no runtime experiment,
measured incident or count claimed.

Incoming synthesis before the newly requested discovery, at tip407a49435278844248d573083fdea6e23fd96c02. Parent
explicitly consolidates r5 with g032/d6's generic ServerExited-as-empty-inventory root while preserving both variants:
startup claimed adoption sends kill-server before Absence publication, without Delete; later steady-state pane_states
publishes false Absent and defeats the fresh NothingAlive probe before recorded Linux scope SIGTERM. Read newest g032/d6
producer check: a normal MSG_EXIT return reply passes per-message allocation before client_dispatch_wait sets
client_exitflag, so a valid final reply allocation failure can still become CLIENT_EXIT_LOST_SERVER/nonzero1 while
server survives. This reinforces, without broadening, the exact conditional client-local producer already cited by r5.
No measured allocation/timing/incident or platform-universal failure claim. Source-confirmed conditional highest
retained, proposed_drop false for this additional startup route; possible_cover g032/d6 same root, retain both IDs and
do not count independent new roots. Not an exact-moment supervisor crash or two coincident independent faults: one query
failure then ordinary successful kill.

Read all latest g028 records in full, including d4 pre-base Gone-host retained-direct-child consent mismatch, d8
explicitly accepted short PID-binding race, and s6 false-Absent loaded-row corroboration. They are separate causes or
exact known corroboration; none repairs/refutes the startup raw kill or the later cached guard, and none permits
dropping their original potential-highest implications. g028/d6 settlement and g025/d19 capped local projection
exclusions remain narrow; temporal epochs (g033/c7/i20) do not authenticate the false inventory. g033/l21 refutes its
own watch lock inversion, not the error producer. No new issue is credited to this synthesis and no previous
confidence/consequence is lowered. No pending interface question; new discovery proceeds beyond all known roots.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g032/d6`, `g031/r5`,
`g009/g009-i18`, `g026/i1`, `g024/s4`, `g025/i17`, `g028/g028-s6`, `g033/i20`, `g009/g009-i20`, `g024/s5`, `g025/i21`,
`g028/g028-s7`, `g031/r1`, `g033/i23`. The retained append-only histories preserve all earlier claims and corrections.
Reviewed source references name their own revisions; the inclusive reviewed tip is the hash above. This review used
source inspection only, with no product runtime experiments or live-install changes.
