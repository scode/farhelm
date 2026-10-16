# Docs media: brand marks, README hero, demo video, docs screenshots and refresh

The root `AGENTS.md` sections on the brand marks, the README hero screenshot, the README demo video, the docs refresh
and the docs screenshots, moved here with their references rewritten. Read it before adding or changing any presentation
of Farhelm's icon or name as a mark, before touching any of the paths `AGENTS.md`, "Instructions by path" sends here or
the brand-mark sources under `website/` (`website/src/site-mark-dark.svg` and `-light.svg`,
`website/scripts/render-svgs.mjs`, `outline-wordmark.py`, `wordmark-outline.json`), and when asked to refresh the README
screenshot, the demo video, the docs screenshots or the docs. For the docs screenshots, also read the "Screenshots"
reading list in `website/AGENTS.md`.

> NOTE: In an unattended run (plan executor, landing monitor, delegate), an instruction here to ask, confirm with, or
> propose to the maintainer follows `AGENTS.md`, "Unattended runs".

## Brand marks: the icon and the wordmark

The app icon is `packaging/farhelm-desktop/icon.svg` (the source of `icon.png` and `Farhelm.icns` beside it). The
wordmark, "farhelm" in JetBrains Mono Nerd Font Bold with the icon's block cursor after it, is
`packaging/farhelm-desktop/wordmark-dark.svg` and `wordmark-light.svg`, one per ground. Its letters are outlines pulled
from the font the UI crate vendors by `website/scripts/outline-wordmark.py` into
`website/scripts/wordmark-outline.json`, and `website/scripts/render-svgs.mjs` inlines them into the wordmark files, the
introduction's header, pillars, and how-it-works blocks under `website/src/assets/intro/` (shown by both the docs
landing page and the README), and the docs website's header mark (`website/src/site-mark-dark.svg` and `-light.svg`).
Anything that needs the name as a mark uses those files; anything that changes the mark changes the script and re-runs
it, never the SVGs by hand, and the outline step only reruns when the word or the font changes.

## README hero screenshot

The image at the top of README.md, also shown on the docs website's landing page, is a real capture of the web UI
against a staged fleet, governed by `docs/readme-hero/SPEC.md`. "Refresh the README screenshot" means exactly: run
`scripts/readme-screenshot.sh`, look at the PNG it prints, run `scripts/publish-readme-hero.sh` on it, and commit the
one-line URL change it leaves behind in README.md and in the landing page. The design (`docs/readme-hero/scenario.json5`
and the transcripts beside it) changes only when the maintainer asks for a different picture. The publish script is the
only thing that pushes the `readme-assets` branch; never run that push by hand, and never commit the PNG on main.
Neither script is a gate: nothing in CI runs or checks the image. Changing the publish script means running its
`--self-test`, which is its whole validation and needs no network.

## README demo video

The demo video is recorded from the web UI against a staged fleet, governed by `docs/readme-video/SPEC.md`; read it
before touching anything under `docs/readme-video/` or `e2e/readme-video/`. "Refresh the demo video" means: run
`scripts/readme-video.sh`, and if a beat no longer works on the current UI, rewrite that beat in
`e2e/readme-video/capture.spec.ts` to realise the same intent, then look at every still it prints and fix annotation
placement until each one points at the right thing and hides nothing the viewer should see.
`docs/readme-video/intent.md`, the scenario, and the transcripts are the maintainer's design and change only when asked.
While `intent.md` opens with its THROWAWAY SAMPLE note, the video only demonstrates the machinery; say so when handing
it over. The upload is manual; nothing in CI records or checks the video.

## Docs refresh

"Refresh the docs" means bringing the whole docs website up to date with everything that landed on main since the last
full refresh: correcting what is now wrong, proposing new sections or material new text for the maintainer to confirm,
and refreshing any screenshot that may be stale. `website/.agents/refreshing.md`, "Refreshing the docs", is the
procedure, and `website/src/data/docs-refresh.json` records the main commit the last full refresh read up to. Only a
completed full refresh moves that marker; a single-page fix or a screenshot refresh on its own does not.

## Docs screenshots

The docs website's annotated screenshots are captured from the web UI against a staged fleet and published off main,
governed by `docs/docs-shots/SPEC.md`; read it before touching anything under `docs/docs-shots/` or `e2e/docs-shots/`.
Screenshots are regenerated in bulk, every page at once from one capture, and the manifest
(`website/src/data/docs-shots.json`) records which main commit that capture ran on as `captured_from`. "Refresh the docs
screenshots" means the procedure in the SPEC's "Refreshing" section: on the latest main, first read the UI changes since
`captured_from` and update the shot specs and pages they affect, then run `scripts/docs-screenshots.sh`, rewrite any
shot that no longer works on the current UI so it shows the same thing, look at every image it prints and fix annotation
placement until each points at the right thing and hides nothing the reader needs, then run
`scripts/publish-docs-shots.sh` and commit the manifest change it leaves. Release notes are the exception: published
releases' `CHANGELOG.md` sections, `website/src/release-notes/*.json`, and the release shots and their tests are never
touched by a refresh, a sweep, or a cleanup, only by a task that targets them, and the scripts carry published release
shots over unchanged (`website/.agents/release-notes.md`, "Release notes"). The publish script is the only thing that
pushes `refs/docs-assets/keep`; never push that ref by hand, and never commit a screenshot on main. Neither script is a
gate: nothing in CI runs or checks the images. Changing the publish script means running its `--self-test`, which is its
whole validation and needs no network.
