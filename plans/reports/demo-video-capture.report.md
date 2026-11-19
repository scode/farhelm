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
