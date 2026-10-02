# Docs website agent instructions

This directory is Farhelm's documentation site (Astro + Starlight). The pages live under `src/content/docs/docs/` and
render under `/docs/`. How to build it and which checks apply are in the root `AGENTS.md`. This file covers the site's
mechanics: previewing drafts, links, the sidebar, stub pages, and formatting traps.

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

This flow is for sessions where the maintainer is reviewing drafts with you. An unattended run (a plan being executed, a
delegate, a reviewer in a scratch copy) starts no preview server: nobody is there to read it, and a server left behind
by a scratch copy holds the port for every later drafting session.

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
- If the script refuses because something else holds the port, hand out no preview links: they would show someone else's
  pages, not your draft. When the script says the holder's checkout no longer exists, run
  `website/scripts/preview.sh takeover` to clear it; that server is no one's. Otherwise it is usually another checkout's
  preview server, which belongs to whoever started it: tell the maintainer what the script reported and ask, and run
  `takeover` only when they say to. It stops only another checkout's background preview server, never a foreground
  `astro dev` or anything else, and then starts this one.
- Leave your own server running when you finish, since the maintainer may still be reading;
  `website/scripts/preview.sh stop` stops it when asked.

The preview does not check links. `bun run build` does (see below), and a page that renders fine in the preview can
still fail the build.

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
