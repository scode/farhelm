## Since the last review

The same three draft PRs remain. The host-side read PR (#1769) now uses protocol 45 alongside main's checkout-trash
protocol 44 and coalesces short filesystem reads into full frames, ending at the first EOF. The endpoint/native-save PR
(#1771) was rebased without changing its implementation. The terminal PR (#1773) now places status at the top, fades
success after five seconds, dismisses failure on the next terminal click or download, and documents intentional saves
without macOS quarantine. No PR was added or dropped.

## What this was about

Agents often return a file by printing its path, including one on another host. Farhelm previously offered no way to
save that file from the terminal. The maintainer chose a fresh hover that discloses host, resolved path and size before
an explicit click, a 100 MB limit, browser saves by basename, and direct Downloads saves in the desktop app.

Absolute and home paths, relative paths containing a slash or file extension, and OSC 8 file hyperlinks are recognized.
A hover immediately shows checking, then the source or a refusal. Downloads are complete before publication, native
saves pick an unused name, and failed or interrupted native saves leave no partial file. The covered TODO entry is
removed.

## Things you should know

Relative paths use the session's launch directory, not a shell's later directory. Ordinary printed paths stop at
whitespace, quotes and brackets; OSC file hyperlinks can carry spaces. Diagnostic line/column suffixes and sentence
punctuation are excluded. Words, numeric versions and scheme-bearing plain text are not file links. Existing web links
retain their behavior; OSC accepts only http, https and file.

Any regular file readable by the session's Unix account is in reach, including symlink targets. Agent-controlled text
remains data, and only the helm may request host-side file reads. A fresh disclosed hover and explicit click authorize
the host-to-viewer transfer. Directories, unreadable or missing files and files over 100,000,000 bytes are refused;
growth beyond the limit fails during streaming. Nothing opens or executes the result. The desktop app deliberately adds
no macOS quarantine flag, as the maintainer decided; SPEC.md now records that choice in its security section.

Status stays away from the agent's input and recent transcript at the bottom. Both browser and desktop success notices
fade after five seconds. Failures stay until the next click or download in that terminal. Notice timers and listeners
belong to the terminal mount; late cancelled replies cannot restore them after closing it.

The protocol now moves from main's 44 to 45. Mismatched supervisors use the existing update path before reconnecting; no
compatibility layer or installer behavior was added. Careful rebase preserved the landed checkout-trash dispatch, host
icons, sidebar geometry and session-directory contracts. Review also exposed a queue assumption: byte credit alone did
not bound the number of legally short filesystem reads. The producer now fills frames and treats its first partial EOF
as final, preserving the bounded queue for slow consumers without another subsystem.

Earlier execution inspected real Chromium local/SSH hovers and exact saved bytes after sidebar drags, plus a real Linux
desktop save and unused-name second save into an isolated Downloads folder. Those observations cover the unchanged
lookup/native-save behavior. This round's rendered notice evidence and current matching-build tests are listed below.
macOS was not tried.

## Open questions and possible follow-ups

No implementation decision awaits the maintainer. A macOS native-window check remains useful because platform folder
resolution and the embedded shell were not exercised on macOS.

## PRs

- [#1769](https://github.com/scode/farhelm/pull/1769/changes): host-side inspection and bounded streamed reads.
- [#1771](https://github.com/scode/farhelm/pull/1771/changes): authenticated endpoints and native Downloads saves.
- [#1773](https://github.com/scode/farhelm/pull/1773/changes): terminal recognition, hover, click, notices, specs and
  user docs.

## Checks run, reused and skipped

Ran the focused supervisor/protocol checks (`94649fe6-8986-4df9-86c6-b559397a71e2`): five passed, covering protocol 45,
access checks, credit, growth and full frames through first EOF. Its source was tip `75b963476df7`; the only subsequent
Rust change collapses a fixture condition with identical behavior, independently reviewed. Current supervisor Clippy and
formatting pass. Endpoint/native-save checks (`3eaff8e0-0674-4e2b-a744-5b69d78a563b`) passed seven, including auth,
CORS, streaming, unique names and cleanup. Selection exclusions were not runtime skips.

Ran final terminal JavaScript (`2db72d0d-93f3-4eee-9eba-f67025ae1fd4`): nine passed, including pre-open mount/disposal,
default browser timers, both save outcomes and stale replies. The earlier whole UI harness
(`9372cbd0-44c4-4e75-8e64-973b9805401c`) passed 212; its other 204 cases are unchanged and reused. Both cargo and
corrected web builds passed. Current matching-build file spec (`69d5d639-be2d-4795-b8ba-99b71d342901`) passed all eight
across Chromium/WebKit, one worker and no retries/skips: local/SSH exact saves, top placement, five-second fade, click
dismissal, OSC refusals and hover cancellation. Runtime Rust runs used pinned nextest 0.9.143 and tmux 3.7c, four slots
and zero retries.

Protocol integration run `67ddc08b-dff7-4eaa-8b72-a88f6a4577b5` passed four cases after its fixture build: complete
credit-paced reads, control traffic and abort, channel reuse, and oversized-file refusals. No selected test printed a
runtime substrate skip.

Manual run `150e1295-1964-4351-8646-3debca511dbf` passed local and SSH saves with exact byte equality after verified
sidebar drags. The executor inspected four screenshots: host, full path and size fit the hover without clipping, file
text was underlined, and the success strip sat at the terminal top while leaving the bottom clear. Both success notices
faded. These are browser observations; macOS was not tried.

Ran final formatting, changed-spec Markdown checks, changelog lint (34 fragments), and the isolated delay checker (279
delays, zero missing rationales); all passed. Later main changes through `8bc1af9a5d9c` affect OMP Resume attribution,
host-support/uninstall prose and shared bookkeeping; they change no download contract, caller or dependency. Existing
coverage therefore applies without another rebase or broad runtime run.

Prior evidence covers the old pushed heads `1e68f6e5bd75`, `6e69f0f242f8`, and `bd8f1c0e5591` and their pre-rebase
source snapshots. Added/removed-line comparison against the new main preserved every plan line except the intended
protocol-version changes; this round separately validates the notice and producer corrections. Authenticated
endpoint/native-save run `4fdb34e2-d6ba-466f-8a74-bc32ab928a2a` passed seven cases. Existing web-link run
`3872b9b8-455e-462d-96b8-eb56d209b21b` passed 28 cases, and later integrated file/wrapped-link run
`8490694e-ba4f-4297-9b55-4e830927145a` passed ten across Chromium/WebKit. The unchanged web recognizer, scheme handling,
selection and wrapping contracts retain that evidence; current file tests cover changed integration.

Earlier desktop feature and binary compile checks, asset parity, docs website build, and the inspected Linux native
attempt `6c7e190d-ff49-44c6-b77e-adf72d92fc2d` are reused. This revision changes no native folder API, native save
implementation, asset registrations, website source, executable examples or generated artifacts. The native notice's
shared JavaScript lifetime is exercised this round; WebKit browser coverage checks the engine family, not the desktop
shell. Old unit/Clippy evidence is supplementary; current protocol/producer checks cover the changed invariant.

The full Rust/browser batteries, desktop shell rerun, doctests, installer, provisioning, release workflow and hosted CI
were skipped because targeted checks cover the changed contracts and concrete interactions. No deployment, release or
live-install mutation was performed.

Failed and interrupted evidence remains private. In this round `231f8236-24cc-484c-b395-2932170d537e` failed a new JS
fixture's guessed microtask readiness; an explicit gate fixed that premise, and later runs are separate observations.
Unit run `8aafd23b-fca2-4a9f-a7f2-ced0ef2b8be7` and its concurrent build were deliberately interrupted before another
producer correction; no runtime pass is claimed. Earlier development/prerequisite failures remain retained in the plan's
working log. Browser run `7b31637c-50ad-44df-9460-ca5c4c573f4e` passed four and failed four: files saved completely, but
the default success timer had an invalid browser receiver and reported failure. That defect was fixed in this session.
The first supervisor Clippy attempt also found a nested-if style error in the new fixture, corrected without changing
behavior. None is classified as a latent flake.

## Review gate outcome

The revised protocol and terminal PRs passed fresh-context gpt-6.1-sol high general reviews with the full test-authoring
checklist. Corrected findings were the pre-open xterm lifecycle and the short-read/first-EOF queue invariant.
Matching-build browser checks also caught a default timer receiver error after a successful save; global-call wrappers
fixed it, with a controlled regression and a further accepted source review. The endpoint PR's prior review is reused
because its implementation is preserved. Required scope reassessment assessed the corrections; it found no unnecessary
mechanism. Every touched file received a separate documentation pass. The host-read commit and PR description were
refreshed to protocol 45 and passed a new wording cold read; the other wording retains its earlier cold-read results.
Reviewers performed source inspection; runtime results and artifact inspection belong to the executor. Native exact
model reporting and usage counters were unavailable.

### Landing

Landed on 2026-10-10 (UTC) as three squash commits on main, in order: #1769 (session-host file reads), #1771
(authenticated file downloads) and #1773 (downloads from terminal paths), in the plan's second round, after
sweep-on-timer's follow-up and waiting-sound. You decided the status message moves to the top of the terminal (success
fades, a failure stays until the next click or download), that downloads deliberately carry no macOS quarantine flag and
SPEC.md says so, and that the protocol moves to 45 because managed-checkout-trash took 44. The rebase met only the
TODO.md entries removed by this plan and waiting-sound, and, at the last merge, the SPEC.md conflict described below.

#### Review before merging

A separate reviewer read the round by reading the code only and found the decision met: the strip sits at the top and
the browser test checks its position, "saved to" fades after five seconds, a failure stays until a click on the terminal
or another download, and SPEC.md's local-authority section records the host-to-machine transfer and the absent
quarantine flag. Protocol 45 is consistent everywhere, and the trash plan's version-44 messages are all still present.

#### Fixes made while landing

- The status strip kept the stacking layer it had when it sat at the bottom, which now let it draw over the header's
  Restart, Replace and Delete confirmations, over dialogs such as the quick switcher, and over the file-hover disclosure
  that shows what a click would download. It now sits just above the terminal's own layers and below every other layer,
  is listed in the stylesheet's layer list, and is capped in height so a long failure message cannot reach down over the
  agent's input. In #1773.
- One new end-to-end test used a nested condition that the toolchain's lint now asks to be collapsed; the landing
  collapsed it. In #1769.

Smaller notes left as they are: one browser assertion that no download request was sent runs immediately after the
click, so it could pass before a request is reported; and the changelog fragment and user docs do not mention the absent
quarantine flag (SPEC.md does, as you asked).

#### Checks

All three plans of this round (sweep-on-timer's follow-up, waiting-sound, terminal-file-download) were stacked in
landing order and checked together.

- Run now: `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop` and `cargo build` (clean after the landing's fixes below); the
  supervisor, helm, UI and protocol unit tests in full with the file-download end-to-end tests, through the recorder
  with pinned tmux 3.7c, four slots and no retries (run `6225cfec`, 2699 of 2700; the failure is sweep-on-timer's
  database upgrade test, fixed as described in its notes, after which the upgrade, schema and store tests passed 113 of
  113 in run `106eaf3c`); the UI JavaScript tests run directly (226 of 226); the desktop asset comparison (run
  `d09df54a`, 24 assets on both sides); and, on Chromium and WebKit with one worker and no retries, the sounds,
  settings, change-feed, sort, stale-read, filter, approval-layout and terminal-file specs (run `208a7db7`, 122 of 122),
  which include the four specs the first waiting-sound round broke.
- Between the checks and the merges, main gained the 2026-10-10 triage decisions written into SPEC.md (#1801) and its
  plan (#1802). They touch none of these plans' subjects; the only effect was a textual SPEC.md conflict in the last
  terminal-file-download PR, resolved by keeping both texts. No checks were re-run for that.

Nothing in the report above was made untrue by the landing, beyond the strip's layering described above.
