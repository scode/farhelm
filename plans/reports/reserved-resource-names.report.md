# Farhelm's reserved resource names and labels

## What this was about

Three review findings concerned sandboxes or containers created outside Farhelm. Reusing a name Farhelm reserves, or
using a Podman clone with copied Farhelm ownership labels, can put independently created work in scope for cleanup. The
maintainer decided that Farhelm may treat those resources as its own.

## Things you should know

The specification now explains the ownership rules for each operation. Tensorlake leftover deletion asks for
confirmation. During recovery of an outstanding create with no saved sandbox ID, Farhelm can identify the sandbox by its
exact recorded name. Delete of a retired Podman session uses its legacy container name. Podman leftover cleanup checks
both the helm tag and the container role label in the host's Unix account and asks before deleting a leftover. The
change is documentation and bookkeeping. The three related decisions are in the archive with their PR links.

## Open questions and possible follow-ups

None.

## PRs

- [#4148](https://github.com/scode/farhelm/pull/4148/changes) documents the reserved ownership rules and records the
  three completed triage outcomes.

## Checks

- `dprint check SPEC.md TRIAGE_OUTCOMES.md TRIAGE_OUTCOMES_ARCHIVE.md review_feedback_queue/INDEX.md` and
  `git diff --check` passed at `355ccbe9e860`.
- `python3 scripts/check-agents-md.py` reported zero errors and one pre-existing warning about an archived citation to
  the “Bound polling time” heading.
- Used fresh source checks. Skipped runtime tests because the change edits the specification and review bookkeeping.

## Review gate

A fresh gpt-6.2-sol reviewer at high effort read the specification and triage diff. The new paragraph now specifies the
different Tensorlake deletion and recovery conditions, the Podman label pair, and the scope of Delete for a retired
session. The reviewer checked those corrections and the three archived entries. A separate cold reader checked the
commit and PR wording.
