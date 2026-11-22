# The supported shell release-note recapture parks on the removed launcher title

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

The shell release-note recapture tries to park on a launcher title that the form no longer draws.

## Details

**other.** Source inspection.

scripts/docs-shots/release-notes.py:149–157,202–218; scripts/docs_shots.py:415–449;
scripts/gui-agent.py:1841–1854,1870–1889 at 1a53d9d35889d7dde96e31320cc1681dd013737d; introduced by
f6b5e5b1a39cb2b69c817a9c1f4d1153ee1f67c6

An explicit correction of the shell sessions release-note shot reaches its current declaration and fails while parking.
docs/docs-shots/SPEC.md:96–105 documents scripts/docs-screenshots.sh --local --recapture-release-shot NAME, optionally
with --only release-notes for preview. The wrapper forwards the same selection to --check-selection and then the actual
capture (scripts/docs-screenshots.sh:50–91,135–146). The Python engine imports and selects the named declaration when
its publication manifest and release listing agree (scripts/docs_shots.py:65–86,110–163,464–497). Both shots qualify
(website/src/data/docs-shots.json:368,396 and website/src/release-notes/v0.42.0.json:15, v0.43.0.json:5). A normal batch
skips them, and full-shot --only alone refuses them. The shell shot waits for Bash and Zsh and then the engine passes
its exact park target to gui.capture (release-notes.py:202–218; docs_shots.py:425–433). gui-agent.py:1841–1854,1870–1889
requires that hover to succeed even with --keep-focus. The launcher uses the control heading
(launcher/view.rs:2705–2711), which omits the id drawn by the fallback title (components/dialog.rs:407–425). Change both
stale park selectors. Antigravity first meets LV-9, so correcting only its staging would expose this later failure.
Ordinary batches preserve the published shots. The reviewer suggested other (maintainer capture). Confidence: confirmed
from source.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-visual/LV-8`.
