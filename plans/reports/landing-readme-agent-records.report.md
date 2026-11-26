# Report: point the landing tooling README at the agent records doc

## What this was about

The landing monitor's tooling README (`scripts/plans-landing/README.md`, the guide to the commands the landing monitor
runs each round) explains the monitor's own records and `cargo xtask records`, but never said where agent records as a
whole are documented. A reader of that README had no pointer to `.agents/agent-records.md`, which describes the records
repository, the files each host keeps for it, and the records tool. The TODO entry asked for one line adding that
pointer, kept deliberately small because this was the first plan to run with its logs in agent records. No triage
decision or earlier Decisions entry applied; the planning decisions were a single PR, an Opus 5.5 high review gate, and
no release note.

The PR adds one sentence at the end of the README's first paragraph under "Arguments every command shares", which is
where the README first introduces the `landing` record and the records tool: "How agent records work (the repository,
the host files and the records tool) is in `.agents/agent-records.md`." It also removes the covered entry from
`TODO.md`.

## Things you should know

- The sentence sits at the end of that paragraph rather than right after the first mention of the `landing` record. Put
  mid-paragraph, it made the following sentence's "the commands that edit it" read as if "it" meant the agent records
  doc instead of the monitor log.
- No changelog fragment: the change has no effect on anyone running Farhelm.
- This plan ran with its working log and the executor log in agent records, as intended. Claiming, pushing the records
  and opening the PR all went through without trouble.

## Open questions and possible follow-ups

None.

## PRs

- #5058 — docs: point the landing tooling README at the agent records doc (draft).

## Checks

Run now, on the PR's commit:

- `dprint check scripts/plans-landing/README.md TODO.md`: passed.
- `python3 scripts/check-agents-md.py`: passed with 0 errors. Its one warning is about a citation in
  `TRIAGE_OUTCOMES_ARCHIVE.md`, a frozen file this PR does not touch, and was already there before this change.

Skipped: every Rust, JavaScript, desktop, installer and provisioning test, since this is a Markdown-only change with no
generation or doctest role (`AGENTS.md`, "Tests and validation"). Nothing was reused from earlier runs.

## Review gate

A single fresh-context Opus 5.5 reviewer at high effort, using the plan's general charter, reviewed the PR's commit and
found nothing to fix. It confirmed the change is confined to the two files, that the sentence's description matches the
sections of `.agents/agent-records.md`, and that the commit title is valid Conventional Commits.
