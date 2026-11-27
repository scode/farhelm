# SPEC_impl's claim that an opt-in real-agent test pins Codex Stop-time transcript availability after /clear is stale after the rotation category replaced the audits

Reviewed commit: 197fcf86ad658224b2de09ebf5fe4a3a1cc6ae55

## TLDR

The implementation spec tells maintainers that a real-agent test still checks one Codex behavior that Farhelm's handling
of a pending `/clear` relies on: at Stop, Codex's callback already names a transcript that exists on disk. The commit
that added the conversation rotation drift category deleted that test, and the new category does not check it. If a
Codex upgrade starts calling Stop before the file exists, no real-agent test notices, while the spec says one would.

## Details

SPEC_impl.md:4476-4479 ("Codex 0.162.0 was run for two actual turns around `/clear`: both Stop callbacks named existing
root transcripts, clear changed both runtime and path, and the old header remained intact. An opt-in real-agent test
pins callback-time availability; deterministic fixtures cover a clear whose first callback precedes the file.") referred
to `real_codex_stop_names_the_persisted_transcript_after_clear` in crates/farhelm/tests/e2e/real_agent_capture.rs. That
test's probe hook read the transcript header inside the callback. Commit 197fcf86 deleted it (see
`git show 197fcf86 -- crates/farhelm/tests/e2e/real_agent_capture.rs`) and changed no SPEC file. The replacement,
crates/farhelm/tests/e2e/real_agent_capture/drift/conversation_rotation.rs, gives Codex `/clear`
`before_prompt: Unspecified` (rotation_spec, the Codex arm) and passes once the row holds any resumable capture of a
different conversation after the next prompt (`wait_for_capture`). Supervisor admission happens when the dropped report
file is applied, after the callback has returned, so a resumable row shows the file existed at admission, not at
callback time. Nothing checks that the old header stays intact either. The tip's own comment in
crates/farhelm/tests/e2e/real_agent_capture.rs:174-184 lists what the category took over from the standalone audits. It
names Claude's reports, Codex's `/new` lag and the `source` fields, and says "Codex's `Stop` confirming a pending
`/clear` is covered by the supervisor's fixtures". So the code deliberately dropped the live callback-time check, and
only SPEC_impl still claims one exists. docs/harness-drift-tests.md likewise leaves the pending path to the supervisor's
own tests. To verify: grep the tree for `callback-time` (only SPEC_impl.md:4478) and for the deleted test name (only
plans/). Fix: either restore a real-agent check of the Stop callback's transcript, or rewrite the SPEC_impl sentence to
say that only deterministic fixtures cover it now and what the rotation category checks live instead.

Reviewer revision notes: Revision: adds the tip's own code comment as further evidence; the conclusion is unchanged.

### Triage notes

- Recorded bucket: `other`.
- Suggested bucket (reviewer): other. A spec/test-coverage drift on an opt-in vendor-drift audit. It produces no
  user-facing failure unless Codex also changes its Stop timing.
- Confidence: confirmed (source: the deletion in 197fcf86, the unchanged SPEC_impl sentence, the rotation module's Codex
  /clear descriptor, and the tip comment at real_agent_capture.rs:174-184).
- possible_cover: g36 (the real_agent_capture.rs side of the same commit) may report the deletion itself.
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g37/f1 (finding), group g37 "Conversation rotation drift: the new module", findings log line 6.
