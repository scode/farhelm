# Docs website agent instructions

This directory is Farhelm's documentation site (Astro + Starlight). The pages live under `src/content/docs/docs/` and
render under `/docs/`. How to build it and which checks apply are in the root `AGENTS.md`. This file covers the site's
mechanics: links, the sidebar, stub pages, and formatting traps.

## Editorial rules

How the pages read (who they are for, which words they may use, what to link) is in
[EDITORIAL_RULES.md](EDITORIAL_RULES.md). Read it before drafting or revising any page. It is not a fixed style guide:
it grows from the maintainer's feedback on drafts, and its "Learning from feedback" section says how. Follow that
section whenever the maintainer comments on or edits wording you drafted for this site, including offering a rule when a
correction looks like it would apply beyond the sentence it was given on.

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
