# Recognize a server that disappears during its version probe

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

If the old terminal server disappears after the supervisor's initial socket check but before its server-version query,
Farhelm can refuse Restart and Delete of its old sessions for the rest of that supervisor run. The error says a running
unusable server holds them even though tmux already reported that no server exists. Restarting the supervisor is needed
to clear this false refusal.

## Details

Source coordinates from the original records:
`crates/farhelm-supervisor/src/tmux.rs:3084-3097; crates/farhelm-supervisor/src/tmux/presence.rs:153-159`. References
below use these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

adopt_running_server first accepts a plain socket connection (3075-3082), then awaits the client-version check before
asking the server version (3084-3086). require_supported_server runs display-message through the typed ordinary command
runner (3384-3388). If that returns NoServer or SocketMissing because the old server crashed/OOMed in the intervening
interval, the error is propagated intact through anyhow context. The existing classifier explicitly recognizes these
exact raw messages and documents crash/OOM as a server-gone case (5088-5097,5147-5162). Nevertheless the outer
if-let-Err stores every version-probe error as Unusable and returns Ok (3088-3097), instead of handling definitive
absence as the following list-sessions branch already does at3105. observe_failure cannot repair this during the initial
probe: presence starts Unprobed, and no_server mutates only Running (presence.rs:280-291). Once set Unusable, synthetic
empty listings also cannot clear it because listed only mutates Running (259-270).

The supervisor then loads the old rows with no terminals, but Restart reads their durable tmux kind and refuses on
unusable_reason at service/core.rs:13117-13135. has_session/kill_session likewise refuse, so old-row Delete reaches the
same stale unusable refusal; d1 does not re-claim known f4's unconsented process kill, because the server/old agents
here are actually gone. No new processes or retained history are lost through this additional misclassification.

The trigger is a single old-substrate disappearance between two startup observations, not unsupported direct socket
manipulation or a supervisor crash. The repository itself models server crash/OOM as an ordinary NoServer diagnostic
(tmux.rs:5088-5091). This is a source-confirmed conditional interleaving, not a measured incident or frequency claim.
SPEC.md Lifecycle operations:1553-1555 says Restart starts an empty terminal when the prior one no longer exists.
SPEC_impl.md Terminal substrate:2830-2836 distinguishes a gone server from an answering unusable server whose agents may
still run. The protective refusal applies to the latter, not a version request that positively established the former.

Fix direction: before set_unusable, classify a server-probe error that positively says NoServer/SocketMissing as absent
and finish adoption normally; preserve uncertainty for genuine support/spawn/unclassified errors. The stricter
alternative is returning the gone error to abort startup, allowing an ordinary retry, rather than permanently labeling
it an answering incompatible server. A focused regression can inject a successful connect/client-floor probe followed by
typed NoServer on the server-version request and assert that old-session Restart is not barred by a stored unusable
reason. No runtime experiment was allowed or run.

Suggested bucket: other, for a rare false lifecycle refusal requiring a supervisor restart after the old substrate was
already lost; no established security or additional data/process-loss consequence. Confidence: confirmed source-level
conditional bug; actual crash/probe timing unmeasured. proposed_drop: false. possible_cover: none in the explicitly read
peer records. Distinct from known f1 (live server wrongly Absent) and f4 (correctly Unusable server admits destructive
work).

Requested further source recheck of d1 at pinned407a49435278844248d573083fdea6e23fd96c02. The new g032/d2 evidence does
not refute d1: official tmux3.7c cmd-display-message.c:34-45 has CMD_AFTERHOOK|CMD_CLIENT_CFLAG|CMD_CLIENT_CANFAIL and
NO CMD_STARTSERVER, whereas cmd-attach-session.c:32-44 explicitly has CMD_STARTSERVER|CMD_READONLY. Sources read in full
via https://raw.githubusercontent.com/tmux/tmux/3.7c/cmd-display-message.c and
https://raw.githubusercontent.com/tmux/tmux/3.7c/cmd-attach-session.c. Thus the adoption's display-message version probe
can report real server absence instead of starting an empty replacement, while later -C attach can create the separately
known idle server from stale predecessor config. d1 remains other, source-confirmed conditional false lifecycle refusal;
actual disappearance timing unmeasured. No additional lost work or process consequence. proposed_drop false;
possible_cover g009/g009-i11 corroborates d1, not a new root. g032/d2 and g033/f8 remain separately known bounded
findings. This is a revision to existing d1, not a new discovery ID. Next fresh search: changed runner/refusal paths and
server-initialization/config consumers beyond all named known roots.

End-of-search interface check at reviewed tip407a49435278844248d573083fdea6e23fd96c02; d1 itself remains unchanged
other/source-confirmed conditional, no measured incident. Official cmd-display-message has no CMD_STARTSERVER while
attach explicitly has it (source check in preceding d1 revision), so known g032/d2's stale-config idle autostart is a
separate control-attach path. Its one-idle-process limit is retained; no churn/user-process loss inferred. Locally
reread presence.rs:130-164 and wrapper tmux.rs:2593-2638 from retained source: g033/f8 correctly identifies the doc
mismatch, because may_hold=false includes Unusable but knows_absent=false means those sessions are unknown. Correcting
the prose does not repair d1's typed-NoServer -> Unusable transition, f1's false Absent, or f4's consent gate. No new
documentation issue is claimed here; possible_cover g033/f8 and g009/g009-i11 for that known corroboration only.

Read exact latest g025/d10 and g026/l4: the pre-base direct-reader removal competes with the in-range aa62b33 notice
owner, while its normal runtime-close premise establishes only the wrong notice (other). It does not establish
additional user-process/work loss or change d1. Read g037/d1's full source/ancestry record and retain parent's pre-base
scope exclusion, not a new correctness refutation. Known g029/l1=g036/l1 scope provenance stays independently handled.
These records were used to avoid duplicate discoveries.

Fresh e2 is another distinct negative-observation edge: a capture has already received NoServer but never publishes it
to Running presence; the next sampler capture can therefore run. d1 instead publishes an incorrect Unusable during
Unprobed startup and then refuses lifecycle operations. Their fixes touch different error-publication contexts even
though both should honor typed absence. f1/f4 remain separate highest roots and are not re-bucketed. proposed_drop false
for d1; no proposed drop or lowering of any peer finding. No unresolved interface premise remains.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g031/d1`. The retained
append-only histories preserve all earlier claims and corrections. Reviewed source references name their own revisions;
the inclusive reviewed tip is the hash above. This review used source inspection only, with no product runtime
experiments or live-install changes.
