## What this was about

Starting a managed checkout in a new sandbox or container could fail before Farhelm created the environment if the
all-hosts checkout folder was unset. Farhelm now uses the new environment’s home folder when no checkout folder is
configured. The same rule applies to a fresh checkout made with **Replace with** inside a session’s existing sandbox or
container.

## Things you should know

A configured checkout folder still applies. Explicit per-host overrides keep their usual precedence over the all-hosts
default. Local and other installed hosts still need a folder in effect and offer inline setup when one is missing.
Settings and the command line continue to report the saved checkout configuration.

The plan’s reference to a fresh checkout belongs to **Replace with**. **Restart with** keeps the session’s current
folder under the lifecycle spec.

Repository suggestions can still come from the installed host you opened the launcher on. If it cannot finish the search
because it lacks a checkout folder, the new environment launch shows the existing hint that you can type an `owner/repo`
pair. The test checks that the hint appears without a setup field or a request to read checkout settings.

## Open questions and possible follow-ups

None.

## PRs

- Use the environment home for managed checkouts. [#4519](https://github.com/scode/farhelm/pull/4519/changes)

## Checks run, reused and skipped

- On rebased revision `eff76b2d`, all nine selected `cargo nextest run -p farhelm-helm -p farhelm-gui --lib` cases
  passed without runtime skips in recorder run `b6651786-c81a-48d9-a7b4-6eafb8fa97df`. The selection was:

  ```text
  a_recipe_checkout_binds_its_new_machine_preview_before_one_create
  a_recipe_checkout_uses_its_environment_home_without_setup
  fresh_replace_on_a_session_owned_host_uses_its_home_without_setup
  create_with_valid_github_repo_without_a_root_points_to_settings
  fresh_replace_reconciles_original_payload_and_vetoes_source_replays
  new_container_checkout_ignores_the_source_hosts_missing_folder
  a_host_without_a_checkout_folder_offers_the_setup_inline
  switching_hosts_drops_the_old_preview_and_checks_the_new_hosts_setup
  hook_setup_consent_reaches_creates_restarts_and_replacements
  ```

- Targeted `python3 scripts/check-gui-goldens.py --update --scene launcher-new --scene launcher-new-container -j 4`
  captured both themes on `ed96233c` in recorder run `d9cd8886-2eeb-40ec-86a7-a4bb675236a3`. The four files matched
  their references. On `eff76b2d`, the same two scenes matched again in comparison mode in recorder run
  `bcb9baf8-0987-45db-9100-3c3c57caca3b`. The matching images carry forward the visual review.
- `cargo fmt --all -- --check`, dprint for the changed Markdown, `git diff --check`,
  `python -B scripts/check-test-sleeps.py`, the plan queue check, and changelog format validation passed.
- The complete workspace suite, cloud launch, and real container lifecycle were skipped. The focused tests exercise the
  Helm and launcher checkout decisions, using stand-ins for providers and supervisor messages. The plan changes the
  checkout folder policy at those boundaries. The selected references have no new images to publish.

## Review gate

The source review passed. Sol high confirmed fixes for the incomplete search hint, host removal during a settings read,
and the Replace with test’s setup. The rebase preserved the reviewed patch.

Opus inspected all four launcher captures. They matched the saved references. The container picture shows the Folder
destination. The launcher test directly checks the missing-folder case with Managed checkout selected. All findings
within this plan’s scope are resolved.

### Landing

#### What landed with it

The plan's PR, #4519, merged alone in this round, with a landing fix, #4532. Since the executor based it, main had
gained the four plans of the previous round: the helm triage, the Podman triage, the exited-terminal scrollback fix and
the status-mark hover change. The helm triage edits the same helm files as this plan (session creation, agent requests
and Tensorlake setup), but in other places; the only textual conflict was in TODO.md, where both plans had removed their
finished entries. The interaction it did have is a test pattern, fixed below. One consequence that follows directly from
the plan: agent-launched recipe templates, which the helm triage now allows from ordinary hosts, follow the same rule,
so they do not need a default checkout folder either.

An independent reviewer read the PR against the plan, its Decisions and the specs. It found no blocker:

- **Plan.** Every acceptance criterion and Decision is met: a session's own sandbox or container uses the default
  checkout folder when one is set and its home otherwise, and nothing asks for a folder. Installed hosts, including
  Tensorlake hosts added through Add host, still need one and still get the inline setup.
- **Every path.** Repository suggestions, the checkout preview and the create (yours, an agent's, and a fresh checkout
  through Replace with) all use the same rule, so the preview and the create agree on the folder. Restart with cannot
  change a session's folder, so it is not affected.
- **The home folder as checkout folder.** Trash, prepared spare checkouts and repository discovery behave correctly with
  the home folder: trash and spares go to hidden folders there, and discovery looks only at the home folder's own
  entries. Settings and the command line still show the stored values, as the plan decided.
- **Specs.** SPEC.md and SPEC_impl.md match the code, and the website docs already limited the setup step to ordinary
  hosts.

#### Fixes made while landing

All in [#4532](https://github.com/scode/farhelm/pull/4532/changes), tests and a comment only:

- **Heartbeats in three tests.** The helm sends each sandbox or container host an empty heartbeat every 30 seconds. The
  scripted hosts in the plan's three environment checkout tests treated every message they received as a request, so a
  run longer than one heartbeat interval would fail them. None had failed yet; the fix is preventive. The helm triage
  had just fixed the same pattern in another test; these now skip heartbeats the same way.
- **A stale quote.** A launcher comment quoted SPEC.md's old wording for when the folder setup appears; it now says
  "installed host", as SPEC.md does.

#### Checks

The plan changes how the helm picks the checkout folder when it previews and creates a managed checkout, and which hints
and setup field the launcher shows for a new environment. It was rebased onto a main that had gained a large helm triage
touching the same files, so the landing ran the whole Rust workspace suite rather than only the plan's tests. The
workspace suite and the lint runs ran on a separate arm64 Linux test worker; the launcher's reference screenshots and
the small checks ran locally on x86_64.

On the plan as delivered, rebased onto main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, with four build jobs, on the test worker (recorder run
  `4368da9b-4bd5-478a-8658-f6ab62c9d3d4`): all 8255 passed, the real Podman environment test among them (it ran, about
  two minutes).
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker,
  not through the recorder: passed.
- `python3 scripts/check-gui-goldens.py --scene 'launcher-*'`, locally (recorder run
  `5cedf8cb-da8e-4444-af5b-af871a3c7d12`): passed, the new-environment launcher scenes included.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` (in its isolated environment), `dprint check`
  and `python3 releasing/check-changelog.py format`, locally: passed.

After the landing fix (#4532):

- `cargo nextest run -p farhelm-helm --lib` limited to the three environment checkout tests the fix changed, on the test
  worker (recorder run `cb073940-4287-455a-9592-22f6e4768227`): all 3 passed.
- `cargo clippy --all-targets -- -D warnings`, on the test worker: passed.
- `cargo fmt --all -- --check` and `python -B scripts/check-test-sleeps.py`, locally: passed.

The rest of the first runs still applies to the fix, which changes only those tests' scripted responders and a comment.
Skipped: the desktop app's own tests and lint runs and the end-to-end smoke test, since the plan does not touch the
desktop app and its launcher change is covered by the GUI library tests in the workspace run and by the reference
screenshots; a launch into a real Tensorlake sandbox or Podman container with no checkout folder set, which would spend
cloud resources (the real Podman test in the workspace run sets its own folder); macOS, since no Mac was part of this
landing; CI, which runs only on demand.

#### Things you should know that the report does not say

- A sandbox or container's own host settings still describe an empty checkout folder as using the default from Settings
  → Checkouts, or show "No default set". With no default set, new checkouts there now go to its home folder, and that
  page does not say so. Showing the stored value was the plan's decision; only the wording is incomplete.
