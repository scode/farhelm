# Editorial guidance for release notes

The maintainer's accumulated opinions on how changelog entries should read, collected from curation feedback that
generalized beyond the entry it was given on. An agent drafting a fragment at PR time or a release section at curation
time reads this first and follows it, so the same correction does not have to be made twice. The format rules themselves
(headings, categories, bullets) live in `releasing/AGENTS.md`; this file is about wording.

Each rule records the guidance and, when it helps, the example that prompted it. Rules are appended as they arrive; one
that turns out to be wrong is removed or rewritten, not argued with in place.

- Name things the way a user meets them, not the way the code does. "Launch composer" is an internal name for the dialog
  that opens when you start a new session; a user has never seen those words. Write "the session launcher", which the
  maintainer judged to have a reasonable chance of being understood, and reserve internal names for the source. The test
  is whether the name appears somewhere the user operates, a screen, a label, or a command; if it only appears in the
  code, say what the thing does for them or which screen it is instead. (Prompted by the v0.12.0 curation, 2026-09-22.)
