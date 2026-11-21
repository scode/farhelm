# Correct the takeover hint on agent host-add requests

## What this was about

When an agent asked to add a host that another Farhelm managed, the refusal pointed to Take over in Hosts. The failed
add had never created a row there. Approving the agent's request still ends in a refusal. When the host reports an
install identity, the refusal now uses the wording you chose, “Another Farhelm (`<tag>`) manages this host. Add it
yourself with Add host to take it over.” Opening Add host yourself offers a separate takeover confirmation.

## Things you should know

If a new host reports who owns it without reporting its install identity, Farhelm explains why takeover is unavailable.
This applies whether you start Add host yourself or approve an agent's request. For a host already in Hosts, the refusal
continues to point to Take over on that row.

## Open questions and possible follow-ups

None.

## Pull requests

- Give a usable takeover hint when Add host finds another Farhelm owns the host.
  [#4354](https://github.com/scode/farhelm/pull/4354/changes)

## Checks

- A filtered Linux `cargo nextest run -p farhelm-helm --lib` passed all four selected ownership tests. They cover the
  new agent hint, missing identity through both add routes, one-use takeover confirmation, and Setup, Update and
  uninstall refusals for listed hosts. Recorder `ceb424d2-1a17-466a-b270-cca20126f853` ran against `612f2027` with the
  plan's changes. The rebase to `d7879ae7` added a queue claim on main and the tested patch stayed byte-identical.
- `cargo fmt --all -- --check`, `dprint check` on the changed Markdown, `python3 releasing/check-changelog.py format`
  and `python -B scripts/check-test-sleeps.py` passed.
- Workspace tests, live SSH and desktop tests were skipped. The changed behavior is the probe's refusal text, covered
  with scripted ownership responses and the actual registration and confirmation logic.

## Review

Independent source review found no issues. It verified which refusal each request reaches and traced the text to the
desktop's existing error display. The commit wording passed a separate cold read.
