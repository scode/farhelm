### What this was about

Removing the browser app broke the command that recorded the README demo. It now records the Linux desktop app against
the existing throwaway fleet and writes an MP4, a list of cue timestamps and seven review stills. The sample still runs
through its two live terminal replies. The overlay draws the cues and lets clicks reach the real controls.

### Things you should know

- The local handoff contains a 90.4-second, 1920×1080 H.264 sample. At 24 fps, software rendering kept the scripted
  controls from responding in time. The 1 fps default keeps them responsive and lengthens fades and pointer glides so
  the recorder captures intermediate frames. `--fps` sets a higher rate on machines that can sustain it.
- The video points to the migration question and to the open session's Running marker. It uses the same crop and private
  staging as the screenshots. It parks the real pointer away from hover targets and runs the screenshot leak checks at
  segment boundaries and marks. Review the full playback before the manual upload.
- The joined video cuts the two scripted waits for agent state changes. Each review still is the first frame of a fresh
  segment that starts once its cue is visible, so the still and video show the same scene.

### Open questions and possible follow-ups

The sample is for checking the recording workflow. The actual demo's scenario and wording are a separate design task.

### PRs

- [#4178](https://github.com/scode/farhelm/pull/4178/changes) paints the staged recording cues in the desktop app.
- [#4184](https://github.com/scode/farhelm/pull/4184/changes) records the scripted desktop demo and its review stills.
- [#4185](https://github.com/scode/farhelm/pull/4185/changes) documents the refresh and removes the finished TODO entry.

### Checks

- Live `scripts/readme-video.sh --no-build` on Linux completed in run `96c853af-6573-41bf-89b6-38e42ab6550e`. Reviewed
  all seven stills and all 92 decoded frames in order. Verified the animation steps, the terminal flow, the crop and
  every mark's encoded timestamp. A local OCR pass found none of the checked machine paths or names. The final overlay
  also caps a staged cue at three seconds, which is the duration this run already used.
- `cargo nextest run -p farhelm-gui --features agent --lib -E 'test(/^demo_video::/)'` passed 1 of 1 in
  `98490232-b718-4d67-8335-a937270e4cb8`. `cargo clippy -p farhelm-gui --all-targets --features agent -- -D warnings`
  passed. Reused the no-default-features GUI build at `f6561faf`, since the later overlay edits remain behind the
  `agent` feature.
- The Python tests `scripts/test-readme-video.py`, `scripts/test-docs-shots.py` and `scripts/test-gui-agent.py` ran with
  `python3 -B` and passed 6, 25 and 93 respectively in `d79d5b87-9335-4756-bdc7-ce21647d6a5b`. Rust fmt, scoped dprint,
  shell lint and the test-delay check passed. The instructions check found zero errors and two pre-existing heading
  warnings.
- The full workspace and release suites were skipped because the capture uses staged desktop tooling. The focused checks
  cover the changed contracts.

### Review gate

All three drafts passed their independent high-effort review, with findings rechecked. The fixes covered click-through
and alignment, selecting the intended frame at each mark, keeping its presentation time, preserving earlier assets on a
failed replacement, showing movement at the lower capture rate, and documenting the capture and privacy requirements.

### Landing

#### What landed with it

It landed in a round of three plans: covered-link-refusal, demo-video-capture and replace-with-checkout-hint. Since its
base, main gained the previous round's nine plans (among them session group dragging, model families, the session-limit
notice and compare-on-save checkout settings), bookkeeping and docs screenshot tooling. None of it touches the video
tooling. covered-link-refusal, in the same round, changes the layer manager the recording overlay paints through; the
two do not interact.

At your request this landing was reviewed with extra skepticism, by me and by independent reviewers, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. Nothing video-only can run in a
release build: all of it is behind the test-automation build, which optimized builds refuse. Every edit to the demo
video's design files is covered by your instruction to update the existing video in whatever way makes it work.

#### Fixes made while landing

- **The video's spec lost content unrelated to the browser.** The rewrite of `docs/readme-video/SPEC.md` dropped:
  - the Publishing section (YouTube, why the upload is manual, the README's linked still);
  - "Do not change the intent to make the capture pass";
  - that a cut belongs where nothing on screen depends on continuity.

  It also garbled the sentence on how a session that stops for input says what it does next. All are restored.
- **The animation module's rules no longer held.** It allows a transition only where SPEC.md asks for one, and only
  under the app's own reduced-motion setting. The new recording transition does neither. The recorder also turns GPUI's
  own reduced-motion flag off in its process. The module docs now record this as an exception for the test-automation
  build only, and a cut sentence is restored: that the app's own transitions read reduced motion from the app's setting,
  not GPUI's flag.

#### Things you should know that the report does not say

- **The video is one frame per second.** The recorder captures at 1 fps at 1920x1080, because at 24 fps the software
  renderer on the test machine kept the app from handling clicks and keys. Every glide and fade is stretched to about 3
  seconds so it spans three frames, and typed text lands in about one frame. The result reads as a slideshow. The plan
  asked for the rate to be chosen after measuring, and nothing between 1 and 24 fps is reported as tried. Please watch
  the sample the executor handed over (or regenerate it with `scripts/readme-video.sh`) before deciding on publishing: a
  faster renderer or a higher rate on a better machine (the script takes `--fps`) may be needed for a real video.
- **The script's output step is bigger than the job.** Writing the finished video now takes a directory lock, refuses
  symlinked outputs, checks the previous stills against the previous run's list of annotation marks, and installs with
  backup and rollback. Three of the six tooling tests cover only that. It is for a throwaway sample under the build
  directory, and the executor's own reviewer asked for it, which the plan said to decline by default. Possible
  follow-up: cut it back to delete and write.
- Smaller: the video spec does not say callout boxes are now measured once per scene step, so a callout can fall out of
  line with UI that moves during the step; the GUI ownership document does not mention the new way the recording overlay
  draws on top of everything; and the scripts' `--help` explains little.

#### Checks

The three plans change the desktop app's GUI crate, a patch to the vendored GPUI core, the demo video tooling and its
documents, AGENTS.md and the specs. The landing ran the lints, the GUI crate's whole suite, the vendored GPUI core's own
tests, the video tooling's tests, the app's end-to-end smoke and the reference screenshots (goldens). Builds ran
locally; the GUI suite and the smoke ran on boat sandboxes (cloud VMs), from binaries built locally.

On the plans as delivered, stacked on main as it stood when the round started:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`,
  `python3 -B scripts/test-readme-video.py`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings` and
  `python3 -B scripts/check-gui-lints.py`: passed.
- The vendored GPUI core's tests (`cargo test --manifest-path vendor/gpui-pre/Cargo.toml --features test-support` with
  the native-key, driver-record and new hit-test occlusion tests), locally: passed
  (`3130bce8-b268-4c31-977b-5bc1afe5a949`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`01c51026-2b44-41af-9830-f4c6eb0e081a`).
- The boat runs did not happen: every remote command on all four boats ended without reporting its result, and the
  harness deleted the boats as unsafe to reuse. That looked like a problem on the boat service's side; nothing ran.

On the fixed tree (the covered-link refusal fix and its test, a screenshot scene's staging, and documents), on four
fresh boats and locally:

- `cargo nextest run -p farhelm-gui`, from a locally built archive, split in three across boats: 1085, 1033 and 1022
  passed, 3140 in all, with no runtime skips (`71d71daf-a7e6-4aab-9dc6-39032f010643`,
  `176070d9-503b-4ea8-a89c-39c3f0062335`, `3d8609ce-34b9-431e-821c-e4ed78423b3d`).
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on a boat: 49 of its 50 steps passed and the clipboard
  paste step was skipped, as always on Linux (`409e2e23-c47f-4c6d-9d19-5355b3103ec2`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`29aca505-d814-43b8-b492-c29f1eea2d14`).
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 scripts/check-agents-md.py` and `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings`, once
  without and once with `--features agent`: passed.

Reused for the fixed tree: the vendored GPUI core's tests and the video tooling's tests, whose code did not change, and
the GUI lint check, the workspace and desktop clippy runs and the changelog format check, since the fixes changed only
the GUI crate (covered by its clippy runs above) and documents, and no lint configuration or changelog fragment.

Skipped:

- The rest of the Rust workspace and the desktop process's tests: the plans stay in the GUI crate and the vendored GPUI
  core.
- A full capture of the demo video: the executor captured the sample and handed it over; the landing changed none of the
  capture code.
- The Mac-only checks: no Mac-specific code changed.
