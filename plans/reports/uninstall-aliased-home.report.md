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

### Landing

Landed as #2551, with the same content the report describes. It was the only plan in this landing round.

#### What landed since the stack was based, and whether it interacted

After the stack's base, main gained only queue bookkeeping and #2513, which records review findings in the review queue
and changes no code, so the plan landed without changes. A fresh-context reviewer and I read it against the helm and
uninstall work that landed in the preceding rounds:

- The uninstall refusals reworded a few rounds earlier (uninstall-refusal-wording) keep their order and text. The
  refusal for a symlinked installation is still checked first and still offers Remove; its message may now name the
  link's target through the other home spelling, which is the same file.
- Uninstall still removes exactly the planned unit, installation directory and terminal socket; the new home handling
  only changes what the safety checks compare.
- Host uninstall never touches checkout folders, so the prepared spare checkouts kept there (spare-launch-checkout) are
  unaffected.
- The inspection is a shell script the helm runs over SSH, with unchanged output, so a host running an older Farhelm is
  inspected the same way. A shell that cannot compare files falls back to the old behavior and the old refusal.

#### Fixes made during the landing

None.

#### Possible follow-ups

- Found at landing, low severity, and the mirror image of a case the report covers: the report's new refusal catches
  data hidden inside the installation under the other home spelling. The opposite arrangement now slips through. When
  the state directory is spelled through a bind mount of the home placed inside Farhelm's own installation directory,
  the new home handling resolves it to its home spelling, so uninstall no longer sees that the data lies inside the
  installation. Before this plan it refused; now the confirmed removal of the installation directory would delete the
  home through that mount. Nobody plausibly mounts a home inside Farhelm's installation, and that layout was already
  unsupported. A comment in the uninstall planning code that says removing the installation cannot remove that data is
  too strong for this case. Possible follow-up: also check the state directory's unrewritten path, refuse when either
  spelling is inside the installation, and correct that comment.
- Informational: the new check proves only that the two paths name the same file, not the same directory entry. With a
  home bind mount that does not include nested mounts, plus a second mount nested below the home on the same filesystem,
  plus a hard-linked copy of the service unit at the matching place, a loadable unit copy could survive uninstall. That
  needs all three at once; no action suggested.

#### Checks

The plan changes about 630 lines: its behavior change is in the helm's remote uninstall inspection script, with comments
in uninstall planning, tests and specification text. This round ran the whole helm crate, which includes the plan's
tests that run the real inspection script against fixture files, and the browser provisioning spec. That spec checks the
uninstall flow and its dialogs around the change; it uses the helm's test backend, which simulates the inspection, so it
does not exercise the new home handling itself. All of the following ran on the stack as it landed:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, the isolated test-sleep check, `dprint check` on the changed Markdown
  files, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`, the workspace debug build
  and the UI's release web build: all passed.
- `cargo nextest run -p farhelm-helm` through the test recorder with pinned nextest and tmux, four slots, no retries,
  run `34d8f3eb-6bba-43b1-991a-89903043faf0`: 1288 passed, none failed, no runtime `SKIPPED` message.
- `npx playwright test` in `e2e/` for the `provisioning` spec on Chromium and WebKit, three workers each with its own
  helm and supervisors, no retries, run `75df8763-718c-4845-997e-ce6026460bde` (kept): 161 passed, 1 failed. On
  Chromium, "a wrong-host UPDATE acceptance warns on its source row and claims nothing", a host update test that does
  not involve uninstall, failed inside the test's own network interception ("Response has been disposed"). Ten repeats
  on Chromium (run `183651b1-1d26-4ef9-8989-99b84a719e32`) and five on WebKit (run
  `f5b2a78a-542e-4481-9194-b034723be90c`) all passed. I treat it as a single failure of the test's interception, not of
  this plan, and did not add it to FLAKES.md on one observation.
- Reused: none. Skipped: the rest of the Rust workspace and of the browser suite (only the helm's uninstall inspection
  changed), the desktop and JavaScript checks (no UI or desktop code changed), the doctests (no documentation example
  changed) and the installer (unchanged).

The report ran the uninstall tests only; this round also ran the whole helm crate and the provisioning browser spec. Its
note that no real bind mount, SSH connection or systemd uninstall was exercised still holds. Apart from the mirror case
above, which it did not consider, nothing in the executor's report became untrue at landing.
