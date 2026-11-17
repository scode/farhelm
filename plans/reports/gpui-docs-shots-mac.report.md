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
- The docs engine and Mac display checks ran against the patch on 94bd124e. The rebase onto a34463f6 updated separate
  entries in the check inventory and left the app, scenarios and capture engine unchanged. The later transport fixes
  were checked by the stand-in and final README run. Broader runtime suites were skipped because the patch changes media
  tooling and documentation.

## Review gate

Passed. GPT-6.2 Sol at high reasoning reviewed the change. The executor verified fixes for all four findings: retaining
successful shots from a partial capture, keeping cleanup errors separate from transfer success, preserving the README
preflight's disk minimum, and removing stale docs refresh wording. Separate cold reads led to a more direct PR title and
description.

### Landing

#### Where docs screenshots stand after this

No machine available today can produce docs screenshots. Linux renders the test window at 1× and the pictures need 2×,
and on the test Mac the app fitted the docs window to 1600×972 points instead of 1600×1000, so both Launch templates
shots stopped at the size check there too. Why the Mac's display left only 972 points of height was not established;
until it is, it is not known whether a display setting on that Mac would be enough or a Mac with a larger display is
needed. The README hero, whose window is smaller, captures fine on the test Mac.

#### What landed with it

Nothing else landed in this round, and since the stack was built main gained only plan-queue changes, so nothing could
interact with it. An independent reviewer read the change against main. It confirmed that the README hero's Mac capture
behaves as before its Mac transport moved into a script shared with the docs command (same exit codes, and the old
picture is still removed before any step on the Mac), and that no failed or partial docs capture can leave a capture
record that the docs publish script would accept: the record is deleted before a run starts and written only after a
complete, checked fetch. Every fix below came from that review; I checked each against the code before making it.

#### Fixes made while landing

- **Two entries in the GPUI port's parity notes had lost their status** (`docs/gpui/parity/foundations.md`, the docs
  screenshot and README hero captures). Those notes require every entry to keep a status line, and the change had
  replaced the status with the test Mac's results. The status line is back, with the Mac results listed under it.
- **The docs instructions now say the docs capture does not fit the test Mac.** They described the new Mac path as
  giving 2× pictures, with nothing about the display. An agent asked to refresh the docs screenshots would have met the
  size-check refusal with nothing telling it not to work around it. The docs media instructions now carry the same note
  the README hero has: report that the display is too small, and do not work around it by resizing the window, scaling
  the picture or patching GPUI. The docs screenshot spec and the check inventory also say the Mac path works only on a
  display that fits.
- **A failed Mac capture now says where its leftovers are.** With this change, every Mac run writes its output and log
  under new file names, where the README hero used to reuse fixed ones. A failed run leaves its log on the Mac, and a
  run whose status never came back over ssh also leaves its output, but the failure messages did not say where. They now
  name both files. Nothing cleans those files up automatically; they are small, and whoever looks into a failure can
  remove them.
- **Two small pointers.** The README command can take a default Mac from an environment variable; the docs command's
  usage text now says that variable does not apply to it (it needs the Mac named every time). And the agent
  instructions' list of files covered by the docs media instructions now includes the new shared script.

#### Things you should know that the report does not say

- The plan removed the TODO entry "Docs screenshots on the test Mac", as its done criterion required. That entry also
  asked to prove both captures fit on the test Mac's display, which did not happen (above), so once you approve the
  report, the report's follow-up is the only record of it. Whether to keep a narrowed TODO entry for it is your call.

#### Checks

The change is capture tooling and documentation: no product code, no Rust, nothing a test suite exercises. All checks
ran locally, directly rather than through the test recorder (they are static checks and a capture tool run, not test
suites), so they have no run ids.

On the stack as delivered:

- `bash -n` on `scripts/readme-screenshot.sh`, `scripts/docs-screenshots.sh` and `scripts/media-mac.sh`, `shellcheck -x`
  on all three, `python3 -B scripts/test-docs-shots.py` (9 passed), `dprint check`, `python3 scripts/check-agents-md.py`
  and `git diff --check`: passed.
- One live `scripts/docs-screenshots.sh --local --only launch-templates` run, which built and staged the GPUI app: both
  shots stopped at the size check as designed on Linux, the capture's checks that no machine path or account or host
  name is drawn in a shot passed, and every terminal host stopped cleanly.

After the landing fixes, which changed Markdown, two failure messages and a usage comment in the shell scripts, and one
line of `AGENTS.md`: `bash -n` and `shellcheck -x` on the three scripts, `python3 -B scripts/test-docs-shots.py`,
`dprint check` and `python3 scripts/check-agents-md.py`: passed.

Reused: nothing. Skipped: the Mac capture itself, because the landing has no access to the test Mac and the landing
fixes change nothing that runs there; and the Rust, GPUI app and installer suites, since nothing they exercise changed.
