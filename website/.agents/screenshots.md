# Screenshots

This file covers the annotated screenshots on docs pages: how a page's shots are written, captured, checked and
published. `docs/docs-shots/SPEC.md` is the design and the authority. It belongs to the "Docs page authoring" (phase
file), "Docs refresh", "Release notes work" and "Screenshots" reading lists in `website/AGENTS.md`.

> NOTE: In an unattended run (plan executor, landing monitor, delegate), an instruction here to ask, confirm with, or
> propose to the maintainer follows `AGENTS.md`, "Unattended runs".

The editorial rules ask for annotated screenshots of the real UI wherever a page shows where to click. They are captured
from a staged fleet and published off main; `docs/docs-shots/SPEC.md` is the design and the authority on what may be
staged or rewritten. In short:

- A page's shots are tests in `e2e/docs-shots/<page>.spec.ts`, one per shot, named `<page>/<shot>`. A shot drives the UI
  to the state it shows (never launching a session or changing a setting), draws red callouts and rings with the docs
  overlay theme, and crops to the regions that matter plus every annotation. The annotation wording is part of the page
  and carries as much of it as the picture can, per the editorial rules: a sentence or two per callout where that
  explains the control, in the rules' vocabulary.
- `scripts/docs-screenshots.sh --only <page>` captures them into the gitignored `website/public/docs-shots-local/`, and
  the preview shows them at once, labelled as a local capture. Look at every image before handing it over: each arrow
  points at what its label says, no label covers something the reader needs, and nothing from this machine shows.
  Callouts do not keep clear of each other or of the viewport's edge on their own: move one with its placement (`side`,
  `dx`, `dy`) rather than rewording it to fit, keep each shot under about 930 CSS pixels wide (the docs' text column)
  with `maxWidth` or by splitting it, and check that every arrow is visible.
- A page that shows shots is `.mdx` and uses `<Screenshot name="<page>/<shot>" alt="…" />`. The alt text says what the
  shot points at, for readers who cannot see it.
- Publishing regenerates every page's shots at once. When the maintainer is happy with the page, run
  `scripts/docs-screenshots.sh` with no `--only` on a checkout based on the latest main, look at every image (other
  pages' shots too, since the UI may have moved), then run `scripts/publish-docs-shots.sh`; it refuses a one-page
  capture. The manifest change it leaves in `website/src/data/docs-shots.json` goes in the same change as the page. A
  production build (`bun run build`) fails on a shot that is not published, so publish before landing. Never push the
  screenshot ref by hand.
