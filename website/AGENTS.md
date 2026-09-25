# Docs website agent instructions

This directory is Farhelm's documentation site (Astro + Starlight). The pages live under `src/content/docs/docs/` and
render under `/docs/`. How to build it and which checks apply are in the root `AGENTS.md`; this file is about writing
the pages.

NOTE: This is not reference documentation, and it is not a copy of the specs. `SPEC.md` and `SPEC_impl.md` stay the
authority on behavior. A page must agree with them, but it explains what the behavior means for the person using Farhelm
rather than restating the spec. Complete reference material (every flag, every field) is out of scope for now.

## Who the pages are for

Someone running Farhelm, not someone working on it. Aim for a good experience before completeness: accessible,
pragmatic, organized around what the reader is trying to do. Lead with the effect on the user and keep mechanism to the
minimum needed to use the thing well. Write in the second person and in plain words, following the maintainer's voice.

The site is meant to go live when the README's "you probably should not use this" notice comes down. Write for that
reader; do not carry the notice, or notes about the documentation's migration from `docs/`, into the pages.

## Every page is user facing

This rule is not negotiable. Every sentence on this site is written for a person using Farhelm, in the words that person
would use. The specs, the code, and the development history all have their own vocabulary, and it leaks into drafts
easily, because it is the vocabulary an agent reads while researching a page. Do not let it through.

Never use internal jargon unless the page cannot do its job without it. When a term really is unavoidable, do not assume
the reader knows it: say what it means in plain words where it first appears on the page, and link its canonical page if
it has one. Describe what the user sees and does ("the session's status dot turns yellow") rather than the mechanism
that causes it. Name things the way the UI names them, and when in doubt, write the sentence the way you would say it to
someone sitting next to you who has never read the source.

Before finishing a page, reread it as that reader and check every term against the vocabulary lists below.

## Vocabulary

This section is how the site learns its own vocabulary. It has two lists: jargon that should not appear on the site, and
terms that are settled as user-facing concepts. Keep it current as part of writing pages. When the maintainer's feedback
on a draft calls a term jargon, or approves one as fine for users, add it to the right list in the same change that
fixes the draft, with a short note: for jargon, what to say instead; for a user-facing term, how a page introduces it. A
term on neither list that sounds like it came from the specs or the code is jargon until decided; if a page needs it,
ask the maintainer and record the answer here.

### User-facing concepts

- **session**: one agent running in one working directory on one host, with its terminal. The central unit of the whole
  product.
- **host**: a machine that runs sessions, your Mac or a Linux machine.
- **helm**: the one program that shows you every host's sessions and serves the UI. Introduce it with a plain
  description and link [The pieces](/docs/how-it-works/the-pieces/).
- **supervisor**: the program on each host that keeps that host's sessions running. Introduce and link it the same way
  as the helm.
- **agent**: the coding agent a session runs (Claude, Codex, and so on).
- **status**: what the session list says an agent is doing (working, waiting on you, done, crashed).

### Jargon

- **harness**: say "agent", or name the agent.
- **provisioning**: say "setting up Farhelm on the host" or "adding a host".
- **registry**, **host registry**: say "your list of hosts".
- **install identity**, **registry row**: rephrase around the host the user added; readers never need these.
- **launch composer**, **structured launch**: say "the launch dialog" or "starting a session".
- **agent kind**: rephrase as "which agent this is" until the maintainer settles a UI name.
- **conversation identity**, **conversation capture**, **conversation reporter**: say what the user gets ("Resume opens
  the conversation you were in").
- **hook injection**: describe the effect, not the mechanism.
- **pane**, **tmux session**, **socket**: say "the session's terminal"; mention tmux only where the reader has to
  install or configure it.

## Cross-reference liberally

Every fact has one canonical page, and every other page that touches it links there instead of restating it. What
survives a restart, for example, lives in `how-it-works/what-survives-what.md`; a guide that mentions a restart links to
it rather than paraphrasing it. Link a concept on its first mention in a section whenever it has a page of its own, and
end a page by pointing to where the reader goes next. When in doubt, add the link: a redundant link costs a reader
nothing, a missing one sends them searching.

Write internal links as absolute site paths with a trailing slash, the form the existing pages use:
`[Manage hosts](/docs/using/manage-hosts/)`, or with a heading anchor, `/docs/agents/grok/#launching`. Starlight does
not check links, so after moving or renaming a page, build the site and check that every `/docs/` link in the output
resolves, anchors included.

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
