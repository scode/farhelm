# An incomplete manual hook setup is reported as agent drift

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

The installed-agent check can report agent drift when its manual conversation hook is configured for the wrong event.

## Details

crates/farhelm/tests/e2e/real_agent_capture/drift.rs:192–225 accepts any matching command string anywhere in the JSON,
regardless of event or command type, and :229–244 only selects the file layout. An entry with the right command under no
conversation-reporting event passes this check. The tracked case then submits at :656–657 and expects an accepted
resumable report at :659. With no reporting event configured, a working CLI can reply while the test times out waiting
for a report. The documented Antigravity SessionStart/Stop entries are in
website/src/content/docs/docs/agents/antigravity.md:29–52, the Grok entries in
website/src/content/docs/docs/agents/grok.md:17–50, and the prerequisite-versus-failure contract in
docs/harness-drift-tests.md:27–42. Validate commands in the documented vendor event structures before spending the turn,
and report a missing reporting hook as COULD NOT RUN. No later range commit changes drift.rs.

Confidence: confirmed by inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `driftRunner/l2`, `agUI/l6`.
