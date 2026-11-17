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
contract. `scripts/docs-screenshots.sh --local --only <page>` runs them through the GPUI test harness. On Linux every
shot stops at the size check because the pictures need a 2x display.

The Mac capture path, annotations and publishing come with the first page that shows shots again, which the maintainer
decides. No page embeds a shot and nothing is published for now.

Published release shots stay frozen. Only `scripts/publish-docs-shots.sh` may push `refs/docs-assets/keep`; never push
the ref by hand.
