# Refreshing the docs

This file is the "refresh the docs" procedure: bringing every page up to date with what landed on main since the last
full refresh, and when the refresh marker moves. It belongs to the "Docs refresh" reading list in `website/AGENTS.md`.

> NOTE: In an unattended run (plan executor, landing monitor, delegate), an instruction here to ask, confirm with, or
> propose to the maintainer follows `AGENTS.md`, "Unattended runs".

"Refresh the docs" means bringing the whole site up to date with what has landed on main since the last full refresh.
`website/src/data/docs-refresh.json` records where that was: `refreshed_through` is the main commit the last full
refresh read up to, and `refreshed_at` is its date. Only a completed full refresh moves it. Fixing one page on request,
a screenshot refresh on its own, or a refresh that stopped partway leaves it alone, so the marker always means that
every change up to that commit has been looked at. Screenshots keep their own marker, `captured_from` in
`website/src/data/docs-shots.json`, which only a screenshot publish moves.

On a checkout of the latest main:

1. Read `refreshed_through` and list what landed since: `git log <refreshed_through>..origin/main`. Read the diffs of
   everything a reader could notice, not just the titles: `feat`, `fix`, `perf` and `style` commits, SPEC.md changes,
   CLI changes, and the changelog sections and fragments. Subagents split by commit range or by area do this reading
   well; check their headline claims against the code before acting on them.
2. Check every page under `website/src/content/docs/docs/` against those changes, and against the code wherever a page
   states behavior they touched. Fix what is now wrong directly: a false statement, a stale label or flow, a link whose
   target no longer says what the linking sentence claims, an existing passage that a change left incomplete.
3. Propose, rather than write, anything material and new: a new page or section, or a substantial new passage for a
   feature or change that warrants one. Show the maintainer what you would add and where, and add it only once they
   agree.
4. The Overview page and the README introduction it mirrors are checked like every page, but anything wrong there is
   proposed to the maintainer as an exact edit, per `website/EDITORIAL_RULES.md`, "The Overview page is the
   maintainer's". Release notes are left alone (see `website/AGENTS.md`, "Published release notes are frozen").
5. If anything that landed changes what a page's screenshots show, refresh them as part of this run: "Refresh the docs
   screenshots" in `docs/docs-shots/SPEC.md`, "Refreshing", which updates the shot specs, captures, and publishes. If
   nothing visible changed, say so and skip it.
6. Set `refreshed_through` to the main commit step 1 read up to (not whatever main has become since) and `refreshed_at`
   to today, in the same change as the page edits. Commits that landed on main during the refresh belong to the next
   one; say which they are. Check with `dprint check` and `bun run build`, hand over preview links as
   `website/AGENTS.md`, "Previews and reports", and `website/.agents/previewing.md` describe, and open the change as a
   draft PR per the `jjstack` skill.
