# README demo video requirements

`scripts/readme-video.sh` records the Farhelm desktop app against a staged fleet. The staging rules in
[the README hero requirements](../readme-hero/SPEC.md) apply to both captures.

NOTE: The current intent, scenario and transcripts are a throwaway sample that exercises the recorder. Nothing in CI
records or compares the video.

## Design and choreography

`intent.md` gives the order of the beats and the exact wording of their annotations. The fleet in `scenario.json5` and
its transcripts are also part of the design. The maintainer changes those files when the video needs a different design.
The fleet and transcripts use the screenshot's scenario format, with one addition: a session whose transcript stops
part-way for input names what it does after its transcript ends (`then`), since its starting status no longer implies
that.

`scripts/readme_video.py` runs the numbered beats against the current app. It locates controls by the bridge's stable
element IDs, checks the real status from the helm and waits for replay in the focused terminal before typing. If a
control moves or a beat no longer works, update the script to realize the same intent. Do not change the intent to make
the capture pass. The script runs without a person or agent driving the app live, so thinking time stays off the
timeline.

## Recording

The script starts its own Linux desktop and staged Farhelm instance through `scripts/gui-agent.py`. The viewport gives
the output's resolution at scale 1. The recorder checks the window's decorated size and each PNG's dimensions before
encoding. The MP4 and the marked stills use the same crop. At scale 1, it removes the 21 px Linux frame on each side and
the 34 px app title bar at the top, giving the content an origin of (21, 55) in the bridge's window coordinates. A crop
that fails leaves the run for inspection.

Each segment starts and stops the bridge's PNG recorder. Its measured duration gives its frames their playback rate. The
default capture rate is 1 fps at 1920×1080. At 24 fps, readback on the test machine's software renderer kept the bridge
from handling clicks and keys. Use `--fps` to set a higher rate on a machine that can sustain it. At lower rates, the
script gives the fades and pointer glides time to appear across several frames. Between segments, the script waits for a
state the viewer need not sit through. Joining those segments leaves that wait out. A cut is a jump in time, so it
belongs where nothing on screen depends on continuity. At each mark the recorder waits for the cue to paint, then starts
a fresh segment with the cue still in place. The bridge saves PNGs asynchronously, so the newest file from the previous
segment could still show the previous cue. The script spaces each segment's PNGs evenly across its measured duration.
ffmpeg encodes the joined timeline once as H.264, preserving the assigned times. The marks file gives the first frame's
time on the joined timeline, and the review still is cropped from that frame's PNG.

Every frame and still must be free of private paths and names. The fleet and fake-agent replies are staged. Terminals
and agent status are real, and the app uses the same pinned ages and folders as the hero capture. The recorder parks the
real pointer away from hover targets between clicks. It runs the screenshot privacy checks at segment boundaries and
marks, using the raw window tree and focused terminal. A detected machine detail fails the capture before it writes the
MP4.

## Annotations

The bridge measures controls in the decorated window. The harness subtracts the crop origin before laying out callouts
with the docs screenshot helpers, then sends the finished coordinates to the app. The app paints the pointer, captions,
title cards, callouts, rings and spotlights above the existing layers in staged capture builds. The overlay stays out of
focus and pointer dispatch. The script uses the real bridge input for each click and waits for the pointer glide to
finish first.

## Refresh and review

1. Run `scripts/readme-video.sh` on Linux. It builds the capture app and prints the MP4, marks file and review stills.
   The default output is `target/readme-video/readme-video.mp4`. Use `--no-build` with current binaries and `--output`
   for a different destination. Failed runs keep their private directory for diagnosis.
2. Look at every still. Arrows should point at the intended control or terminal line and leave the relevant UI visible.
   Adjust the placement in `scripts/readme_video.py` and record again if needed.
3. Play the whole MP4 to check the cuts, cue timing and privacy. Hand over the MP4, marks and stills for review. Mention
   the throwaway sample while `intent.md` still says so. Upload is manual.

The harness needs the Linux tools in [the GUI agent instructions](../gui-agent.md), including Mutter and lavapipe, and
ffmpeg with libx264. It starts a private SSH server for its staged hosts.

## Publishing

The video is meant for YouTube, and the upload is done by hand. YouTube's upload API locks videos from unaudited API
projects to private, and an audit is not worth it for something refreshed this rarely. GitHub strips iframes from
READMEs, so the README cannot embed a player; it can show a still that links to the video. The docs website can embed
the player. Neither page is wired up yet.
