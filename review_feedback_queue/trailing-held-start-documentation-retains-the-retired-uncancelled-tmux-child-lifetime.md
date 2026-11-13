# held-start documentation retains the retired uncancelled-tmux child lifetime

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: held-start documentation retains the retired uncancelled-tmux child lifetime

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g108/l3`.

### g108/l3

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 21 (finding, open):

# describe the fake host's cancellation owner accurately

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513

## TLDR
The converted provenance test tells maintainers that cancelling its held start leaves the fake program alive until a release file or its iteration cap ends it. The host-start path now owns a kill-on-drop child and a bounded start timeout. This misstates the fixture's cancellation/cleanup contract; no runtime test failure or leaked process is established.

## Details
The assigned patch converts the fake tmux start into a fake terminal-host first stage and rewrites ReleaseHeldStart's explanation around spawn_agent_in_host. At core.rs:26335–26345 it still says the iteration cap is the fallback when the release cannot land/be acknowledged, and 'so a cancelled hold ends at the cap'; the Drop docstring at :26363–26373 likewise says the cap is what ends the shell on cancellation. The guard itself indeed only writes the release file, but the test's process owner does more than that.
The actual test directly joins spawn_agent_in_host with its observer (core.rs:26534 onward), with no detached task retaining the start future. spawn_agent_in_host directly awaits starting.finish() at core/host_launch.rs:573–575. hosts/spawn.rs:347–360 creates the first-stage Child with kill_on_drop(true); StartingHost holds that Child (:292–300), and finish moves it into a local first (:403–414), then directly awaits its wait under timeout (:419–451). Dropping the test's joined future therefore drops that Child and requests its kill. A start deadline likewise explicitly calls first.start_kill and reaps (:461 onward). The shell cap is still a conservative additional bound, and the release/ack protocol still supports the passing and caught-panic observation ordering; it is not the whole-start cancellation owner described by the changed comment. A kill request is not synchronous proof of process exit, so this finding does not claim the fake is always already reaped when Drop returns.
Attribution: the patch changes the ordering test from its explicit old sup.spawn_agent/fake-tmux future to sup.spawn_agent_in_host/fake-host, not merely a default terminal switch. The removed introductory explanation itself described the old run_bytes/Command::output path without kill_on_drop, explaining why that old fake needed an independent release/cap. Its new replacement drops that qualifier but carries the old cap-only cancellation conclusion into a path with different ownership. This is introduced inaccurate lifecycle prose, not an independent defect sought in retired tmux code.
Later check: at pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db core.rs:24247–24256 and :24273–24281 retain the same cancellation/cap assertions, and the host spawn's kill-on-drop ownership remains. Correct the prose to distinguish the release guard's file protocol, the joined host-start future's kill-on-drop/deadline ownership, and the shell's final self-bound. Retain the ack protocol's known observation-ordering rationale rather than removing it on the basis of this documentation finding.
Contract: root documentation rules require accurate lifecycle dependencies and test-harness assumptions, and the supplied checklist requires clear owned-process lifetimes. The source's own StartingHost contract (:280–291) explicitly says dropping it kills the first stage. Suggested bucket: other (documentation correctness). Confidence: confirmed source ownership and prose comparison; no execution/reproduction. proposed_drop: false. possible_cover: none among supplied neighbour records; g114/g115 concern a different runtime fixture and owner-exit oracle. Could-be-highest: no established or suspected security/loss-of-work route. Caveats: this does not refute c1's meaningful provenance observation boundary and does not add another failed-spec oracle finding; it only corrects the changed guard documentation's process-lifetime model. Cancellation is defense in depth here and is not directly exercised by this test, as the comment itself states. No missing source premise remains for this bounded documentation finding.

Additional attribution check completed during discovery: at the exclusive range base the same ordering test explicitly invokes the tmux path (core.rs:29169); at the immediate parent, tmux.rs:3627–3629 passes None as its create-command deadline to run_bytes_asking. The actual None branch at :3001 uses command.output() without setting kill_on_drop, while only the separate Some(deadline) branch sets it at :3005. Thus the old lifetime premise was applicable before the assigned explicit fixture conversion, not merely inherited inaccurate prose predating the whole range. No remaining attribution premise is open.

Returned proposal (claims-070, event 21): **retain-verified**, suggested bucket **other**.

The converted fixture's explanation carries a cap-only cancellation conclusion into a directly owned kill-on-drop host first stage. The guard itself only writes a release file; the enclosing start future additionally owns child termination and a deadline. Keep the release/ack observation-ordering rationale and distinguish a kill request from synchronous reaping.

Original stronger consequence: Inaccurate maintainer process-lifetime model only. No suspected security issue or loss of user-owned work.

Unresolved premise or limitation: None for the bounded ownership/prose mismatch. kill_on_drop requests termination; it does not prove the shell has synchronously exited or been reaped when Drop returns. Cancellation is not exercised by this test, and no runtime leak or failed provenance oracle is alleged.

Proposed basis: Retain as introduced lifecycle-documentation inaccuracy; keep the shell cap as an additional bound and preserve release/ack ordering context.

Existing parent disposition (event 21): verified. Parent explicitly accepted named retain-verified proposal from claims-070 after inspecting it. The converted fixture's explanation carries a cap-only cancellation conclusion into a directly owned kill-on-drop host first stage. The guard itself only writes a release file; the enclosing start future additionally owns child termination and a deadline. Keep the release/ack observation-ordering rationale and distinguish a kill request from synchronous reaping. Retain as introduced lifecycle-documentation inaccuracy; keep the shell cap as an additional bound and preserve release/ack ordering context. Checked source/results: ["647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513:crates/farhelm-supervisor/src/service/core.rs:26335-26373 says a cancelled hold ends at the cap and the cap ends the shell when release is missed. :26534 onward directly joins spawn_agent_in_host with its observer, without a detached future holding the child.","647f0227:service/core/host_launch.rs:573-575 directly awaits starting.finish(). hosts/spawn.rs:280-300 documents drop-kills-first-stage ownership; :347-360 sets kill_on_drop(true); :403-475 keeps the child locally under the start timeout and requests kill/reap after timeout. Cancellation drops that owner.","647f0227^:tmux.rs:2990-3015 uses command.output() without kill_on_drop in the None-deadline branch, and :3627-3629 selects None for create. The base core.rs:29160 onward explicitly exercises the fake tmux fixture, so the old lifetime explanation was applicable before conversion.","00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:core.rs:24247-24281 retains the same cap/cancellation assertions. Source contract comparison only; no cancellation or process-leak experiment."] Current coverage/results: ["Both specs searched for host first-stage cancellation, kill-on-drop and publication ownership. SPEC_impl.md:2200-2233 distinguishes owned first stage/spec delivery/retry; no accepted rule makes this cap-only prose accurate.","TODO.md Planned empty; BUGS.md unrelated. plans/queue searched for held-start/cancellation/cap and comment conversion. tmux-drop-from-docs.md:67-104 broadly rewrites tmux/xterm rationales and audits remnants, but this already rewritten host comment has no tmux text and no stated fix for its cancellation owner. Retain rather than infer that broad sweep covers it.","INDEX.md and active TRIAGE_OUTCOMES.md searched. held-start-test-lacks-prelaunch-fencing-witness.md was read in full: it concerns Restart prelaunch fencing at a different fixture, not this release guard's process-lifetime prose. No exact queue/ledger cover or useful extension. No archive read.","FILTER.md does not cover persistent inaccurate test-harness lifecycle documentation."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "Inaccurate maintainer process-lifetime model only. No suspected security issue or loss of user-owned work."; "None for the bounded ownership/prose mismatch. kill_on_drop requests termination; it does not prove the shell has synchronously exited or been reaped when Drop returns. Cancellation is not exercised by this test, and no runtime leak or failed provenance oracle is alleged.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g108 record](records/review-through-00de0b6c/groups/g108.json). Record ID: `l3`.

