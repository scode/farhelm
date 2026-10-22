## What this was about

Uninstall from the hosts panel refused an otherwise ordinary remote installation when SSH and the host's user service
manager named the same home folder through different paths. Resolving symlinks did not collapse a bind mount, so
uninstall concluded that the service came from somewhere other than the file it would remove.

The maintainer chose support for this one case, confined to uninstall. Inspection now recognizes an alternate name for
the login home, and accepts the rewritten path only when both full paths identify the same file. The existing unit,
binary and data-location checks then use that spelling. This also exposes a data directory hidden inside the removable
installation by the other home spelling, so uninstall refuses to remove that data. The specifications describe this
narrow exception and the code comments preserve its rationale.

## Things you should know

Shared unit folders, shared installations and hard-linked unit copies at a different location remain refused with the
existing messages. A missing or unresolvable home keeps the previous behavior. No other operation or path comparison
gains support for filesystem aliases, and the local installation was not changed or used for validation.

Tests run the production inspection script and ordinary uninstall planning against private fixture files. A symlink and
a controlled readlink substitute model two home spellings without collapsing them before the comparison. The accepted
case also runs the real file-removal scripts and verifies that the data remains. This is not an actual bind mount, SSH
connection or systemd uninstall. A missing file exercises rejection by the full-path identity guard; different nested
mounts beneath a non-recursive bind remain untested. Failing and empty home resolution are covered; unset and
root-valued homes are checked by inspection of the explicit fallback branches.

## Open questions and possible follow-ups

A privileged mount fixture could exercise the non-recursive bind topology directly. No decision is needed for this
delivery. Other path comparisons, including local setup, retain their previous scope.

## The PRs

[#2551: recognize aliased homes during host uninstall](https://github.com/scode/farhelm/pull/2551/changes), a draft fix
PR with the changelog fragment and covered TODO removal.

## Checks run, reused and skipped

- Ran `cargo nextest run -p farhelm-helm --lib -E 'test(provisioning::tests::uninstall)'` through the recorder on Linux
  with pinned nextest and tmux: 22 selected cases passed, including all seven new cases; 1,266 other cases were selected
  out. Run `43eda962-35c5-4a7b-9786-03339fff4b57` has complete evidence and no runtime skip messages. The existing
  private service/tmux cleanup test also exercised its SSH and user-manager substrate. The source was the reviewed tree
  committed as `c97635ea`; no code changed afterwards.
- Ran `cargo fmt --all -- --check`, the isolated test-delay checker (314 delays, zero missing rationales),
  `python3 releasing/check-changelog.py format`, and `dprint check` on the changed Markdown; all passed.
- Ran `cargo clippy --all-targets -- -D warnings`; passed on the same source tree.
- Reused no earlier session's checks. Skipped browser, desktop, installer and broad workspace execution because this
  change is confined to the remote uninstall inspection, with the relevant real-script and planning cases selected
  above. Actual mount-topology coverage is limited as described above.

## Review gate outcome

The required single fresh gpt-6.1-sol/high adversarial review passed with no actionable findings. It inspected the
normalization, identity guard, preserved refusal rules, specification changes and tests. No findings were rejected. The
commit-message cold read also passed.
