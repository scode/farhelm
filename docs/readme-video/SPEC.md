# README demo video: requirements

This directory is the design of the demo video, and the requirements that govern how it is produced. The recording code
lives under `e2e/readme-video/` and the command under `scripts/`; this file is what they are built to satisfy. It is the
video counterpart of `docs/readme-hero/SPEC.md`, and everything that document says about what is real and what is staged
applies here unchanged, because the video stages its fleet with the same code.

NOTE: The current `intent.md`, scenario, and transcripts are a throwaway sample that only demonstrates the machinery.
This document describes the machinery and the process, which outlive that sample.

NOTE: This is not a visual regression fixture. Nothing in CI records the video, compares it, or fails when it drifts. It
exists so the published video shows the real product at whatever version is checked out, and so an agent can refresh it
without inventing anything.

## The one rule, and the layer the screenshot does not have

The design is fixed and the pixels are not, as for the screenshot. What differs is that a video has a choreography:
clicks, typing, annotations that point at things, all of which have to be expressed against a UI that keeps changing.
That is split into two layers with different owners.

`intent.md` is the script in the film sense: what happens, in what order, and the exact wording of every annotation. It
is the maintainer's, and it changes only when the maintainer wants a different video. It says nothing about selectors,
coordinates, or timings, so a UI change never invalidates it.

`e2e/readme-video/capture.spec.ts` is the compiled form of the intent: a Playwright spec that makes each beat happen
against the UI as it is today, with its beats numbered as in the intent. An agent writes it from the intent and rewrites
it when the UI moves under it. A refresh does not involve an agent driving the browser live; it replays the compiled
spec. That is deliberate. An agent deciding each step while the recording runs puts its thinking time into the video as
dead air and makes every run different, which is exactly what a reproducible capture is for avoiding.

`scenario.json5` and the transcripts under `transcripts/` are part of the design, like the intent. They use the
screenshot's scenario format, with one addition: a session whose transcript stops part-way for input names what it does
after its transcript ends (`then`), since its starting status no longer implies that.

## What the agent does on a refresh

"Refresh the demo video" means:

- Run `scripts/readme-video.sh`. It builds what is missing, stages the fleet, runs the choreography while recording, and
  prints the MP4, a marks file, and a directory of stills, one per mark.
- If the run fails because a beat can no longer happen as written (a selector that no longer matches, a control that
  moved behind a menu), rewrite that beat in the capture spec to realise the same intent on the current UI and run it
  again. Do not change the intent to make the capture pass.
- Look at every still. Each mark sits where an annotation is fully on screen, so the stills are the review surface: an
  arrow must point at what the intent says it points at, and an annotation must not cover what the viewer is meant to be
  watching. Fix placement in the capture spec (each callout takes a preferred side) and record again.
- Hand the MP4 to the maintainer to watch. An agent's look at the stills catches misplaced annotations; it does not
  replace someone watching the result.

The capture is not a test, but its waits are real readiness checks against the helm and the terminal buffer, so a beat
that cannot happen fails the run instead of recording a video in which nothing happens.

## Recording

The video is recorded through Playwright's screencast API, not its built-in video recording, whose encoder settings are
fixed (VP8 at about one megabit) and smear text. The screencast delivers each repaint as a JPEG with a timestamp;
`e2e/readme-video/recorder.ts` lays those frames out on a timeline, holding each one until the next, and encodes H.264
with ffmpeg once the choreography has finished, so encoding speed never affects what the browser shows.

Frames arrive at the viewport's CSS-pixel size whatever the device scale factor (observed with headless Chromium and
Playwright 1.62), so the scenario's viewport is the video's resolution and its scale stays 1. A page has one screencast,
and the first client to start it picks the frame size; Playwright's trace screenshots are such a client and cap frames
at 800 pixels wide, so the video's config keeps failure traces but turns their screenshots off, and the recorder fails
the run on any frame that is not the viewport's size. The first recordings came out at 800x450 before that was
understood.

Waits the viewer should not sit through, such as a status that takes several seconds to be re-sampled, are cut from the
timeline: the spec pauses the recorder around the wait and resumes it afterwards, and the video continues from the
paused frame without a gap. A cut is a jump in time, so it belongs where nothing on screen depends on continuity.

Everything the viewer sees is the real UI driving real terminals, with the screenshot's two exceptions (the staged fleet
and the two rewritten listing fields; see its SPEC). Because the video's rows keep the scenario's ages through the
listing rewrite, a session that becomes active during the video still shows its scenario age, and the list does not
reorder under the viewer.

## Annotations

Annotations (a visible pointer, callouts with arrows, highlights with an optional spotlight, captions, and title cards)
are drawn by the page itself, in an overlay injected by `e2e/readme-video/overlay.ts`, so they are part of every
recorded frame. They are anchored to elements rather than coordinates: the overlay re-measures its target every frame
and draws the arrow to wherever it is. That is what lets annotation wording stay fixed in the intent while the layout
under it changes.

The overlay sits in a shadow root on a full-viewport host that takes no pointer events, so scripted clicks reach the app
as if it were not there, nothing in the app's layout moves, and neither side's CSS can restyle the other. Headless
Chromium draws no mouse pointer, so the overlay draws one, and every scripted click glides it to the target and clicks
exactly where it lands.

## Publishing

The video is meant for YouTube, and the upload is done by hand. YouTube's upload API locks videos from unaudited API
projects to private, and an audit is not worth it for something refreshed this rarely. GitHub strips iframes from
READMEs, so the README cannot embed a player; it can show a still that links to the video. The docs website can embed
the player. Neither page is wired up yet.

## Prerequisites

Those of the screenshot (a cargo build, a dx web build, Playwright with Chromium installed under `e2e/`, passwordless
ssh to the scenario's remote destinations), plus ffmpeg with libx264 on `PATH`. The recorder needs Playwright's
`page.screencast`, which arrived in 1.59; `e2e/package-lock.json` pins 1.62, while `e2e/package.json` still admits older
versions, so install from the lockfile (`npm ci` or `npm install` without upgrading). The recorder fails with a named
error if the API is missing.
