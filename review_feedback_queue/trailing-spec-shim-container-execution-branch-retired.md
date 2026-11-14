# Launch contract still describes the retired shim container branch

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: Maintainers following the rewritten launch contract are told that the shim has a
container execution branch. The old branch has been retired, so the contract describes the wrong execution boundary for
container sessions.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g009/l3`.

### g009/l3

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 11 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: Maintainers following the rewritten launch contract are
told that the shim has a container execution branch. The old branch has been retired, so the contract describes the
wrong execution boundary for container sessions. Details: assigned SPEC_impl.md:2753-2754 says 'The shim and its
container branch receive the same Bash vector, so expansion happens on the session's host or inside its container.' At
this revision launch.rs:220-225 retains LaunchSpec.container only to decode and refuse old queued specs; :1165-1172
writes the retired-model failure sentinel and returns whenever that marker is present. The remaining shim then executes
the ordinary agent_command at :1232, after normal checkout/cwd preparation. service/core.rs:14500 creates only
container: None. Source search for spec.container/container_exec/exec_container finds no alternative live shim branch.
The ordinary supervisor runs inside a first-class container environment; that fact does not make a container branch
exist in the launch shim. Suggested correction: describe one ordinary shim execution path on the session host, with
container sessions reaching that path through the supervisor installed inside their environment. Suggested bucket:
other, inaccurate authoritative implementation guidance. Could-be-highest: no established or suspected security or
loss-of-work route from the wording; no runtime misexecution is alleged. Confidence: confirmed source and contract
mismatch, not execution. proposed_drop: false. possible_cover: none in the supplied neighbour owner/summary inventory
and pertinent full records; no prohibited coverage sources searched. Attribution so far: git path history identifies
in-range 04d85f0c98a9b635709d00b056719a0baa7fd3fc as the legacy-container retirement, before the assigned 1ed482d
relocation retains this sentence. Exact parent/base comparison and pinned-tip persistence will be checked next. This
closes lead l3 on the documentation contradiction, preserving its original consequence. No runtime tests or
reproductions. Attribution and governing contract settled: in-range 04d85f0c98a9b635709d00b056719a0baa7fd3fc changes
LaunchSpec.container from ContainerLaunch to a decode-only JSON marker, adds the sentinel refusal, and deletes the
former container.prepare()/runtime.exec_command(&spec.argv) branch. Thus the claim was true at the excluded base and
became stale within this pinned range; predating the assigned 1ed482d rewrite does not make it pre-range. The assigned
patch relocates the unchanged stale sentence into its new Launch failure classification section. At the reviewed
revision SPEC_impl.md:8440-8453 expressly states that the new container supervisor runs inside its container and every
terminal/session uses ordinary host paths, while the supervisor-outside model is retired and only removal/ticket
handling remains. The sentence at :2753 contradicts that complete governing subsection. At pinned tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db the stale sentence remains at SPEC_impl.md:2655-2656, so no in-range repair
removes this documentation defect. This is source-confirmed implementation-guidance drift, not a claim of production
launch failure. No readily checkable premise remains unresolved; suggested bucket other and proposed_drop false are
unchanged.

Returned proposal (claims-004, event 11): **retain-verified**, suggested bucket **other**.

The authoritative launch explanation still names an execution branch retired within this range. The actual container
supervisor now uses the ordinary launch path inside its environment.

Original stronger consequence: Maintainers are directed to a retired container execution branch. No runtime
misexecution, security effect or loss of work was alleged.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain other, with retirement attributed to 04d85f0c rather than the later documentation relocation.
Source-only conclusion.

Existing parent disposition (event 11): verified. Parent explicitly accepted named retain-verified proposal from
claims-004 after inspecting it. The authoritative launch explanation still names an execution branch retired within this
range. The actual container supervisor now uses the ordinary launch path inside its environment. Retain other, with
retirement attributed to 04d85f0c rather than the later documentation relocation. Source-only conclusion. Checked
source/results: ["1ed482dde220452e424973faf38a5e1318986333:SPEC_impl.md:2753-2754 describes the shim and its container
branch receiving the same Bash vector. Current SPEC_impl.md:2655-2656 retains this sentence.","At the reviewed hash
launch.rs:220-225 preserves container only as a decode/refuse compatibility marker; :1165-1172 returns the retired-model
sentinel before preparation or exec; :1232 executes ordinary agent_command.","In-range
04d85f0c98a9b635709d00b056719a0baa7fd3fc retires legacy containerized sessions and changes launch.rs. Reviewed
SPEC_impl.md:8440-8453 explicitly says the current supervisor runs inside its container and uses ordinary host paths,
with the supervisor-outside model retired.","This is stale execution-boundary guidance, not evidence of a production
launch failure or an isolation bypass."] Current coverage/results: ["Current SPEC.md/SPEC_impl.md searched for
shim/container branch and rootless container execution. The correct current-container subsection supplies contradictory
context, not acceptance of a nonexistent shim branch.","TODO Planned empty; BUGS.md unrelated; active TRIAGE_OUTCOMES.md
and INDEX.md have no matching shim-branch item. Existing launch-input-database-duplication-rationale-overstated concerns
missing database prompt content, not the execution boundary.","No unlanded matching plan found; broad tmux-doc cleanup
is already landed per plans/REPORTS.md:32. FILTER.md has no matching boundary for persistent incorrect architecture
guidance."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "Maintainers are
directed to a retired container execution branch. No runtime misexecution, security effect or loss of work was
alleged."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g009 record](records/review-through-00de0b6c/groups/g009.json). Record ID: `l3`.
