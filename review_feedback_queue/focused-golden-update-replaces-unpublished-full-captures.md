# A valid focused update replaces unpublished full-run captures

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

A valid focused golden update can overwrite a complete set of unpublished captures.

## Details

The supported preview configuration is --update with one or more valid --scene patterns in the same checkout as an
unpublished completed update. The runner documents this as inspection at scripts/check-gui-goldens.py:16–23 and
.agents/validation.md:728–731. At the pin, the update acquires the same capture-directory lock as a full run at
:221–226, validates every pattern at :131–141 and :233–235, and clears the shared capture directory and its completion
marker at :236–244 before rendering. Any explicit selection returns without certification at :268–270, even when it
selects the whole catalog. The publisher requires the marker at scripts/gui-goldens.py:255–288. A valid selected
comparison uses a private directory at scripts/check-gui-goldens.py:245–254 and leaves the staged update alone. A build
or inventory failure, or any unmatched pattern alongside valid patterns, returns before clearing captures, as fixed in
f2ff20c493f034fc1c5264163a7fb53009e0deaf and checked at scripts/test-check-gui-goldens.py:211–224. Give valid focused
updates their own inspection directory and preserve the publication directory for full updates.

Confidence: confirmed by inspection at the final pin.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `goldenRunner/l3`, `shots/l7`, `goldenRender/f1`.
