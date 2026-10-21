# Clear host uninstall refusals

## What this was about

Choosing uninstall for a host Farhelm could not use produced an internal state name and told the user to resolve it
without saying how. The refusal now names the problem and its remedy for an older host, a newer host, a changed
identity, an unidentified install, and two entries reaching the same machine. Each offers Remove, which forgets the
entry without changing the host.

## Things you should know

An older host directs the user to its update button before trying uninstall again. A changed identity names the recorded
and reported identities: adopt the host from its menu if it is the intended machine, or correct the SSH destination in
settings. An install that has not identified itself says Farhelm keeps checking and will clear the problem once it does;
a wrong destination can be corrected in settings. It does not offer adoption of an unidentified install.

Uninstall's admission checks and their order stay the same. A disconnected host still needs to reconnect, and a host
whose identity changes during the fresh uninstall check still directs the user to Retry and the host list's advice.
Those two refusals remain ordinary refusals without the Remove button.

A newer host names both builds and says to update this helm, or use a helm running that host's version. It gives a
specific “version or later” target only for a stable semantic version; development, prerelease, build-metadata and
unrecognized labels do not become upgrade targets. Two entries reaching the same machine name the other entry by its
current display name, falling back to its host number if it was removed meanwhile. Review found that the other entry can
be the helm's own machine, which has no uninstall action in its menu; that case now points to farhelm uninstall on that
machine instead.

The refusal text starts with Farhelm and has no final period, matching the existing refusal that offers Remove. The
changelog fragment is kind changed. The matching TODO entry is removed. The incidental docs edit is in Manage hosts,
under uninstall; this unattended run starts no preview server.

## Open questions and possible follow-ups

None.

## The PRs

[2491 — explain host uninstall refusals](https://github.com/scode/farhelm/pull/2491/changes), open draft based on main,
head 13df92cfd6f7553aab96f85f215ae00e0b3bab65.

## Checks run, reused and skipped

Runtime checks used the recorder with nextest 0.9.148, four slots, zero retries and the required, hash-matched tmux 3.7c
on Linux x86_64. The final run's source capture is complete: base 8d6bfc54 with the reviewed 1682486b tree applied,
tracked-diff SHA256 87d82b9bb9a24e8dfaf212ea9ab2ea4f17cbb0e7eba468ed1085c9650d2b5754. That tree and the published PR's
tree are identical; publication changed commit metadata only. No runtime SKIPPED marker appeared in any of the three
complete retained outputs. Cases excluded by the filters are not runtime passes.

- Final run 2a332eb8-f7d4-4cf3-aecd-0652bb797067 passed 21 of 21 selected cases. This covers the new sentences, remote
  and local twin naming, a removed twin, the typed HTTP remedy, and existing uninstall admission and retry paths.
  Recorded child command:

  ```sh
  cargo nextest run -p farhelm-helm --lib -E 'test(provisioning::uninstall_refusal::) | test(provisioning::tests::uninstall_) | test(provisioning::tests::an_unconnected_host_is_refused) | test(only_typed_uninstall_remedy)'
  ```

- Correction run d8d18bdf-53ff-4f1c-bdae-dfd6d0ef015f passed nine of nine at the intermediate f438cd9f tree, before
  completing the unaliased local name. It checked the corrected assertions and local-host advice narrowly before the
  final selection above. Recorded child command:

  ```sh
  cargo nextest run -p farhelm-helm --lib -E 'test(provisioning::uninstall_refusal::) | test(uninstall_duplicate_) | test(uninstall_refuses_an_unverified_peer_and_a_foreign_unit) | test(uninstall_refuses_an_identity_reported_for_a_host_with_none_on_record) | test(only_typed_uninstall_remedy)'
  ```

- Initial run 6656072e-9d3b-4de3-8a09-e98f81217aa0 failed four new assertions, with 15 other selected cases passing. The
  assertions incorrectly expected unquoted values or a different control-character notation from Farhelm's existing
  escaping. Production escaping was preserved and the expectations corrected. This failed command is retained privately
  and is not counted as a pass. Recorded child command:

  ```sh
  cargo nextest run -p farhelm-helm --lib -E 'test(provisioning::uninstall_refusal::) | test(provisioning::tests::uninstall_) | test(provisioning::tests::an_unconnected_host_is_refused)'
  ```

- On the final source, cargo fmt --all -- --check and cargo clippy -p farhelm-helm --all-targets -- -D warnings passed.
  The isolated interpreter's python -B scripts/check-test-sleeps.py passed: 307 delays inspected, none without a
  rationale. These inspect the changed Rust code and tests.
- Reused the successful dprint check of SPEC.md and TODO.md at f438cd9f; these files did not change afterwards. At
  282d4d61, python3 releasing/check-changelog.py format passed with 66 fragments and python3 scripts/check-agents-md.py
  reported zero errors and warnings. The fragments, instruction files, paths and Markdown headings they inspect stayed
  unchanged afterwards.
- Reused website's bun run build at 282d4d61: 32 pages built and all internal links valid. The website page did not
  change afterwards, and the later Rust/SPEC corrections do not feed that build.
- Skipped the full helm suite, workspace Clippy and broader runtime batteries: this changes refusal prose and an
  existing refusal's type within the helm, without changing admission, wire contracts or other crates. Targeted tests
  and helm all-target Clippy cover those risks. No inexpensive browser fixture drives actual version-skew uninstall; the
  existing browser fixture substitutes a generic refusal reply. The typed HTTP test verifies the response flag. The
  dialog's existing handling of that flag is unchanged and was inspected; no browser run or UI change was added.
  Installer, release and live-install checks are outside this change.

Main moved only for another plan's claim; its full diff was inspected and had no interaction with this work. No rebase
or further runtime run was needed for that bookkeeping change.

## The review gate's outcome

The required single fresh-context gpt-6.1-sol/high review passed on 1682486b, whose source tree matches the draft PR.
Its findings about escaping assertions, the local machine's unavailable menu action, the overly broad SPEC promise and
the absent-identity sentence were all addressed. No findings remain. The commit title and description also passed the
required cold read. A fresh scope reassessment found no unnecessary complexity in the local-host correction.

### Landing

Landed as #2491, with the same content the report describes, rebased onto this round's main. It was the only plan in
this landing round.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained ui-peer-text-type, which makes the UI escape host- and agent-supplied text on
display, and plan bookkeeping. A fresh-context reviewer and I read it against this plan. The new refusals embed values
from the host (another entry's name, recorded and reported identities, build versions). They reach the screen as one
message that the uninstall refusal dialog escapes and isolates on display, after the helm has already quoted each value,
so hidden or direction-changing characters in them show escaped and cannot rearrange the sentence. Whether Remove is
offered comes from a separate flag in the helm's reply, never from the text. ui-peer-text-type changed only how that
dialog calls the escaping, not what it does. helm-owned-by-default was already in this plan's base: uninstall requests
finish even if the client disconnects, and this plan only adds a read of the host list there. The plan applied without
conflicts.

#### Fixes made during the landing

None.

#### Possible follow-ups

The review at landing found two issues the executor's review did not; neither blocks landing.

- Two uninstall refusals show the host's identities: the changed-identity refusal, which this plan rewrote to show them,
  and the refusal when the identity changes during the fresh uninstall check, which already showed them before this
  plan. Both put each identity in quotes with control characters escaped, but leave spaces and look-alike letters as
  they are, so for example `ab 12` and `ab  12` (two spaces) look the same. SPEC.md says identities the user compares
  must show spaces visibly so that two different ones never look alike, and the host list and the adopt prompt already
  show them that way. No unsafe action follows, since adopting shows the identities correctly. Possible follow-up, which
  I recommend: show identities in these refusals the way the host list does, with spaces and other non-ASCII characters
  made visible. The alternative is to drop the identities from these messages and point to the host list.
- For two entries reaching the same machine, the refusal names the other entry, which takes a read of the helm's own
  host list. If that local read failed, the user would get a generic error instead of the refusal; before this plan they
  saw a less helpful refusal naming an internal state. Such a read essentially only fails if the helm's database is
  broken, so this is low priority; falling back to the refusal without the other entry's name would close it.

#### Checks

This plan was executed before ui-peer-text-type landed. It changes only the helm's uninstall refusals, SPEC.md and one
docs page, plus its changelog fragment and TODO entry, so the browser runs below select the two spec files that drive
host uninstall rather than the full suite. All of the following ran on the plan stacked on this round's main:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, the isolated test-sleep check, `dprint check` on every changed
  Markdown, JSON and TOML file, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`, and
  the website build (`bun run build` in `website/`): all passed.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  parallel test slots and no retries, run `a7a3e8f9-7104-476d-aa06-8af4056f4ea3` (kept): 4006 passed, 1 failed:
  `terminal_backpressure::shallow_pause_resumes_without_reset_or_replay`, which has an open TODO deflake entry ("Shallow
  pause resets under load") and fails nine runs in ten on main without this plan (see supervisor-request-modes' landing
  notes). This plan does not touch terminals or the supervisor. No test reported itself skipped at runtime in this
  output.
- `npx playwright test 'provisioning\.spec\.ts' 'terminal-multihost\.spec\.ts'` in `e2e/` on Chromium and WebKit, three
  workers with their own helm and supervisors, no retries, run `ed5bedf6-7b30-4450-beaf-fd97809fbc21`: 242 passed, none
  skipped. A first attempt of the same command was refused by the test recorder before any test ran (run
  `5e0eb05e-a0f8-43ba-bde6-52789fa3177d`); its reason was not captured, and the repeat ran normally.
- Skipped: the full browser suite (this plan changes no UI code and only the uninstall paths those two files cover), the
  desktop and JavaScript tests and the desktop checks (no UI or desktop code changed), the doctests (no documentation
  example changed), the native Mac app (not runnable here) and the installer (unchanged).

Superseded at landing: the report's checks and its statement that no rebase was needed (replaced by the runs above on
this round's main), its "None" under open questions and follow-ups and its "no findings remain" (the review at landing
found the two follow-ups above), and its PR line (now merged). Nothing else in the executor's report became untrue at
landing.
