# Screenshots

This file covers the annotated screenshots on docs pages: how a page's shots are written, captured, checked and
published. `docs/docs-shots/SPEC.md` is the design and the authority. It belongs to the "Docs page authoring" (phase
file), "Docs refresh", "Release notes work" and "Screenshots" reading lists in `website/AGENTS.md`.

> NOTE: In an unattended run (plan executor, landing monitor, delegate), an instruction here to ask, confirm with, or
> propose to the maintainer follows `AGENTS.md`, "Unattended runs".

The editorial rules ask for annotated screenshots of the real UI wherever a page shows where to click. They are captured
from a staged fleet and published off main; `docs/docs-shots/SPEC.md` is the design and the authority on what may be
staged or rewritten. In short:

A page's shots are declared in `scripts/docs-shots/<page>.py`; see `docs/docs-shots/SPEC.md` for the page module
contract. On Linux, `scripts/docs-screenshots.sh --local --only <page>` runs them at 2x through the GPUI test harness.

A shot's annotations (red callouts with arrows, and highlight rings) are declared with it and drawn onto the picture by
the engine; see `docs/docs-shots/SPEC.md`, "Adding or changing a shot". For a full capture, run
`scripts/docs-screenshots.sh --local`; a docs refresh publishes it with `scripts/publish-docs-shots.sh`. Release shots
follow the release procedure.

Published release shots stay frozen. Only `scripts/publish-docs-shots.sh` may push `refs/docs-assets/keep`; never push
the ref by hand.
