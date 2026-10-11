## What this was about

Agents could start a session only in an existing folder. Asking an agent to work through a list of tasks, each in its
own managed checkout, was refused even though the launcher already supported that operation.

The agent CLI now accepts `create --gh acme/web`, with `--github` as an alias, and applies managed-checkout launch
templates. Agents can also create or edit those templates. The checkout uses the helm's configured root and post-clone
command, with the same naming and refusals as the launcher: title "Fix parser" gives `web-fix-parser`; no title gives
the next free `web-N`. A taken explicit name is refused rather than suffixed.

## Things you should know

The usual agent approval and YOLO rules still apply. The approval card names `GitHub: acme/web` and the exact
destination directory before anything is cloned. A denied request spends no key and asks again when retried. Once an
attempt has been approved and durably admitted, the same idempotency key recovers that attempt without another card or
clone, including an interrupted start. A different request, or a template changing between a folder and a checkout,
cannot reuse that reservation.

`--gh` conflicts with `--cwd`. Only agent create can request a fresh checkout; agent clone and spawn still cannot
request one, including through a managed-checkout template. Repository intent stays within the existing agent authority:
only the helm supplies checkout configuration to the supervisor. Repository interfaces now explicitly name their source,
leaving room for sources beyond GitHub.

README and the docs Overview were left to the other work the maintainer had already assigned. Both gained the requested
managed-checkout example on main during execution, so this PR removes the covered TODO entry entirely. The launch
templates documentation and agent instructions were updated because their former refusals would now be wrong.

The careful rebase included the launcher folder dropdown, terminal path-hover fix, README/Overview edits and unrelated
TODO captures. The dropdown applies to folder destinations, and the hover fix has a separate terminal lifecycle. No
functional conflict with agent checkout admission or approvals was found; the TODO conflict was resolved by preserving
the unrelated entries.

## Open questions and possible follow-ups

None required for this plan. Supporting another repository source remains future work; this change establishes the
source-naming rule without adding another provider.

## The PRs

- [#1886](https://github.com/scode/farhelm/pull/1886/changes), draft: agent CLI managed checkouts, templates, approval
  disclosure and keyed recovery.

## Checks run, reused and skipped

- Run now: `cargo fmt --all -- --check`, changed-Markdown `dprint check`, `python -B scripts/check-test-sleeps.py` with
  the isolated interpreter (287 delays, zero missing reasons), and `python3 releasing/check-changelog.py format` passed
  after the rebase and final test correction.
- Run now: recorded helm unit selection for launch resolution, templates, approval, YOLO and keyed requests,
  `cargo nextest run -p farhelm-helm --lib -E 'test(agent_launch::) or test(template_write) or test(approved_template_write) or test(agent_request_digests) or test(keyed_create) or test(replayed_keyed_create) or test(template_supplies) or test(agent_create_waits) or test(yolo_rule)'`,
  passed 17 tests in run `75013058-f8b4-4842-97c8-3fb58bf2e6dd`. No runtime substrate skips.
- Run now: recorded CLI, agent instructions, GitHub checkout and real-agent relay selection,
  `cargo nextest run -p farhelm --bin farhelm --test agent_cli --test e2e -E 'binary(agent_cli) or test(agent_instructions::) or test(managed_checkout_flags_are_scoped) or test(github_checkouts::) or test(agent_listing_real_stack::)'`,
  ran 52 tests in `76fda6c1-ff10-4026-be63-baa5a170fa11`: 51 passed and the new taken-name assertion failed. Exact
  reproduction `212f8607-bb25-4445-be2d-1631f741c0d5` confirmed the test wrongly required the path in the existing
  occupied-name refusal. The corrected exact scenario passed in `9964706d-cc12-44e0-a0e7-525adaa735d0`, including
  managed-template application and preservation of untracked work. The other 51 passes are reused because only that
  test's assertions and explanation changed. No runtime substrate skips; the failures remain retained.
- Earlier template-write run `72522f09-2882-4015-a8fe-e2779c09440d` passed the three launch-resolution tests but failed
  an old fixture-count assertion after the test added a third template. The corrected exact test passed in
  `7602f8bc-99fe-4abb-b5af-ee272740a6d5` and again in the 17-test helm selection. Both authoring failures were corrected
  in this session; neither is treated as a latent flake or erased by the later passes.
- Run during this plan and reused: `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings` and `cargo check -p farhelm-ui --features desktop` passed at
  `1a7c3182`. The intervening changes to this plan were test assertions, comments and TODO bookkeeping. Upstream
  launcher and terminal-hover changes were inspected for interaction; they do not change agent admission or approval
  contracts. The current web build covers the rebased renderer and assets.
- Run now: the required debug `cargo build` and release web `dx build --package farhelm-ui --platform web --release`
  passed. The website's frozen install and build passed with 32 pages and all internal links valid. The warm website
  build passed again on the final rebased source; the frozen install is reused because the lockfile did not change.
- Run now: recorded `npx playwright test github-checkouts.spec.ts` on Chromium and WebKit, one worker and zero retries,
  in `0a10caf4-48e0-4052-95f3-f47cddfb9196`: 25 passed, one failed, no skips. The new WebKit test failed before asking
  for a checkout because its observer and driver pages both auto-selected the requester; the trace showed the driver
  detached by the other client. An unchanged exact WebKit reproduction passed in `3250ab6c-4e64-402d-918d-d0d1795dfec7`,
  so that pass did not settle the intermittent fixture race. The test now completes the observer's attachment to another
  session before creating the requester. Recorded
  `npx playwright test github-checkouts.spec.ts -g 'agent managed checkout approval and keyed recovery'` passed both
  engines in `8d64fedc-0d33-4e13-b04d-df7cce27bb89`, two cases, no skips. This covers denial, the disclosed repository
  and directory, a live created session, and keyed recovery without another card or clone. The 25 other full-spec passes
  are reused because their tests and product behavior did not change. Failed evidence is retained; this was a fixture
  written and corrected in this session.
- Skipped: full workspace, desktop-runtime, installer, provisioning and full browser batteries. Focused real-stack
  checkout and approval coverage addresses the changed lifecycle; no installer, provisioning or desktop-shell code
  changed. No website deployment or hosted CI run was requested.

## Review gate's outcome

The required fresh-context GPT-6.1 Sol reviewer at high effort found no correctness, design, idiomaticity or
test-contract findings. It also accepted the fixture-count and taken-name assertion corrections, the documentation pass,
and the browser attachment fixture correction. This was a source review; runtime evidence is reported separately above.
The commit and PR prose passed a fresh GPT-6.1 Sol cold reader at medium effort after one rewrite.

The executing agent did all implementation work. Separate agents checked the source, the commit and PR wording, the
saved execution state, and this report. Their written findings were retained and read by the executing agent. The source
and wording reviewers were requested at the models and efforts stated above, but the harness did not expose independent
confirmation of which models actually ran or their usage. Some initial launch bookkeeping was also missing. These gaps
limit model attribution and cost accounting; they do not replace the retained review findings or the separate recorded
runtime evidence.
