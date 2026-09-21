# Goose

## How Farhelm identifies the foreground conversation

NOTE: this used to be report-only — Farhelm took the helper's word for which conversation was in front. It no longer
does. When the helper reports a conversation, Farhelm now reads that exact session's metadata from Goose's own store and
accepts the report only if the row proves a foreground root. That is a deliberate contract change, and the rest of this
section is the new contract.

Why the read exists: Goose hands the MCP reporter to its native children, and parent and child share one process. A
credential only proves the report came from inside the session's processes, and a PID only proves a process exists —
neither can tell the parent's conversation from a delegated child's. The store row can: Goose records each session's
role and parentage, so the row distinguishes a root conversation from a delegated one where process evidence cannot.

What is read: the reported session's id, role, and parent link, plus the minimum compatibility evidence the check needs
(the sessions table's pinned columns and the store's schema version). Prompts, messages, and other sessions are never
read; nothing is enumerated, and there is no newest-row or different-home fallback. The lookup is addressed by the
reported id — the report names the conversation, the read validates that one record.

Boundaries: the store opens read-only, and read-only is enforced several layers deep rather than trusted to one flag.
Nothing is created (an absent store stays absent — no database, no WAL, no directories), nothing migrates, nothing
checkpoints, and no pragma is ever written. The store path resolves from the running Goose process's own environment —
an absolute `GOOSE_PATH_ROOT` wins, then an absolute `XDG_DATA_HOME`, then the runtime's `HOME` — on Linux and macOS
alike, so custom roots work through exact paths rather than discovery. Farhelm never guesses a home, never the
supervisor's own.

When the metadata is unavailable, busy, malformed, or from an unsupported schema — or the row is anything but a `user`
session with no parent — the report is refused and nothing changes: no new capture, no resumed target, and the parent's
existing target is left exactly alone. A rejected child never erases it. One startup report lost to a busy store or a
saturated reader stays lost until that session's next report, because the reporter fires once per process start; that is
fail-closed, not a retry bug.

What this does not cover: scheduled sessions stay runnable without capture (policy, not oversight). A shell-wrapped
Goose launch is not injected — Farhelm will not append vendor flags to wrapper argv it cannot verify — so it stays
without capture unless you declare the reporter yourself; a manually declared reporter under a transparent wrapper still
proves rather than silently failing. Docker and Flatpak Goose are unsupported: host-only evidence cannot prove which
store inside the container is authoritative. The supported shape is the native CLI at a pinned source revision (verified
against Goose 1.50.1's wire behavior); anything else fails closed. Stale parent links from export/import flows the
pinned revision does not exhibit reject conservatively, as does a store path that cannot be represented exactly.

## Resuming outside Farhelm still needs Farhelm installed

Farhelm starts a small helper alongside Goose to learn which conversation you are in. When you type `/new` in Goose to
start a new conversation, the helper tells Farhelm to resume that conversation instead of the old one. Goose treats the
helper as an extension and saves its startup command with the conversation, so it starts again on later resumes.

That means reopening the conversation directly in Goose, outside Farhelm, still requires the `farhelm` command to be
available in your shell (`PATH`). If you uninstall Farhelm or move the conversation to a machine without it, Goose can
report an extension-startup error. Resuming through Farhelm supplies the helper's location automatically.

The saved command contains no Farhelm credentials or machine-specific executable paths. Outside Farhelm, the helper
starts but does nothing: it sends no reports and gives the agent no tools or instructions. No global Goose settings are
changed.

Disabling [Farhelm's conversation reporting](../agent-hook-injection.md#turning-it-off) prevents the helper from being
added to new conversations but does not remove its startup command from conversations Goose has already saved.

The identification contract above changes none of this: the saved command stays credential-free and machine-neutral, and
outside Farhelm the helper still starts but does nothing — the store read happens on Farhelm's side, only for reports
Farhelm receives.
