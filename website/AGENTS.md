# Docs website agent instructions

This directory is Farhelm's documentation site (Astro + Starlight). The pages live under `src/content/docs/docs/` and
render under `/docs/`. How to build it and which checks apply are in the root `AGENTS.md`. This file covers the site's
mechanics: previewing drafts, screenshots, links, the sidebar, stub pages, and formatting traps.

## Editorial rules

NOTE: Reading [EDITORIAL_RULES.md](EDITORIAL_RULES.md) in full is a hard requirement before you make ANY change under
`website/`: a page, a stub, the sidebar, styles, site configuration, a script, or these instruction files. It is not
optional, and there is no exception for a small edit, a link fix, or a change made in passing while working on something
else. Read it again after a context compaction rather than working from a summary of it.

That file holds how the pages read (who they are for, which words they may use, what to link) and which pages agents do
not edit on their own initiative. It is not a fixed style guide: it grows from the maintainer's feedback on drafts, and
its "Learning from feedback" section says how. Follow that section whenever the maintainer comments on or edits wording
you drafted for this site, including offering a rule when a correction looks like it would apply beyond the sentence it
was given on.

## Previewing drafts

The maintainer reviews drafts in a browser, served from this checkout by `website/scripts/preview.sh` at
`http://127.0.0.1:14000/`. This local server is not a Vercel preview deployment (see "Vercel deployments" in the root
`AGENTS.md`), and it is not `bun run preview` either. The port is fixed so that the maintainer can forward it once; the
script's header says why, and what that costs. Use the script rather than `bun run dev` or `astro dev` directly, which
pick their own port.

When the main thing you were asked to do is change the docs website (writing or revising pages, their screenshots, the
site's structure or look), this flow is the default: start the preview and hand over the links every time you report the
change, without being asked, and again when you open or update its PR. It does not apply when a docs edit rides along in
a change whose point is elsewhere, such as a code fix that also corrects a sentence on a page; name the page in your
report there instead. An unattended run (a plan being executed, a delegate, a reviewer in a scratch copy) starts no
preview server either: nobody is there to read it, and starting one would take the port from the maintainer's own
drafting session.

- Run `website/scripts/preview.sh` each time before you hand the maintainer a change. It starts the server if this
  checkout is not already serving, waits until it answers, and prints the docs front page URL; when the server is
  already up it only prints the URL. Running it every time rather than once per session matters because the server does
  not outlive the agent session that started it, and because its exit status is what tells you the links are good. The
  server picks up every saved edit by itself, new pages and sidebar changes included, so a browser reload shows them. A
  dependency change (`package.json` or `bun.lock`) is the exception: run `website/scripts/preview.sh stop`, then start
  it again.
- Every time you report a change under `website/`, give the maintainer a direct link to where they can see it: the URL
  of each page you changed (`website/scripts/preview.sh url <file>` prints it), with the heading anchor when the change
  sits in one section. When the change has no single page (the sidebar, styles, site configuration, a sweep across many
  pages, or a change to these instruction files), or when in doubt, link the docs front page,
  `http://127.0.0.1:14000/docs/`. The links assume the maintainer's browser reaches 127.0.0.1:14000 on this machine,
  either because it runs here or through their port forward; give them as they are.
- When another checkout's preview server holds the port, the script stops it and takes the port without asking: the
  maintainer drafts in one session at a time, so that server shows a draft nobody is reading. It never stops a
  foreground `astro dev` or anything that is not a preview server. If the script refuses because something like that
  holds the port, hand out no preview links (they would show someone else's pages, not your draft) and tell the
  maintainer what the script reported.
- Leave your own server running when you finish, since the maintainer may still be reading;
  `website/scripts/preview.sh stop` stops it when asked.

The preview does not check links. `bun run build` does (see below), and a page that renders fine in the preview can
still fail the build.

## Screenshots

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

## Release notes

The Release notes page (`/docs/release-notes/`, the sidebar entry under Overview) is generated from the repository's
`CHANGELOG.md` by `scripts/release-notes.mjs` on every build and preview start, and is gitignored. Do not edit the page;
a change to the notes themselves is a change to `CHANGELOG.md`, which `releasing/AGENTS.md` governs. Each release has a
stable anchor named after its version, such as `/docs/release-notes/#v0.23.0`.

An entry can show screenshots of what it changed, behind a closed **show screenshot** link under the entry's text.
`src/release-notes/<version>.json` (for example `src/release-notes/v0.25.0.json`) lists them, naming each entry by a
pull request number the entry cites; the generator places them, and fails the build when no entry of that release cites
the number. `CHANGELOG.md` and the GitHub release stay text only. The screenshots are ordinary docs shots: tests in
`e2e/docs-shots/release-notes.spec.ts` named `release-notes/v<X-Y-Z>-<what>`, captured and published with every other
shot until their first publish, after which they are frozen (below). Give a screenshot only to an entry whose change is
visible: a new dialog or control, or a redesign, not a fix or behaviour a picture cannot show. Most entries get none,
and a release with no visible change gets no file.

NOTE: Release notes are stable once published. The `CHANGELOG.md` sections of published releases, the generated page,
`src/release-notes/*.json`, and the release shots with their tests are never part of a change by default: not a
screenshot refresh, not a sweep across pages, not a rename or a wording cleanup, and not a UI change that makes an old
shot look dated. They change only when a task specifically targets them, such as a correction to a published release.
The tooling holds the screenshots to this (`docs/docs-shots/SPEC.md`, "Release shots are frozen"): once a release shot
is published, a capture skips it and the publish carries it over unchanged, so its test may stop matching the current UI
and that is fine. A correction recaptures it with `scripts/docs-screenshots.sh --recapture-release-shot <name>`.

## Refreshing the docs

"Refresh the docs" means bringing the whole site up to date with what has landed on main since the last full refresh.
`src/data/docs-refresh.json` records where that was: `refreshed_through` is the main commit the last full refresh read
up to, and `refreshed_at` is its date. Only a completed full refresh moves it. Fixing one page on request, a screenshot
refresh on its own, or a refresh that stopped partway leaves it alone, so the marker always means that every change up
to that commit has been looked at. Screenshots keep their own marker, `captured_from` in `src/data/docs-shots.json`,
which only a screenshot publish moves.

On a checkout of the latest main:

1. Read `refreshed_through` and list what landed since: `git log <refreshed_through>..origin/main`. Read the diffs of
   everything a reader could notice, not just the titles: `feat`, `fix`, `perf` and `style` commits, SPEC.md changes,
   CLI changes, and the changelog sections and fragments. Subagents split by commit range or by area do this reading
   well; check their headline claims against the code before acting on them.
2. Check every page under `src/content/docs/docs/` against those changes, and against the code wherever a page states
   behavior they touched. Fix what is now wrong directly: a false statement, a stale label or flow, a link whose target
   no longer says what the linking sentence claims, an existing passage that a change left incomplete.
3. Propose, rather than write, anything material and new: a new page or section, or a substantial new passage for a
   feature or change that warrants one. Show the maintainer what you would add and where, and add it only once they
   agree.
4. The Overview page and the README introduction it mirrors are checked like every page, but anything wrong there is
   proposed to the maintainer as an exact edit, per "The Overview page is the maintainer's" in
   [EDITORIAL_RULES.md](EDITORIAL_RULES.md). Release notes are left alone (see "Release notes" above).
5. If anything that landed changes what a page's screenshots show, refresh them as part of this run: "Refresh the docs
   screenshots" in `docs/docs-shots/SPEC.md`, "Refreshing", which updates the shot specs, captures, and publishes. If
   nothing visible changed, say so and skip it.
6. Set `refreshed_through` to the main commit step 1 read up to (not whatever main has become since) and `refreshed_at`
   to today, in the same change as the page edits. Commits that landed on main during the refresh belong to the next
   one; say which they are. Check with `dprint check` and `bun run build`, hand over preview links as "Previewing
   drafts" above describes, and open the change as a draft PR per the `jjstack` skill.

## Internal links

Write internal links as absolute site paths with a trailing slash: `[Manage hosts](/docs/using/manage-hosts/)`, or with
a heading anchor, `/docs/agents/grok/#launching`. The build checks every internal link and anchor
(`starlight-links-validator`) and fails on a broken one, on a relative link, and on an internal link written as a full
`https://farhelm.io` URL, so `bun run build` is the check to run after moving a page or renaming a heading. It only
proves a link resolves; whether the linking sentence still matches what the target page says is yours to check.

## The outline is the sidebar

The sidebar in `astro.config.mjs` is the documentation's outline, in reading order: Overview, Get started (a guided
first run, one step per page), Using Farhelm (task guides), Agents, and How it works (the mental model, without
internals). Each group is generated from its directory, and pages order themselves with `sidebar.order` in their
frontmatter, so adding a page only means creating the file.

Pages that are planned but not written are stubs, so that the shape of the site is visible and other pages can link to
them already. A stub carries a `Stub` badge in its sidebar entry (frontmatter `sidebar.badge`, with `variant: caution`)
and opens with a `:::note[Stub]` aside, followed by a short description of what the page will cover and which pages it
will link to. When you write a stub's content, remove both the badge and the aside in the same change. A new planned
page starts as a stub in the same way.

Two formatting traps. Keep blank lines inside an aside (`:::note[Stub]`, a blank line, the text, a blank line, `:::`):
without them `dprint fmt` joins the three lines into one paragraph and the aside stops rendering. And quote a
frontmatter `title` or `description` that contains `": "`, or the YAML parse fails the build.
