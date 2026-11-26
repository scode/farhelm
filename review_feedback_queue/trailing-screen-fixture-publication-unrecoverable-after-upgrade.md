# An unfinished screen-fixture publication cannot be recovered once the agent's version changes

Reviewed commit: 062c87e54ec7882f2cbf008864a63ffbf3ff5369

## TLDR

If a screen capture is killed mid-publication and the agent is upgraded before the maintainer reruns the capture, the
capture tool never repairs the half-written version directory. Upgrades are routine here: the repository's own drift
runner upgrades every selected agent by default, and Claude Code updates itself at startup. The fixture tests then keep
failing with advice to 'Run the capture tool again', which cannot help, so the maintainer has to roll back the journal
by hand.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t06a-drift-fixtures` `f1` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Reviewer's suggested bucket kept. The commit's docs paragraph ("rerun capture for the same installed version") partially
acknowledges the constraint; it is prose, not a spec acceptance. No coverage source covers it.

### Reviewer record

Location: `scripts/capture-agent-screens.py:929`

Revision in synthesis turn: the upgrade premise is now established from repository sources rather than general
knowledge. docs/harness-drift-tests.md line 5 says 'runner first upgrades each selected agent'; line 87 says 'Claude,
the Muse launcher and OMP can update themselves during startup'; and scripts/harness_upgrades.py holds the update
recipes for claude and codex, which are the two harnesses scripts/capture-agent-screens.py drives (HARNESSES at
:781-782). The rest is unchanged. (All line numbers at 062c87e54e.) capture_harness
(scripts/capture-agent-screens.py:927-935) recovers only FIXTURES_ROOT/<name>/<harness_version(...)>, the version
currently installed, and publish_fixture_pairs (:866-873) does the same. Nothing else calls recover_fixture_publication,
and the script has no option to name a version directory. A journal left by a process exit or SIGKILL (the case
test_next_capture_recovers_a_process_that_exited_between_pair_renames covers) sits in the old version directory with
manifest.json present and the pair half-swapped. Once the installed version differs, the old directory is never visited
again. load_version_fixtures (crates/farhelm-supervisor/src/agent_kind/screen_fixtures.rs:121-127) then panics on every
fixture run with 'fixture publication is unfinished in <dir>. Run the capture tool again to recover it'.
docs/agent-screen-fixtures.md (added lines 45-48) says 'rerun capture for the same installed version', but the panic
does not, and after an upgrade that rerun needs a downgrade of the vendor CLI. No later commit in the range changes the
capture script or the loader. The route is established from source; no execution was observed. Fix: have capture (or a
'--recover' mode) scan every version directory under FIXTURES_ROOT/<harness> for a journal and run
recover_fixture_publication under the version lock before capturing. Alternatively, at least make the Rust panic and the
docs say how to recover a version that is no longer installed. A unit test can create a journal in version A, run
capture_harness with harness_version mocked to B, and assert that A is restored.

Suggested bucket: other. The harm is confined to maintainer test tooling and fails loudly; no user-facing product
behavior is affected.

Confidence: confirmed (source-established route and upgrade premise)

proposed_drop: none

possible_cover: docs/agent-screen-fixtures.md, the paragraph added by this commit ('rerun capture for the same installed
version') partially acknowledges the constraint.
