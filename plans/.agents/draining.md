# Draining

How a drain runs plan after plan in one checkout. Drainers read it before step 1 of `plans/.agents/executing.md`,
"Executing one plan", together with the executor's files. The landing monitor reads it too, because
`plans/.agents/watching.md`, "Drain and keep monitoring", which the monitor reuses, builds on it.

## Draining: "drain the plans"

Start with step 1 of `plans/.agents/executing.md`, "Executing one plan", which settles any claim or close that an
earlier run of this executor left unfinished; a dirty working copy that belongs to such a claim is resumed or reset
there, not reported. Then get onto a clean, freshly fetched `main` with the clean-main boundary's starting check, even
when the working copy already looks clean: an older `main` is not good enough. If that check finds a dirty working copy,
abort the drain and tell the maintainer which paths are dirty. Then execute one plan after another, until a pick takes
nothing. Each plan's close resets the working copy to a clean, fresh `main` (`plans/.agents/executing.md`, "Executing
one plan", step 12), so the next pick always starts there. Every "drain again" under monitoring
(`plans/.agents/watching.md`, "Drain and keep monitoring") starts with the same starting check. Finish by reporting what
was delivered and what blocked, with PR links, restating any open questions the way
`plans/.agents/maintainer-writing.md`, "Writing for the maintainer", requires.

Any number of executors may drain at once, each in its own checkout; claims keep them off each other's plans.
