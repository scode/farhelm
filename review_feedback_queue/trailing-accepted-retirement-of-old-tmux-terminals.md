# Deleting tmux history capture tests adds no further history loss

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Deleting tmux history capture tests adds no further history loss

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g166/c4`, `g167/l1`.

### g166/c4

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 21 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333

## TLDR
Considered whether removing the tmux scrollback capture and seed tests loses terminal history on Restart. Host-to-host Restart still obtains a history seed; old tmux sessions already had no terminal to read at the parent revision.

## Details
Assigned patch lines 630-1351 remove history capture, locale, bounded capture and styled-seed fixtures/tests. At the parent revision, service/core.rs:6426-6438 ends the private server and disables its driver before reload, and :6965-6969 already maps TerminalKind::Tmux to no terminal. At the reviewed revision, service/core.rs:12486-12493 still obtains a restart seed when the entry has a terminal. service/core/host_restart.rs:418-485 reads the old host's history seed with MAX_SNAPSHOT, after bounded settling; :199-220 uses that seed for the new host. Thus the deleted tmux tests cover a retired capture path, not the remaining host-to-host history seed. The original suspected consequence is lost user terminal history (potential highest); this deletion does not newly cause it.

SPEC.md, Terminal experience, explicitly says: 'After an update to this release, Restart, Clone and Replace start such a session in a new, empty terminal.' The same text is already in the parent revision at lines 3015-3016. Lifecycle operations further states: 'When the terminal no longer exists (the process that kept it ended, after a reboot, or an old terminal this release ended, see [Terminal experience](#terminal-experience)), Restart starts with an empty one.' The candidate is source-refuted as an added removal consequence; if treated instead as a challenge to losing old tmux history at all, these are the exact acceptance texts for an independent drop check.

Suggested bucket: highest if an unaccepted loss of retained user terminal history were established. Could-be-highest: yes. Confidence: confirmed source analysis; no execution. proposed_drop: source refutation of regression in this deletion, since old tmux terminals were already ended and unadoptable before it, while host history seeding remains. possible_cover: exact SPEC.md acceptance quoted above, without searching any other coverage source. Caveat: sibling groups own the earlier server-retirement change and the snapshot encoder itself.

## Synthesis revision
The original loss-of-user-terminal-history consequence and all source refutation above are preserved. This is now a finding with proposed_drop, for the independent check; no drop is accepted. proposed_drop: SPEC.md, Terminal experience, explicitly accepts: After an update to this release, Restart, Clone and Replace start such a session in a new, empty terminal. The exact quoted acceptance text and the Lifecycle operations exception are retained above. The earlier retirement supplies the provenance; this deletion removes the now-unreachable capture path. The loss is not a new discovery this turn. Suggested bucket remains highest for the original loss-of-work consequence, with the acceptance awaiting independent judgment. Confidence: confirmed contract and source analysis, not execution.

## Surviving test responsibility
The retained host seed test at service/core/tests/host_terminals.rs:3817-3849 restores the seed, requires early history, excludes the on-screen final line and requires an empty screen, then checks that subsequent output reaches the runtime. The end-to-end host_relaunches.rs:490-569 test checks a new socket, retained early history, exclusion of both final-screen markers from history and screen, reset modes/title, persisted generation/address and confirmed termination of the old host. The failed-replacement test at host_relaunches.rs:574-665 checks that a definitively refused replacement retains the old host and its screen, including adoption by the next supervisor. Thus deletion of tmux-only history fixtures does not remove the only assertions for surviving host history transfer or failure preservation. These checks do not certify every style/encoding edge case; snapshot encoder coverage belongs to its assigned review groups. Original potential-highest history-loss consequence, exact acceptance and finding/proposed_drop status remain unchanged.

Discovery pass checked surviving seed assertions separately: farhelm-terminal-emulator/tests/snapshot.rs:1252-1312 creates colored history, a link and wrapped line, active modes/title/colors and a partial sequence on the alternate screen; it requires blank restored screen/default state, equal history size and compares every old primary-history cell to the restored history with cell_eq. This retains a concrete host-seed oracle rather than inferring test adequacy from correct implementation. The assigned removed tmux-specific Carried style decoding, UTF-8-client format separator and capture retry/budget mechanisms have no remaining crate references by their deleted symbols at the reviewed commit. No additional missing surviving responsibility was established in this assigned packet. Original highest history/process-loss consequence and exact proposed-drop acceptance remain unchanged; no drop accepted.

Returned proposal (claims-105, event 21): **covered**, suggested bucket **highest**.

The original history-loss consequence is real for retired tmux terminals and explicitly accepted. The narrower claim that deleting tmux capture tests newly removes surviving host history transfer is source-refuted. This is a proposed disposition, pending the independent drop check.

Original stronger consequence: Lost retained user terminal history on Restart; the history also preserves a potential-highest process-loss concern. Do not replace that wording with a test-cleanup-only consequence.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Exact specification acceptance of retiring old tmux work/history; separately, source and surviving test assertions refute an additional host-history regression caused by this test deletion.

Existing parent disposition (event 21): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g166 record](records/review-through-00de0b6c/groups/g166.json). Record ID: `c4`.

### g167/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 6 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: The upgrade ends programs still running in old terminals and discards their screen/scrollback; failed cleanup can instead leave them inaccessible, with Restart starting a second agent beside them. Preserve this original potentially highest consequence for independent proposed-drop review. Suggested bucket: highest if treated as a bug; could-be-highest: yes, user-owned running processes/work. Confidence: confirmed by source, not execution. Source checks: service/core.rs:6112-6117 checks the store first; 6142-6151 ends the private server only under state-directory ownership, before 6168-6184 reloads sessions; 6675-6679 treats old terminal rows as terminal-less; 6805-6822 converts live rows; 7042-7090 sweeps then commits conversions/notices. core/old_terminals.rs:176-233 identifies the socket peer and invokes verified termination without needing this codec. service/terminals.rs:225-248,259-265 resolves only host terminals and explains absence at 195-207. Pinned git grep across crates found no remaining references to control_codec/read_command_block/PassthroughDecoder/parse_pane_facts/unescape_control_output. Final attribution check: git diff of core/old_terminals.rs against this commit's parent is empty; the parent already calls end_private_server (parent core.rs:6434), returns no terminal for Tmux (6969), converts TerminalEnded (7110), and records the notice (7382). Thus this group's decoder deletion does not newly end or strand a supported terminal; retirement was already operative. proposed_drop: both source refutation of introduction by this deletion and explicit specification acceptance of the preserved consequence. Exact pinned SPEC.md Terminal experience, lines 3003-3016: 'This release ends them: the first time a host's supervisor of this release starts, it ends that tmux server, and with it everything that still ran in an old terminal, without needing a tmux program.' Also: 'one whose agent had already ended keeps its outcome, but its terminal's last screen, which lived in the server, is gone; old tabs are closed.' Also: 'A failure to end the server or to stop what ran there is logged and never stops the supervisor from starting; those sessions are treated as ended all the same, accepting that a Restart could then start a second agent beside one still running.' Pinned SPEC_impl.md Terminal substrate: private tmux server, 2882-2889: 'This release ends them at startup (below); every terminal runs in a terminal host' and 'The running supervisor runs no tmux at all, and its driver of the old terminals is gone'. possible_cover: these exact specifications only, no coverage sources searched. Caveats: independent caller drop check still required because the preserved consequence could be highest. No runtime tests or reproductions. Lead settled; no missing source premise remains for the assigned removal implication.

Returned proposal (claims-105, event 6): **covered**, suggested bucket **highest**.

Ending old tmux programs, losing screen/history, and treating failed cleanup as ended are explicit product decisions. Removing the now-unused decoder adds no still-supported terminal path loss. Retirement itself was introduced inside this review range, so immediate-parent preexistence alone is insufficient; exact acceptance supplies the broader basis.

Original stronger consequence: Upgrade ends programs still running in old terminals and loses their last screen/scrollback; failed cleanup may leave them inaccessible and let Restart start a duplicate agent. User-owned processes/work make this highest if unaccepted.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Exact current and reviewed specifications accept the full preserved consequence; source refutes an additional loss introduced by deleting this unused decoder.

Existing parent disposition (event 6): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g167 record](records/review-through-00de0b6c/groups/g167.json). Record ID: `l1`.

