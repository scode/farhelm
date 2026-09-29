# Editorial guidance for release notes

The maintainer's accumulated opinions on how changelog entries should read, collected from curation feedback that
generalized beyond the entry it was given on. An agent drafting a fragment at PR time or a release section at curation
time reads this first and follows it, so the same correction does not have to be made twice. The format rules themselves
(headings, categories, bullets) live in `releasing/AGENTS.md`; this file is about wording.

Each rule records the guidance and, when it helps, the example that prompted it. A rule is added only when the
maintainer stated it, or when the agent proposed a generalization and the maintainer said yes; an inferred rule never
goes in unasked. Rules are appended as they arrive; one that turns out to be wrong is removed or rewritten, not argued
with in place.

- Name things the way a user meets them, not the way the code does. "Launch composer" is an internal name for the dialog
  that opens when you start a new session; a user has never seen those words. Write "the session launcher", which the
  maintainer judged to have a reasonable chance of being understood, and reserve internal names for the source. The test
  is whether the name appears somewhere the user operates, a screen, a label, or a command; if it only appears in the
  code, say what the thing does for them or which screen it is instead. (Prompted by the v0.12.0 curation, 2026-09-22.)
- Write the first draft of every entry plain-spoken and free of jargon; do not wait to be asked. A fragment is a draft
  written at PR time, often in the change author's vocabulary, so reword it for someone who runs Farhelm rather than
  copying it: "the top of an open session" rather than "the session header", "adding or updating a remote host" rather
  than "provisioning". (Prompted by the v0.19.0 curation, 2026-09-29.)
- When an entry names a tool or a part of the system, say why it matters to someone running Farhelm, not only what
  changed about it. The v0.19.0 entry about `farhelm helm setup` picking a `tmux` from a relative `PATH` entry only made
  sense once it opened with "Farhelm runs every session inside tmux, and `farhelm helm setup` records which `tmux`
  program to use for all of them." (Prompted by the v0.19.0 curation, 2026-09-29.)
- Something users can newly do or see belongs under Added, even when its PR was typed `fix`. The v0.19.0 terminal-link
  hover, which shows where a link really goes, came from a `fix:` PR and a `kind: fixed` fragment, and moved to Added.
  (Prompted by the v0.19.0 curation, 2026-09-29.)
