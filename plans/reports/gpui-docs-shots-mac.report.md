## What this was about

The docs screenshot command could run the GPUI app locally, but Linux renders the test window at 1× and the docs shots
require 2×. The docs command now sends the source files to the Mac and builds and captures there. README hero and docs
captures take turns on the Mac. The originating checkout still checks the app source against main and writes the capture
record with the returned images' checksums.

## Things you should know

The test Mac captured the README hero at 2560×1600, from a 1280×800-point window at 2×.

Both Launch templates shots reached the size check. The Mac fitted the 1600×1000-point request to 1600×972, yielding
3200×1944 pixels at 2×. The runner refused both shots, returned no PNGs and stopped the staged app and supervisors. The
plan agreed upfront to keep the Mac path if the display was too small. A complete Mac docs capture remains unverified.

With keyboard focus on New session in the staged app, native Cmd+C left focus there and did not activate the system
audio output. A control beep activated the same output device.

## Open questions and possible follow-ups

On a Mac that fits the 1600×1000-point window at 2×, rerun the full docs capture to verify both checked Launch templates
crops return and the originating checkout records them.

## PRs

- [#4057](https://github.com/scode/farhelm/pull/4057/changes) adds remote Mac capture for docs screenshots.

## Checks run, reused and skipped

- After rebase, `bash -n` and `shellcheck -x` passed on both capture commands and the shared transport. `dprint check`
  on changed Markdown, `python3 scripts/check-agents-md.py` and `git diff --check` passed. The instructions checker
  repeated an existing archive heading warning.
- `python3 -B scripts/test-docs-shots.py` passed all nine tests (run b9ba858f-9541-48bf-b6a7-79dd9c6d0ab6).
- `scripts/readme-screenshot.sh --mac <destination> --output <file>` passed on the final transport (run
  aedf6d4b-eaf7-4b8f-a09f-da9a76f83e39). A separate manual Cmd+C probe posted the native chord to the staged app and
  sampled audio output with an idle baseline and a control beep.
- `scripts/docs-screenshots.sh --mac <destination> --only launch-templates` exited 1 and produced no PNGs or complete
  capture record in both Mac runs (950e1b85-ecad-4515-97c8-49a1ed5b07d1 and cab61462-11fb-489a-a08f-be0ae7407477).
- The local transport stand-in passed (selection `docs Mac stand-in including cleanup failure`, run
  2fa1d08c-a11e-4e4a-aba9-a21f260538f7). It checked the recorded main revision and PNG checksum, preview with no record
  for edited app source, partial capture with no complete record, and a cleanup failure after a successful fetch.
- The docs engine and Mac display checks ran against the patch on 85990c44. The rebase onto 1ba2ab0c updated separate
  entries in the check inventory and left the app, scenarios and capture engine unchanged. The later transport fixes
  were checked by the stand-in and final README run. Broader runtime suites were skipped because the patch changes media
  tooling and documentation.

## Review gate

Passed. GPT-6.2 Sol at high reasoning reviewed the change. The executor verified fixes for all four findings: retaining
successful shots from a partial capture, keeping cleanup errors separate from transfer success, preserving the README
preflight's disk minimum, and removing stale docs refresh wording. Separate cold reads led to a more direct PR title and
description.
