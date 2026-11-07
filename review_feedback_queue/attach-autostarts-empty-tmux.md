# Prevent terminal attachment from restarting an empty old server

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

After upgrading a host with old terminals, an attachment that races the old terminal server going away can start a new
empty tmux server. Farhelm then stops querying it but leaves that unnecessary process running, contrary to the promise
that tmux ends with the last old terminal. No agent or user-work loss is claimed.

## Details

Source coordinates from the original records: `crates/farhelm-supervisor/src/tmux.rs:2805-2844,3218-3219,3606`.
References below use these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

The predecessor's generated on-disk config explicitly sets exit-empty off (f972458^ tmux.rs:2547-2550). Adoption now
reconciles exit-empty on only on the running server and does not rewrite that file (tip tmux.rs:3075-3140,3196-3206).
prepare_server_start writes the new exit-empty-on config only for explicit old-terminal creation (3210-3259,3600-3607),
so a normal Host-default upgrade can retain the old file indefinitely while driving an adopted supported server.

The assigned input change checks only cached presence before spawning tmux -C attach (input.rs:35-51); the replay path
has the same shape (tmux/stream.rs:271-299). Stock tmux 3.7c attach declares CMD_STARTSERVER (official
cmd-attach-session.c:36-47). client.c:248-273 turns that into CLIENT_STARTSERVER, and client_connect:125-164 starts a
server after ENOENT/ECONNREFUSED. The plain presence guard cannot synchronize the existing server's lifetime with that
later connect. A concrete shipped caller is prompt delivery: it queries paste mode, then opens the input client
(core.rs:12628-12640). If the adopted server dies between that successful query/guard and attach's connect, attach can
start a fresh empty server using the stale -f config. The attach cannot find its named session and fails; the empty
server nevertheless retains exit-empty off. Pinned server.c:281-282 returns without exiting whenever exit-empty is off,
regardless of an empty session tree. No test-only old-terminal create or unsupported hand edit is required: the file was
written by the previous ordinary build, and a tmux process can go away independently while the supervisor remains up.
Actual crash/connect timing is unobserved; this is a conditional source route, not a measured occurrence.

The next pane_states query classifies the empty server as empty and calls presence.listed with an empty set
(tmux.rs:4887-4903); that sets Absent. Nothing in that path kills the still-running empty server. The new startup-only
empty-server kill (3112-3131) does not run again in this supervisor. Thus the process can remain until a later
supervisor reconstruction adopts/cleans it, even while all remaining terminals use hosts. The same race is harmless to
this lifetime property when the disk config was already prepared with exit-empty on; the stale predecessor config is the
relevant upgrade premise.

SPEC.md, Terminal experience, 2523-2525 says the private server runs only while it holds old terminals and ends with the
last one. SPEC_impl.md, Terminal substrate: private tmux server, 2818-2830 says likewise and says explicit new-session
is what starts a server. That no-idle-server promise is the new behavior; keeping an empty server was intentional in the
parent, so this is a missed path in the changed policy rather than alleging the parent violated its old contract.
Prevent implicit autostart on attachment clients (tmux's -N), or otherwise ensure an attachment cannot recreate the
empty stale-config server. Updating the generated config on adoption is another relevant mitigation, though no
implementation is authorized here.

Suggested bucket: other, for a stray idle process/resource/lifetime-contract defect; no material work-loss,
duplicate-agent, credential or security consequence established. Confidence: source-confirmed conditional behavior from
Farhelm and pinned tmux; runtime incidence/timing unobserved. proposed_drop false. possible_cover none in the latest
records read from g031,g033,g034,g035,g036,g037,g038,g027,g010,g024. The initial wait-for and capture variants were
refuted by command flags (wait-for flags=0, capture only CMD_AFTERHOOK); preserve those exact refutations separately
from this attach finding. This is genuinely new discovery evidence, not the known startup false-Absent or late-Unusable
Delete finding.

Final source checks for d2 at reviewed tip 407a49435278844248d573083fdea6e23fd96c02. Exact predecessor source was read,
not inferred from comments: f972458^ tmux.rs:2547-2557 emits exit-empty off and 3070-3077 writes that file before
starting/adopting the old server. Introduction f972458 tmux.rs:3234-3265,3322-3333 reconciles live options without
rewriting the file, same behavior as tip 3075-3140,3196-3206. The new preparation write is separate and only reached on
an explicit old create.

Pinned tmux source confirms configuration is loaded for an implicitly started control client before its attach command:
tmux.c:418-433 retains CLIENT_CONTROL and the supplied -f file; server-client.c:2573-2581 calls start_cfg for the first
non-exiting client; cfg.c:71-90 blocks that client's command until cfg_files have been loaded. Nothing about -C cancels
CMD_STARTSERVER. tmux.c:448-450 sets CLIENT_NOSTARTSERVER for -N, and client.c:128-133 checks it before autostart,
supporting the proposed mitigation. cmd-wait-for.c:40 has no start flag and cmd-capture-pane.c:51 has only
CMD_AFTERHOOK; those original d2 sub-routes stay refuted.

Clarify empty-list observation references: pane_states_checked can learn the new server is empty either through a
tolerated empty diagnostic at 4901-4903 or a successful empty fact map at 4908-4920. Neither branch kills it.
cmd-list-panes.c:51-92 enumerates all sessions/windows for -a; no rows exist on the implicitly started empty server.
server.c:281-282 does not exit with exit-empty off. Consequently presence can stop future tmux traffic while the idle
process remains. This predicts one leftover server, not unbounded server creation, traffic on every poll, resurrected
terminals, or lost user processes. A later supervisor startup can kill that empty server.

The changed tests' ordinary ScratchServer constructor prepares a fresh exit-empty-on config
(tmux/test_support.rs:53-84); the assigned normal creation and server-exit tests therefore do not exercise this
retained-predecessor-file boundary. This is supporting evidence, not a separate test-quality finding. No runtime
experiment was performed. Suggested bucket other; confidence source-confirmed conditional route, actual
server-loss/connect timing unobserved. proposed_drop false. possible_cover none in all authorized peer latest records.
d2 is resolved as a new finding; no missing accessible source premise remains. No higher consequence is claimed.

Additional d2 source check at reviewed tip 407a49435278844248d573083fdea6e23fd96c02: startup calls
sweep_tmux_config_temp_files at core.rs:8548, but its exact prefixes at 4430 are .tmux.conf.tmp- and
.running-version.tmp-. The predicate/remove at 4442-4445 therefore leaves the actual tmux.conf untouched. This is not a
hidden migration that replaces or deletes the predecessor exit-empty-off file. Keep d2's exact ancestry and
source-confirmed conditional one-idle-server consequence; actual server-loss/connect timing and occurrence remain
unmeasured. Suggested bucket other, proposed_drop false, possible_cover none in the authorized latest peer records. This
is verification of the already-known d2, not a new finding or a higher consequence.

At reviewed tip 407a49435278844248d573083fdea6e23fd96c02, quiet_control_clients_before_exit (2805-2844) snapshots
control clients and disables output; it does not end an empty server or rewrite its config. The source-wide
prepare_server_start/prepare_server_start_with_seam call search finds the explicit create get_or_try_init at 3606, test
calls at 6517/6587/6624/6657/6939, and the test_support constructor, with no separate adoption or polling writer. These
checks do not undo d2's retained stale-config premise. This does not claim an exhaustive whole-service shutdown audit or
that a later supervisor startup cannot clean the idle server: that later cleanup remains explicitly acknowledged.
Other/source-confirmed conditional route, one idle server, runtime occurrence unmeasured; proposed_drop false;
possible_cover none. No newly discovered consequence or independent finding.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g032/d2`. The retained
append-only histories preserve all earlier claims and corrections. Reviewed source references name their own revisions;
the inclusive reviewed tip is the hash above. This review used source inspection only, with no product runtime
experiments or live-install changes.
