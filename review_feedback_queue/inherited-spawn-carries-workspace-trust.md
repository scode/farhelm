# Inherited spawn can trust an ordinary folder

Reviewed commit: 0f982643c59f553d153c29ac343798c903da3796

## TLDR

An agent launched with managed-checkout trust can spawn another session in an ordinary folder while keeping the trusted
launch flags.

## Details

The helm skips workspace-trust normalization for `farhelm spawn --inherit-agent`
(`crates/farhelm-helm/src/agent_requests.rs:1348–1372`). That path copies the parent's stored launch while accepting the
caller's required new working directory (`agent_requests.rs:1592–1641`), then dispatches both without recalculating
trust (`agent_requests.rs:1867–1928`). A trusted Codex, Muse or Pi session can therefore pass its trust override to a
folder outside a managed checkout. If the host allows agent actions without asking, no approval card intervenes.

Suggested bucket: highest, because repository settings or commands may run with trust the ordinary folder was not meant
to receive. Confidence: the static control flow was independently confirmed; no runtime exploit was attempted.
Provenance: trailing review of the stated commit, independently checked by a second reviewer. `SPEC.md`, “Managed
checkouts,” says ordinary selected directories get no trust override, while “Agent-spawned sessions” says inherited
spawn reuses the stored launch. Neither explicitly accepts carrying managed-checkout trust into a different ordinary
folder. A fix needs to reconcile inherited launches with the destination's checkout membership before dispatch.
