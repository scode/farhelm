# The portable changes page omits scenes that failed to render without warning

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

A portable screenshot report can omit failed scenes without telling its reviewer that the run was incomplete.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d crates/gui/farhelm-gui/src/goldens/mod.rs:1747-1772;
scripts/check-gui-goldens.py:159-167,309-345; scripts/golden_changes_page.py:14-31,96-119,126-172;
.agents/validation.md:722-734

A reviewer given the changes directory can see valid changes from completed scenes without any indication that other
selected scenes failed before producing a screenshot. The renderer records a failed capture under its scene and
appearance, skips the save via and_then, and continues rendering peers. The runner packages a page from the existing
files and prints its path even after a non-cancellation render failure or an incomplete full update. The HTML receives
no failure or coverage information and its only whole-page warning concerns missing references. The documented handoff
tells operators to give the reviewer the directory and intended design, without requiring the terminal output. Compare
must distinguish normal pixel differences, which also return nonzero, from scenes that could not be drawn. Update can
record rendering/completeness independently from optional review evidence. Preserve usable rows and include the failed
or missing scenes and the selection in the exported report. The runner itself still reports failure and a failed update
cannot certify captures. This is independent of absent or partial PNG contents in GR-08 and requires no cancellation
from GR-02. Introduced at 0d52ff68e92cd2bee5b4752d02c3a3210ffbdd5d.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `golden-report/GR-10`.
