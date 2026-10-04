# Editorial rules for the docs website

How the pages on this site should read: who they are written for, which words they may use, and how they hand the reader
from one page to the next. Reading this file is required before any change under `website/`; `AGENTS.md` says so too.
The site's mechanics (link syntax, the sidebar, stub pages, formatting traps) are in `AGENTS.md` beside this file.

The goal is documentation that agents can mostly write and that still reads as if someone who cared wrote it. That only
works if the maintainer's corrections stick, so this file is not a finished style guide. It grows from feedback on
drafts, and [Learning from feedback](#learning-from-feedback) at the end says how. Follow that section whenever the
maintainer comments on, or edits, wording you drafted for this site.

## What the site is not

NOTE: This is not reference documentation, and it is not a copy of the specs. `SPEC.md` and `SPEC_impl.md` stay the
authority on behavior. A page must agree with them, but it explains what the behavior means for the person using Farhelm
rather than restating the spec. Complete reference material (every flag, every field) is out of scope for now.

## The Overview page is the maintainer's

The Overview page is the docs front page at `/docs/`, the sidebar's "Overview" entry, rendered from
`src/content/docs/docs/index.mdx`. The root `AGENTS.md` and the README scripts call it "the landing page". Agents edit
it only when the maintainer asks for that specific edit, in conversation or in a TODO entry or plan of theirs that names
the Overview page. The maintainer edits its wording by hand, sentence by sentence, and an agent improving it on its own
initiative undoes that work.

That covers everything the page shows, including the words in its drawings: the pillar titles and the how-it-works text
are written in `scripts/render-svgs.mjs` and drawn into the SVGs under `src/assets/intro/`. Re-running that script after
a requested change to the brand mark is fine as long as the Overview's words stay as they are.

It holds even when another change seems to require touching the page: a link to a page you renamed or moved, prose its
own comment says to keep in step with the README's introduction, a term that changed in the vocabulary lists, or a stub
it points at that you just wrote. Do not edit it. Tell the maintainer what would need to change, show the exact edit you
propose, and leave the decision to them. When the rest of your change cannot work without that edit (the build fails on
a link the Overview page holds, for example), the change waits for the maintainer's answer; do not land it broken, and
do not route around the rule by keeping the old page alive under its old name. A request to rewrite "the docs" or "every
page" does not cover the Overview page either; ask.

One edit is exempt: "Refresh the README screenshot" in the root `AGENTS.md` has `scripts/publish-readme-hero.sh` rewrite
the screenshot URL between the markers in this page, and committing that one-line change is part of the refresh.

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
it has one. Describe what the user sees and does ("the session's status changes to waiting") rather than the mechanism
that causes it. Name things the way the UI names them, and when in doubt, write the sentence the way you would say it to
someone sitting next to you who has never read the source.

Before finishing a page, reread it as that reader and check every term against the vocabulary lists below.

## Vocabulary

This section is how the site learns its own vocabulary. It has two lists: jargon that should not appear on the site, and
terms that are settled as user-facing concepts. Each entry carries a short note: for jargon, what to say instead; for a
user-facing term, how a page introduces it. A term on neither list that sounds like it came from the specs or the code
is jargon until decided; if a page needs it, ask the maintainer and record the answer here. Terms the maintainer rules
on while reviewing a draft are recorded the same way, per [Learning from feedback](#learning-from-feedback).

### User-facing concepts

- **session**: one agent running in one working directory on one host, with its terminal. The central unit of the whole
  product.
- **host**: a machine that runs sessions, your Mac or a Linux machine.
- **helm**: the one program that shows you every host's sessions and serves the UI. Introduce it with a plain
  description and link [The pieces](/docs/how-it-works/the-pieces/).
- **supervisor**: the program on each host that keeps that host's sessions running. Introduce and link it the same way
  as the helm.
- **agent**: the coding agent a session runs (Claude, Codex, and so on).
- **harness**: the agent program a session runs, as the UI names it ("choose a harness"). Use it where the reader is
  looking at those UI labels; plain prose can say "agent".
- **session launcher**: what you start a session from. Use this name, not "launch dialog" or "launch composer".
- **status**: what the session list says an agent is doing: running, waiting (it needs you), idle, exited, interrupted,
  or error. Use these words, the ones the UI shows.
- **directory**: a file system directory, in prose, callouts, and alt text alike; never "folder". An exact UI label that
  says folder (**browse folders on**, **use existing folder**, the `folder:` search prefix) is still quoted the way the
  UI shows it. (Stated by the maintainer, 2026-10-04.)

### Jargon

- **provisioning**: say "setting up Farhelm on the host" or "adding a host".
- **registry**, **host registry**: say "your list of hosts".
- **install identity**, **registry row**: rephrase around the host the user added; readers never need these.
- **launch composer**, **structured launch**: say "the session launcher" or "starting a session".
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
nothing, a missing one sends them searching. How to write the link is in `AGENTS.md`.

## Rules from feedback

Rules the maintainer has stated or approved while reviewing drafts, recorded per
[Learning from feedback](#learning-from-feedback). They bind as much as the sections above. Each one gives the guidance
and the example that prompted it, with the date.

- Show the UI with real screenshots, annotated with red arrows and callouts, instead of narrating it in prose. The
  screenshots are captured from the real UI and can be refreshed against the latest main, the way the README screenshot
  is; they are never drawn or edited by hand. (Prompted by the first draft of
  [Start a session](/docs/using/start-a-session/), which walked through the session launcher's buttons in words only,
  2026-10-03.)
- Tell as much as possible with the annotated screenshot alone, starting with what matters most; where possible it
  stands on its own. A callout may take a sentence or two to explain what a control does, not just point at it: the
  permissions callout says what default and yolo each mean, not only "No approval prompts". The text then fills in what
  the image does not carry: things outside it, and detail about something it calls out that is too much to keep in a
  callout. Assume the reader can see the screenshots. Do not spend a run of sentences or bullets on what the screenshot
  already shows; naming a control in the text is fine where plain readability calls for it, as a judgement call rather
  than a ban. (Prompted by the second draft of Start a session, whose harness screenshot had terse callouts and was
  followed by bullets restating the controls it showed, 2026-10-03.)

- When a screenshot demonstrates something, use it the way a person actually would. Show the common case first, and
  demonstrate an option (a prefix, a flag) with a case that needs it, not one where it is redundant. (Prompted by the
  search shot on Start a session, which typed `harness:` when plain `claude` finds the same thing; it became two shots,
  `claude` and `name: fix login bug`, 2026-10-03.)

## Learning from feedback

This applies whenever the maintainer reviews wording an agent drafted for this site, interactively or in a later
session: a comment in conversation, a rewrite they dictate, or an edit they make to the page themselves (diff the page
against your draft to see what they changed; their edits are feedback even when they say nothing about them).

First apply the feedback to the draft. Apply exactly what was asked; a correction to one sentence is not permission to
rework the paragraph around it.

Then ask whether the correction would apply beyond the sentence it was given on. The useful signals are a word called
jargon or approved for users, a shape of sentence the maintainer rewrites, a kind of detail they cut or add, an ordering
they change, or a tone they push back on. A correction that only fixes one fact on one page is applied and not recorded.
A correction an existing rule already covers means that rule was missed or is unclear; say which, and offer to sharpen
it if it is unclear.

A rule gets into this file two ways, and only two:

- The maintainer states it as a rule in so many words ("X is jargon, say Y", "never open a page with a definition").
  Record it.
- The agent sees a generalization in the feedback. OFFER it: at the end of the reply that applies the correction, quote
  the exact rule you would add and where it would go, and record it only on a yes. Never record an inferred rule on your
  own judgment. This file is the maintainer's opinions, and a guessed one would steer every future draft.

Offer when the generalization looks useful, even if you are not sure; a declined offer costs one line. Do not hold
offers back for a batch at the end of the session: by then the example that prompted it is cold.

Recording a rule:

- Terms go in the [Vocabulary](#vocabulary) lists with their note. Everything else goes in
  [Rules from feedback](#rules-from-feedback) as one bullet: the guidance first, then the example that prompted it
  (before and after, when the maintainer rewrote a sentence), then the date.
- When a new rule refines or contradicts an existing one, rewrite or remove the existing rule rather than adding a
  second one beside it, and say in the offer that you are doing so.
- Record the rule in the same change as the page edit that prompted it, so the two land together.
- If the rule would also apply to release notes, say so when offering it. `releasing/EDITORIAL_GUIDANCE.md` is the
  equivalent file for changelog wording, and the maintainer decides whether the rule goes there too.
