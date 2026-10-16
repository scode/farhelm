# Fixing forward

What happens when a release attempt of any kind fails: the fix lands on main under the release request's authorization,
and the next attempt is cut from main with a settled base and version. It is on the stable release, RC release, dev
release and fixing forward and stable re-cuts reading lists in `releasing/AGENTS.md`. For a stable release, the next
attempt is cut per `releasing/.agents/stable.md`, "Re-cutting a stable release".

> NOTE: In an unattended run (plan executor, landing monitor, delegate), an instruction here to ask, confirm with, or
> propose to the maintainer follows `AGENTS.md`, "Unattended runs".

When a release attempt of any kind, stable, RC or dev, turns up a problem, the default is to fix forward without
stopping to ask: fix it on a branch off the latest main, land the fix on main, and cut the next attempt from main's new
tip. That covers every way an attempt fails: a pre-tag check (the version-parity tests, `dist plan`, the changelog
checks), the tag build's gate, the brick test, or a problem the maintainer hits trying an RC or dev release. Whatever
else landed on main in the meantime rides along. That is fine, and it is not a reason to cut from an older commit or to
ask first. The maintainer set this default for prereleases on 2026-10-10 and extended it to stable releases on
2026-10-11: release attempts exist to flush out exactly this kind of problem and to smoke test the real release
artifacts, so a fix that keeps the next attempt moving is the point, and waiting on the maintainer for one only stalls
them.

The release request is the authorization: open the fix as a PR, mark it ready and merge it as part of landing it, after
the validation the fix itself calls for (`AGENTS.md`, "Tests and validation", and `.agents/validation.md`). This covers
fixes that are reasonably straightforward and unambiguous. A fix that needs a design decision or a redesign, grows in
scope beyond the failure it fixes, changes product behavior in a way the specs do not already settle, or has more than
one reasonable answer is still the maintainer's: describe it and ask. An explicit instruction otherwise (cut the next
attempt from a stack, hold the fix for review, stop after a failure) overrides the default.

The next attempt's base is therefore settled (latest main), and so is its version, so neither needs asking about again:

- Once a tag has been pushed its name is spent, whether or not its build succeeded. An RC or dev release cuts `rc.N+1`
  or `dev.N+1`, and a stable release cuts the next patch version. The stale tag stays (tags are never deleted).
- A failure before any tag was pushed spends nothing, and the next attempt keeps the version.

Every attempt gets its own risk report (`releasing/.agents/risk-report.md`, "The release risk report"). For a stable
re-cut it still reaches the maintainer before the tag is pushed, but the agent does not wait for a reply to it.
