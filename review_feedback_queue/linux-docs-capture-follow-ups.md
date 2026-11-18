# Three screenshot capture follow-ups after Linux 2x support

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

The README hero still uses the Mac workflow by default. A small Templates button crop would fail the publisher’s size
check if a page uses it, and the template search crop catches a warning below the search box.

## Details

Bucket: **other**. GUI migration final whole-tree review, #4088 docs screenshot follow-ups (three observations).

These follow-ups were observed during #4088, after the reviewed commit above. The post-review source and capture
evidence were checked separately at main 968c8397d262445feee5027f825e6994bbddcc02.

1. `scripts/readme-screenshot.sh:15–23, 103–105` still selects Mac capture when configured and uses local Linux as a
   development run. `docs/readme-hero/SPEC.md:83–98` describes it that way. The #4088 shared harness captured the staged
   hero on Linux at 2x (2560×1600), but the wrapper still uses the 1x Linux default. Decide whether to have the wrapper
   set the proven 2x Linux path and update the hero procedure together. The existing Mac default is intentional in the
   spec, so this is a workflow follow-up.
2. `scripts/docs-shots/launch-templates.py:21` captures the Templates button and New session. The #4088 capture report
   measured 1,423 bytes at 168×112. `scripts/publish-docs-shots.sh:92, 269` requires at least 2,000 bytes for a
   referenced shot. The button crop was unreferenced during verification, so publication impact is conditional on a page
   using it. Revisit the crop or the minimum for small verified shots before referencing it.
3. `scripts/docs-shots/launch-templates.py:11–23` frames the apply shot from Search and results. The captured box
   includes a sliver of the Folder field, in its warning state because the staged `~/src/farhelm` folder does not exist.
   Stage the folder under the scenario's private home and verify the resulting geometry. Retain the capture's tight
   focus on the template suggestions.

Confidence: #4088 reports observed 2x capture and crop measurements, with the warning sliver supplied in the follow-up
manifest. Verification here was by inspection of the capture report and current source. The maintainer classified these
as low priority.
