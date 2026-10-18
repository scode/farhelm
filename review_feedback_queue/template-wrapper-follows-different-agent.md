# A template wrapper can follow a different agent without approval

Reviewed commit: a1c06723ba62cc29eae9c4ce28e7d7ce2f8acae2

## TLDR

An agent can select a saved template, switch to a different agent type, and run the template's wrapper command with that
new agent without a new wrapper approval.

## Details

An agent request applies a named template before its `--agent` edit (`crates/farhelm-helm/src/agent_launch.rs:164–180`).
The reconciliation keeps the template's wrapper prefix when the agent changes
(`crates/farhelm-proto/src/launcher.rs:565–590,742–744`), and the resulting launch uses those words
(`agent_launch.rs:210–219`). The approval check treats the prefix as agent supplied only when the CLI edit itself set it
(`agent_launch.rs:254–257`). By comparison, the GUI loads the newly selected agent type's remembered prefix. The landing
report in `plans/reports/agent-wrappers.report.md:168` records the unresolved mismatch.

Suggested bucket: highest, because a command saved for one agent can run with another across an approval boundary.
Confidence: the composition and approval paths are confirmed statically; a concrete security effect depends on what the
saved wrapper does. Provenance: trailing review of the stated commit and its interaction with the preceding wrapper
implementation. `SPEC.md` expects agent changes to use that type's remembered prefix and exempts a named template's
intended wrapper from separate approval. It does not accept pairing that wrapper with a different agent. Reconcile the
prefix when the agent type changes, or treat the changed combination as a new wrapper choice requiring approval.
