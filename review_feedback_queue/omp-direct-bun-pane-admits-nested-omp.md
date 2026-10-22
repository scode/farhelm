# A direct Bun OMP launch with an unrecognized entry file can credit a nested OMP's conversation

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

Suppose an OMP session is started as a custom command that runs Bun on OMP's entry file directly, and Farhelm cannot
match that entry file to OMP (it is named relative to the working directory on a Mac, or comes from an install layout
Farhelm does not know). Then a second OMP started inside the session that loads Farhelm's reporter can have its
conversation recorded as the session's, and Restart resumes that conversation instead of the user's. The trigger is very
unlikely.

## Details

Found by the 2026-10-22 prioritized persistent review, group `206-harness-non-first-class-2of2` (finding f2).

`classify_omp_launch` (`crates/farhelm-supervisor/src/agent_kind/omp.rs:53-70`) puts every launch whose effective
program is `bun` or `bunx` under `OmpLaunchProgram::Bun`. That includes a direct `bun <entry> {farhelm_args}` command
launch, which gets the extension when it declares OMP (SPEC.md around 2283: "so does a command launch that declares OMP,
where its `{farhelm_args}` stands"). In such a launch the pane process is the session's OMP runtime.

`omp_corridor` (`crates/farhelm-supervisor/src/procs/omp.rs:271-389`) counts a Bun link as a runtime only when
`is_omp_runtime_link` (`omp.rs:424-436`) matches its entry: the raw spelling ends with
`node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js` or `packages/coding-agent/src/cli.ts`, or `resolved_omp_entry`
canonicalizes it to one. A relative spelling resolves only against the recorded cwd, and on macOS `process_cwd` always
returns `None` (`procs.rs:1574-1576`). So a relative entry on macOS, or an entry from any other layout on either OS (a
copied bundle, a source checkout's `dist/cli.js`), leaves the session runtime uncounted. The pane rule then exempts it:
`if pane_index != emitter_index && (is_bun_image || is_node_image) && (pane.argv.is_none() || matches!(program, OmpLaunchProgram::Omp))`
(`omp.rs:330-348`, the check itself at 341) refuses only an unreadable pane or an installed-`omp` launch. A readable Bun
pane under `OmpLaunchProgram::Bun` is accepted by position, and the intermediary loop skips the last link.

Triggering chain: the reporter, then a nested OMP runtime whose entry is recognized (the user's installed `omp`, say)
and that loads Farhelm's reporter, then the session's Bun runtime at the pane (readable argv, unrecognized entry). With
no shell link between them (`bash -c '<single command>'` execs, and an OMP-spawned child has no shell), the nested
runtime is the only emitter. `omp_runtime_tui_grammar` passes for an interactive child, launch provenance passes, and
`report_omp_conversation` (`crates/farhelm-supervisor/src/service/core/vendor/omp.rs:77-260`) admits the nested
conversation with no ordering check. The session's own reports always refuse in this configuration ("the hook has no
attributable OMP runtime"), so the first Resume offer the user ever sees is for the wrong conversation. The nested
report also passes the OMP file-header check, because its file is a real OMP session file.

SPEC_impl.md "The OMP ownership proof" (around 3110-3116) supports treating this as a defect: "Nested or additional
session-hosting runtimes of any kind, unclassified intermediaries, ... unknown wrappers ... all refuse, for every launch
program. Under a `bun x` or npm launch, a readable pane keeps its positional exemption". A direct `bun <entry>` launch
is not a `bun x` launch, yet the code applies the exemption to it because `classify_omp_launch` folds both into `Bun`.
The asset itself anticipates the nested-reporter premise: "A separately launched interactive OMP child passes this gate
... so the supervisor's foreground-process admission (not this file) is what refuses it"
(`crates/farhelm-supervisor/assets/omp-conversation-v1.ts:31-34`).

How to verify: a unit test beside the existing Bun-launch corridor tests (`procs.rs` around 3349-3389) with the chain
[hook reporter, `bun /home/u/.bun/install/global/node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js` (nested,
recognized), pane `bun ./vendor/omp/cli.js` with `cwd: None`] and program `Bun`. `omp_corridor` returns the nested pid
today.

Fix sketch: under every launch program, refuse when the pane is a Bun or Node process that is not the emitter and whose
argv looks like an OMP runtime rather than a launcher (for Bun, argv[0] basename `bun` with argv[1] not the `x`
subcommand and not a `bunx` form). Readable `bun x`, `bunx` and npm launcher panes keep the positional acceptance the
triage decision chose for npm's rewritten titles. Alternatively, classify direct `bun <entry>` launches separately from
`bun x` and apply the installed-`omp` pane rule to them.

Suggested bucket: highest by consequence (resuming the wrong conversation; SPEC.md "First-class harnesses" keeps that in
scope for non-first-class harnesses), with negligible likelihood, like the OMP precedents.

Confidence: likely. The code path is confirmed by reading. Unverified premises: a user declares OMP on a command launch
that runs `bun <entry>` with an entry spelling Farhelm cannot match; and a nested interactive OMP inside that session
loads Farhelm's reporter (which needs Farhelm's `-e` path passed on, for example by an OMP feature that relaunches with
the parent's extensions; unverified) with no shell link between it and the pane.

Reviewer's proposed drop, kept for triage: possibly covered by TRIAGE_OUTCOMES.md `omp-bun-pane-proof.md`, "Later
decision": "replaces the top-process launcher match with unreadable-arguments refusal for `bun x` and npm launches: a
readable top process is accepted there without a launcher match, including npm's rewritten process title." That decision
accepted readable top processes for `bun x` and npm launcher panes. Its assessment named the direct `bun <entry>` launch
as the exposed shape ("Only a direct `bun <entry> …` command launch is exposed") but treated only the unreadable or
over-budget pane. Whether the acceptance extends to a readable direct-Bun runtime pane whose entry cannot be matched is
for triage to decide; recording did not treat it as a cover because the decision and SPEC_impl.md both scope the
exemption to `bun x` and npm launches.

Possible cover: TRIAGE_OUTCOMES.md `omp-bun-pane-proof.md` (Later decision, quoted above) and
`omp-corridor-uncounted-pane-runtime.md`. FILTER.md "Rare edge cases in harnesses without first-class support" does not
apply, because it excludes "resuming the wrong conversation".
