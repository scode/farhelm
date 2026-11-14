# Old terminals and their running work end on upgrade (specification-accepted; proposed drop)

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Updating a supervisor ends programs still running in old terminals and discards their terminal
screen and scrollback.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g170/f1`, `g171/f1`.

### g170/f1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 3 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: Updating a supervisor ends programs still running in
old terminals and discards their terminal screen and scrollback. Details: The assigned deletion removes the tmux
control-client replay/output path (parent stream.rs lines 1–1205 in the assigned packet). SPEC.md, Terminal experience,
lines 3003–3016 explicitly accepts this consequence. Exact quoted text (line breaks preserved): 'This release ends them:
the first time a host's supervisor of this release starts, it ends that tmux server, and with it everything that still
ran in an old terminal, without needing a tmux program.' The paragraph also states: 'A session whose agent or tabs
already run in terminal hosts keeps them.' Source validation: service/core.rs:6112–6184 opens/checks the store and gates
server termination on state-directory ownership before reload; :6675–6704 gives old rows no terminal while adopting host
rows independently; :6805–6822 converts live old rows, and :7042–7090 sweeps before committing, then records notices.
service/core/old_terminals.rs:176–279 identifies and terminates the old socket peer; :290–408 uses
agent-only/tab-specific sweeps, with terminal-host tab records excluded from scope discovery. The parent revision
already calls OldTerminals::end_private_server (core.rs:6434) and maps TerminalKind::Tmux to None (:6969), so deleting
stream.rs removes an already-retired runtime rather than introducing this startup consequence. Suggested bucket: highest
for the original loss of running user-owned processes/work; could-be-highest: yes. Confidence: confirmed
source/specification analysis, no execution. proposed_drop: explicit specification acceptance and source refutation of
introduction by the assigned deletion; independent check required because the original consequence can be highest.
possible_cover: none; no coverage sources searched. Caveats: the charter requires potentially highest consequences
accepted solely by specification to remain findings proposed for drop. This record is not a claim of an implementation
deviation; the retained host path is distinct. Revision reason: source investigation settled startup boundaries and
confirmed the retirement already exists in the parent; original loss-of-work consequence preserved.

Returned proposal (claims-107, event 3): **covered**, suggested bucket **highest**.

Ending old terminals, their running programs and their terminal contents on upgrade is explicitly accepted. This
acceptance is limited to those old terminals; it does not cover unrelated processes or surviving terminal hosts.

Original stronger consequence: An upgrade ends running user-owned programs in old terminals and discards their screen
and scrollback. Highest remains the consequence bucket even though the specification accepts it.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Explicit specification acceptance for old-terminal retirement only, plus no additional loss attributable
to deleting the already-unused stream. Do not call the retirement out-of-range merely because the assigned deletion's
parent already implements it.

Existing parent disposition (event 3): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g170 record](records/review-through-00de0b6c/groups/g170.json). Record ID: `f1`.

### g171/f1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 11 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333

## TLDR

The original potentially highest consequence remains: an upgrade ends user programs in old tmux terminals and loses
their retained terminal contents. Retirement is expressly accepted, but that acceptance does not authorize harming
surviving terminal hosts or unrelated processes. This assigned deletion removes an already-unused stream; startup
retirement is a distinct attribution question. No drop has been accepted.

## Details

Coverage-repair/synthesis revision to f1, preserving the original consequence and all prior proposed-drop bases.
Assigned patch 00957-001.patch:1-1205 deletes next_output (parent stream.rs:1206), paused replay (:1500), shutdown
(:1659), reaper (:1677), and tmux-specific tests. Seven raw evidence stdout packets in turn 2 now deliver the entire
assigned deletion.

Exact accepted behavior, pinned SPEC.md:3003-3016: "This release ends them: the first time a host's supervisor of this
release starts, it ends that tmux server, and with it everything that still ran in an old terminal, without needing a
tmux program." Also: "one whose agent had already ended keeps its outcome, but its terminal's last screen, which lived
in the server, is gone; old tabs are closed." Also: "After an update to this release, Restart, Clone and Replace start
such a session in a new, empty terminal." The same paragraph requires terminal hosts to survive: "A session whose agent
or tabs already run in terminal hosts keeps them."

Attribution check: old_terminals.rs has identical Git blob 910e414971731b1401b2a6af95a01cfd1559f583 at this commit and
its parent, confirming this deletion commit did not alter that retirement module. Separate source checks for attribution
and targeting: service/core.rs:6141-6185 already ends the private server before reloading; :6675-6680 never resolves a
stored tmux address, while its Host branch retains adopted or unavailable host runtimes; :6805-6820 converts live
old-kind rows only. service/core/old_terminals.rs:181-277 connects only to state_dir/tmux.sock, verifies same-user peer
credentials and process identity, and obtains its subtree only for the verified server. hosts/process.rs:181-184
recognizes the exact socket following -S; :476-500 resolves that socket absolutely and verifies before SIGTERM/SIGKILL;
:544-577 checks pinned start identity both before and after reading argv. old_terminals.rs:300-347 uses AgentOnly for
converted rows and exact Tab targets for old tabs; :363-410 excludes scope ids with terminal-host launch records.
service/sweep.rs:247-260 requires the session marker for every target and AgentOnly excludes tab-marked roots. These are
actual targeting checks, not an inference from retirement acceptance. No wrong-target or surviving-host harm route was
found in these calls. This is not a proof against every possible pre-existing sweep defect.

Host replacement identity is independent: service/core/host_handover.rs:645-808 selects Host rows, preserves already
adopted runtimes and tab sockets, scans under the spawn lock, matches agent tags to session plus generation, and adopts
over the scan's existing connection. service/terminals.rs:125-153,232-255 resolves only HostTerminal;
service/handlers.rs:1928-1958 sends only grids. Removed stream symbols have no surviving executable callers (only a
historical comment in connection.rs:986). Thus the deletion itself supplies neither a live tmux client to lose nor a new
cleanup target.

Suggested bucket: highest/loss of user-owned processes and terminal contents if the original consequence violated the
contract, or if wrong-target/surviving-host harm were established. Could-be-highest: yes. Confidence: confirmed accepted
retirement and source-supported attribution of the assigned deletion; no independent wrong-target route established.
proposed_drop: retain exact retirement acceptance for the old terminals only, plus source refutation of an additional
loss caused by this deletion. Acceptance is not the basis for excluding surviving-host harm. possible_cover: none;
forbidden coverage sources not searched. Source-only analysis; no runtime execution or reproduction. No unresolved
concrete lead for f1; broader prior-retirement defects cannot be excluded by reviewing this deletion alone.

Returned proposal (claims-107, event 11): **covered**, suggested bucket **highest**.

Same accepted retirement consequence as g170/f1. The source also refutes an additional supported replay/viewing route
lost solely through deleting the unused stream. Original running-work and history loss remain explicit.

Original stronger consequence: An upgrade ends user programs still in old tmux terminals, loses their retained terminal
contents and prevents viewing those contents afterward. Acceptance does not authorize harming surviving terminal hosts
or unrelated processes.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Exact acceptance for old-terminal retirement plus absence of an additional live stream dependency.
Independent agreement is still required; this consultation authorizes no drop.

Existing parent disposition (event 11): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g171 record](records/review-through-00de0b6c/groups/g171.json). Record ID: `f1`.
