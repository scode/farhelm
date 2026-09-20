# Foreground conversation ownership across harnesses

NOTE: Historical assessment recorded on 2026-09-20 against the stack containing PR #801, at commit
`650c16e3defdfeb481a3beb70a711b7803a63323`. This is source-level evidence, not an end-to-end reproduction of every
vendor's subagent behavior. The entry is not maintained, and it does not specify an implemented cross-harness fix.

## Question and conclusion

Can subagents, whether native or shelled out, overwrite the restart target of the conversation visible in a Farhelm
terminal? [PR #801](https://github.com/scode/farhelm/pull/801), `fix: resume the foreground codex conversation`, closes
that reporting hole for Codex. The underlying ownership problem is not specific to Codex: Claude, Goose, Pi, and OMP
still lack an equivalent foreground-ownership check at report admission.

The required invariant is that a child conversation cannot replace or withdraw its parent's restart target. A valid
inherited reporting credential establishes which Farhelm session a report addresses, not which vendor conversation
is in the foreground. Whether each vendor automatically loads the reporter in its children is a separate question,
and was not established by this investigation.

## What the Codex fix checks

PR #801 adds two independent checks. Process attribution rejects a nested Codex executable reporting through its
parent's inherited credential. Exact transcript verification requires root-session metadata matching the reported
runtime identity; process ancestry alone cannot distinguish native child threads sharing the foreground process.
The locator keeps runtime identity separate from the persistent thread ID used for resume.

Those checks are inside the `kind == AgentKind::Codex` branch of `Supervisor::report_conversation`
(`crates/farhelm-supervisor/src/service/core.rs:13047-13131`). Process attribution is in
`crates/farhelm-supervisor/src/procs.rs:177-220`; root metadata validation is in
`crates/farhelm-supervisor/src/agent_kind/codex.rs:146-207`.

Legitimate foreground clear/new transitions can replace the target. An attributed clear whose transcript is not yet
persisted withdraws the old target while waiting for that exact new record. Unverified historical bare IDs remain
stored but no longer qualify for exact Codex resume. The PR description records a reproduction of inherited reporting
with Codex 0.155.1, while explicitly leaving the historical incident's exact emitter unproven.

## Remaining reporting paths

- **Claude:** a plausible plain conversation ID and the inherited Farhelm session credential are sufficient for report
  admission. There is no foreground-process or native-child check in that path. The scan fallback's root-only traversal
  does not protect accepted reports, which take precedence over scans.
- **Goose:** the MCP reporter reads `AGENT_SESSION_ID` and the inherited Farhelm credential at startup. There is no check
  that the reported Goose conversation belongs to the foreground session. The reporter declaration is persisted with
  the conversation and reused on resume (`crates/farhelm/src/main.rs:990-1031`).
- **Pi:** admission checks locator format, and resume verifies the exact saved file against the reported session ID.
  That proves consistency between a file and an ID, not ownership by the foreground agent. A child that loads the
  reporter can replace the target; a fileless report can withdraw the parent's resume offer.
- **OMP:** the same ownership gap remains. The extension serializes reports and cancels stale IDs within one extension
  instance, but parent and child instances do not share that queue. File checks do not establish foreground ownership.
  A fileless child report can withdraw the parent's offer (`crates/farhelm-supervisor/assets/omp-conversation-v1.ts`).
- **Muse and OpenCode:** these structured launch choices currently map to `Generic`, which rejects conversation reports.
  This particular report-overwrite path therefore does not apply.

The common path is: a child inherits the parent's Farhelm credential, runs a reporter, authenticates as the parent's
Farhelm session, passes the non-Codex ID/locator shape check, and replaces the saved target. The credential is installed
in the agent's environment by `crates/farhelm-supervisor/src/launch.rs:903-908`. Authentication is in
`crates/farhelm-supervisor/src/service/handlers.rs:3491-3592`. `accepts_reported_conversation` in
`crates/farhelm-supervisor/src/agent_kind/mod.rs:2498-2509` checks shape and durable agent kind, not reporter ownership.
The non-Codex path at `core.rs:13169-13172` calls `record_reported_conversation`, whose SQL replaces the identity under
an ID-and-generation predicate (`crates/farhelm-supervisor/src/store.rs:4789-4812`). The generation fence does not
separate a parent from a child running during the same launch.

Mixed-harness delegation also needs assessment. Claude and Goose accept untagged plain IDs; that format does not prove
which vendor emitted the report. Pi and OMP's distinct locator tags prevent accepting each other's locator format, but
do not distinguish parent and child instances of the same vendor.

## Evidence limits

The missing admission guard is confirmed in source. Automatic triggering by every vendor's native or shelled-out child
is not. Claude's injected `--settings` argument and Pi/OMP's `-e` argument are not inherited merely because environment
variables are inherited. A plain shell-out may never load the reporter. That limits exposure but does not establish
that child reports cannot overwrite the parent.

The investigation traced the common supervisor path and independently inspected the Claude and extension-based
reporters. It also extracted the repository's replacement SQL and executed it against an isolated in-memory SQLite
row: a same-generation child report changed `parent-conversation` to `child-conversation`. That exercises persistence
semantics only; it is not a vendor reproduction or an end-to-end supervisor test. No implementation changed.

## What to assess next

Assess the evidence each reporting integration can provide for both process ownership and root-conversation ownership.
Reproduce native-child and shell-child reporting, including mixed-harness delegation and persisted versus ephemeral
children. Decide which supervisor checks and reporter changes are needed to reject child reports without breaking
legitimate foreground clear, new, switch, fork, and resume transitions. Include attempts to withdraw the parent's
resume offer, not only attempts to replace it with another saved transcript.

Do not make capture write-once: that would prevent overwrites by breaking legitimate foreground transitions. Likewise,
credential inheritance, locator syntax, saved-file validity, and a queue local to one reporter are not substitutes for
foreground ownership. The next task is assessment and a concrete fix plan, not an assumption that the Codex-specific
process check can be copied unchanged into every integration.
