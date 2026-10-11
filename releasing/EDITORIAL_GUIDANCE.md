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
- Open an entry with what the user sees or can do, not with an umbrella word like "changes" or "improvements" that only
  makes sense once the list after it has been read. The v0.20.0 draft's "Changes that happen on a host by themselves now
  show up almost at once…" left the reader confused until the end; it became "The app now shows almost at once, instead
  of up to three seconds later, when an agent's status changes, an agent exits, …". (Prompted by the v0.20.0 curation,
  2026-09-30.)
- Do not tell users that a release cannot be downgraded from, under Breaking or anywhere else: most releases upgrade a
  database an older build then refuses to open, so it is normal and goes unsaid. The v0.25.0 draft opened with a
  Breaking entry modeled on v0.23.0's "After upgrading, you cannot go back to an earlier release …", and two of its
  fragments ended with the same warning; all three were dropped. (Stated by the maintainer during the v0.25.0 curation,
  2026-10-07.)
- Keep each entry to what the user sees or has to do, in one or two short sentences. Leave out how it works, rare edge
  cases and the conditions they need; keep a caveat only when a user would otherwise be caught out by it. The v0.26.0
  draft's Codex entry ran six sentences, through when Farhelm reads a saved conversation, what happens if its file was
  deleted and what a passing read error does; it became one sentence saying which sessions to relaunch and why. The
  whole draft got the same pass. (Stated by the maintainer during the v0.26.0 curation, 2026-10-10.)
