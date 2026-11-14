# Cleanup-failure risk predates the removed GUI warning

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Cleanup-failure risk predates the removed GUI warning

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g033/r1`.

### g033/r1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 11 (finding, open):

# Cleanup-failure risk predates the removed GUI warning

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e

## TLDR

Failed update cleanup can leave an old agent alive while presenting its session as ended; Restart can create a second
agent beside it. The removed migration banner already received no old terminal to describe. This consequence and both
proposed exclusion grounds remain retained for independent checking.

## Details

Discovery-input synthesis revision of r1. New neighbor g032/l5 describes a distinct confirmed route to loss of
user-owned processes: a Running Restart question stays open, later renders Interrupted with "nothing left to stop", then
its answer authorizes a Restart that reaps surviving old-agent work. That record establishes a highest-impact
misleading-confirmation route in status.rs/session_view.rs at dbfc69751215dbf15e1d31d5bed73c3754c4ebc4. It is not
refuted by accepted imperfect startup cleanup and is not proposed for dropping here. My r1 exclusion applies only to
attributing concealment to the assigned migration-banner deletion and the separately accepted
cleanup-failure/duplicate-agent behavior. It does not exclude or lower g032/l5's destructive-consent consequence. No
merge of those findings is proposed merely because both start with surviving old processes.

Source attribution remains checked: parent migration.rs:99-117 requires agent_terminal == Tmux and !status.has_ended(),
or a Tmux-kind tab. Parent proto lib.rs:925-935 counts Interrupted as ended. Parent service/status.rs:379-388 derives
agent_terminal from entry.terminal and lists only host tabs; :431-445's true new-terminal switch does not change the
absent evidence. Parent core.rs:6675-6679 already supplies no found terminal for Tmux rows. Current core.rs:6675-6680
retains that rule, :6808-6822 proposes TerminalEnded, :7110-7111 constructs absent-terminal entries with committed
outcomes, :7148/:7247 supplies adopted host tabs, and core/host_tabs.rs:270-275 lists only those runtimes. Thus
retaining the deleted module/fields would not disclose the survivor. Original consequence is preserved: the UI's
ended-state claim may conceal living work and Restart can introduce concurrent agents acting on that work, but this
assigned deletion is not established as its cause.

Boundary checks remain: protocol 77 and exact-version admission (proto lib.rs:255, io.rs:587-660; helm
client.rs:1755-1759) exclude older serving supervisors. Startup cleanup precedes reload (supervisor core.rs:6142-6185),
sweep precedes committed conversion (:7045-7050), notices and old-warning resolution follow (:7077-7090;
core/old_terminals.rs:513-548). These do not establish process death after failure.

Exact specification acceptance for independent checking: SPEC.md, Terminal experience, :3011-3015: "A failure to end the
server or to stop what ran there is logged and never stops the supervisor from starting; those sessions are treated as
ended all the same, accepting that a Restart could then start a second agent beside one still running." SPEC_impl.md,
Terminal substrate: private tmux server, :2827-2831: "A failure anywhere is logged and never fatal (the maintainer chose
to start anyway and treat old sessions as ended, accepting that a Restart could then start a second agent beside one
still running)". Proposed_drop: source refutes attribution to the assigned banner deletion; independently, these
specifications accept cleanup failure and a second agent. Neither proposed exclusion is accepted by the parent. Keep r1
as a finding pending independent decision.

Suggested bucket remains proposed high for incorrect-ended-state/duplicate-agent behavior if judged a defect, with
could-be-highest: yes. Preserve potential work loss from concurrent agents, which is not established on r1's own
duplicate-agent route. The newly supplied g032/l5 route does establish a separate surviving-process loss consequence and
remains highest in that owner's record; no consequence lowering is assumed or proposed for it. Confidence: confirmed
source refutation of banner causation and explicit contract acceptance of duplicate-agent behavior; no execution.
g004/f1 remains distinct: general survival assurance followed by intended successful termination at
78e9d68a95119db8452ee139efde13d38546f648, rather than failed cleanup and surviving agents. Possible_cover: related
caller-supplied g004/f1 and g032/l5 only, not equivalent coverage. No coverage sources searched. No unresolved source
premise for this disposition; independent exclusion/consequence decisions remain pending.

Range-attribution check during discovery: git log over exclusive 6e423e30068b62c6cfa276c6d720ee35bb70387a through
f560058da19e02716e6f943883a9508b447aff4e for migration.rs and service/core/old_terminals.rs identifies
dbfc69751215dbf15e1d31d5bed73c3754c4ebc4 as the earlier old-terminal-ending change and
f560058da19e02716e6f943883a9508b447aff4e as the later GUI removal. Therefore this risk predates the assigned GUI
deletion, but does not predate the full pinned review range. No range-wide pre-existence exclusion is claimed. The
earlier startup/confirmation change remains covered by its owners, including supplied g032/l5; independent checks of r1
and its proposed exclusions remain pending.

Returned proposal (claims-019, event 11): **covered**, suggested bucket **high**.

The full-range cleanup-failure behavior is real but expressly accepted by both current specifications. Separately,
removing the GUI migration banner did not newly conceal these survivors because its prior predicate and projection
already excluded them. Neither conclusion excludes the distinct misleading confirmation in g032/l5. This proposed
omission and consequence classification require independent checking.

Original stronger consequence: Failed cleanup can conceal living work behind an ended-state claim; Restart can create
concurrent agents acting on the same user-owned work, potentially damaging it. Earlier history proposed highest, later
high; neither lowering nor omission has been accepted by the parent. g032/l5 separately establishes a process-loss
confirmation route and remains distinct.

Unresolved premise or limitation: A shipped route from duplicate agents to actual user-work loss is unestablished. That
missing stronger consequence does not itself justify dropping the established behavior; exact specification acceptance
is the proposed omission basis.

Proposed basis: Independent check: (1) exact specification acceptance of ended state and a second agent after failed
cleanup; (2) narrowly refuted attribution to the GUI banner deletion; (3) proposed high rather than highest because
actual duplicate-agent work loss is not established. Do not erase in-range introduction at dbfc6975 or collapse the
distinct supplied g004/f1 update-survival promise or g032/l5 confirmation.

Existing parent disposition (event 11): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: exact-spec-accepted-old-cleanup-ended-state-and-second-agent. Full019
andparentactualSPEC2985-3020/impl2770-2830 explicitlyacceptfailedoldcleanup,endedstateandRestartsecondagent;
exacttrigger/consequence/scope. Inrangeintroduceddbfc,notoutofrangejustpredatesf560UIbannerremoval.
Oldbannerpredicatealreadyexcludedterminal-lessInterruptedsurvivor,protocolfenceonlyoldernativepeerNOTcurrentcleanupfailures.
Originalpossibleduplicateagentworkdamageremainsunestablishedbutkeptforgate,
basisdropisexactspecacceptancenotmissingstrongerharmalone.
Distinct032l5falsewarningand004f1updatesurvivalpromisenotcoveredbythisacceptance.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g033 record](records/review-through-00de0b6c/groups/g033.json). Record ID: `r1`.
