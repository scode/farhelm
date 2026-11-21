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
