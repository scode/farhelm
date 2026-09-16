# Goose

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
