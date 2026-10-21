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
