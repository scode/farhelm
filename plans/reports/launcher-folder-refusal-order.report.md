# Show the folder error the host reported

## What this was about

The session launcher can show a warning under the folder field when the host refuses the path. It used to look for
familiar words anywhere in the error and pick whichever warning it checked for first. A path name containing those same
words could make the launcher say a file was missing when the host had refused it because it was a file.

The launcher now reads the first matching phrase in the error. The host puts its diagnosis before the path it names, so
a second matching phrase in the path cannot change the warning.

## Things you should know

The warning still points at the affected field. The host still decides whether it can launch there. The change covers
both a folder check and a refused launch. The feedback item moved to the triage archive.

## Open questions and possible follow-ups

None.

## Pull requests

- [#4387](https://github.com/scode/farhelm/pull/4387/changes) fixes the folder warnings in the session launcher.

## Checks

- Ran 32 focused Linux GUI tests with `cargo nextest run -p farhelm-gui --lib -E` and the folder classifier, folder
  check, browse and replacement test selections. All passed in recorded run `55f49942-7ea7-4562-9511-26d9bf083cfb` on
  `5e028e12` plus the fix. The rebase onto `33feb664` kept the tested source diff byte-for-byte. The intervening changes
  affected installer tests, release harness tests and queue bookkeeping.
- Rust formatting, Markdown formatting, the changelog and test sleep checks, and the whitespace check passed. The
  instruction link checker reported zero errors and one existing citation warning.
- The focused tests cover the matching logic and its launcher callers. The warning layout stayed the same, so I skipped
  screenshot checks, the full workspace suite and native desktop testing.

## Review

Independent source review confirmed that both new cases distinguish the host's diagnosis from the words in the path.
Neither case would pass under the earlier matching order. Commit wording received a cold read.
