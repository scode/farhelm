# Retiring tmux intentionally ends running legacy terminals

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Retiring tmux intentionally ends running legacy terminals

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g169/f1`.

### g169/f1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 12 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR (original wording preserved): Updating a host to this
release ends any agents and terminal tabs still running in the old terminal system and loses their live terminal
contents; they do not survive the supervisor upgrade. Details: This potentially-highest consequence was considered while
reviewing sink/driver removal. core.rs:6147-6151 calls OldTerminals::end_private_server before reload;
core/old_terminals.rs:168-172 calls end_server; core.rs:6675-6679 deliberately resolves every old tmux row to no
terminal. Synthesis checked conversion :6805-6825 and sweep/commit/notice ordering :7036-7094: legacy live launches are
proposed as TerminalEnded, sweeping precedes transition commit, and the ended notice follows a committed interrupted
outcome. Exact SPEC.md, Terminal experience, :3006-3008: 'This release ends them: the first time a host's supervisor of
this release starts, it ends that tmux server, and with it everything that still ran in an old terminal, without needing
a tmux program.' Its paragraph explicitly discards an ended agent's last screen and closes old tabs. Exact SPEC.md,
Durability and resume, :3471-3473: 'only a host reboot takes sessions down (and, once, the update that ended old
terminals; see Terminal experience).' Known related residual, now made explicit under this same provenance: failed
legacy termination can leave a still-running old agent and allow Restart to start a duplicate. Exact acceptance in
SPEC.md :3014-3016: 'A failure to end the server or to stop what ran there is logged and never stops the supervisor from
starting; those sessions are treated as ended all the same, accepting that a Restart could then start a second agent
beside one still running.' SPEC_impl.md Terminal substrate :2880-2991 specifies both the retirement and this residual.
This is not new discovery in synthesis. Suggested bucket: highest/loss of user-owned processes, live screens/history and
potentially concurrent agents, if treated as a defect. Confidence: confirmed intended source behavior; no execution
observed. proposed_drop: only the exact deliberate-retirement and duplicate-on-failed-termination consequences above are
accepted by those specification exceptions. This remains a proposal, requiring the caller's independent highest-bucket
check; no drop is accepted yet. possible_cover: cited SPEC.md/SPEC_impl.md retirement contract only; excluded coverage
sources not read. Could-be-highest: true. Material caveat: that acceptance does not settle misleading update-survival
advice, absence of required warning, wrong-target termination, or falsely successful Delete. No concrete distinct route
to those consequences was supplied by these assigned deletion patches or established in the checked retained host
responsibilities; no comprehensive correctness claim is made for adjacent migration/provisioning/UI implementations.

Returned proposal (claims-106, event 12): **covered**, suggested bucket **highest**.

Both the deliberate ending/loss of old terminals and the duplicate-agent residual after failed termination are explicit
specification exceptions. The proposed coverage is limited to those exact consequences and needs an independent
highest-bucket drop check.

Original stronger consequence: Updating a host ends agents and terminal tabs still running in the old terminal system
and loses their live contents/history. Failed legacy termination may leave an old agent alive and Restart may start
another beside it. Highest process/user-work loss remains the consequence for the drop check.

Unresolved premise or limitation: None for this exact accepted source behavior. Any misleading survival advice, missing
required warning, wrong-target termination or falsely successful Delete needs its own concrete route and is not covered
by this exception.

Proposed basis: Exact specification acceptance only for deliberate retirement and duplicate-on-failed-retirement
consequences. The independent checker must agree before omission; no parent decision inferred.

Existing parent disposition (event 12): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g169 record](records/review-through-00de0b6c/groups/g169.json). Record ID: `f1`.
