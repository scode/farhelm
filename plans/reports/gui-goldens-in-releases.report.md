### What this was about

The desktop's screenshot tests compare reference images pixel for pixel. The maintainer kept those checks in development
and pre-merge landing validation only, for changes to the app's drawing code, bundled fonts, or GUI toolkit. The
validation guide now records that decision and the TODO entry is closed.

### Things you should know

Exact comparisons depend on hosted runners' Mesa and font versions, and fetching the reference images directly requires
access to a private repository. Those dependencies are why pixel comparisons stay out of CI and release jobs. Release
coverage comes from Linux and macOS desktop unit tests and a headless Wayland smoke test on Linux.

### Open questions and possible follow-ups

None.

### PRs

- Records when to run the desktop screenshot comparisons and removes the settled TODO entry.
  [#4692](https://github.com/scode/farhelm/pull/4692/changes)

### Checks run, reused and skipped

`dprint check .agents/validation.md TODO.md` and `python3 scripts/check-agents-md.py` passed. The instruction check
reported one existing warning about a heading cited in the triage archive. The change is Markdown only, so validation
was limited to documentation checks.

### Review gate

The requested independent source review passed after clarifying that private repository access is needed for a direct
fetch. A comparison can also use a local copy of the references. The PR text passed a separate cold read.
