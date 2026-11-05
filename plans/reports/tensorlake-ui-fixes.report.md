## What this was about

Tensorlake's interface made several operations hard to follow: Suspend claimed the sandbox was suspended before the
service confirmed it; launching on a suspended host briefly said it was unavailable and showed no resume progress;
recipe Save required hunting below the fields; Refresh sandboxes appeared inert; and Add host's creation question was an
unstructured block of text. This plan addresses those six TODO entries in four draft PRs.

Suspend and Resume now show their pending state, and Suspended appears only after confirmation. The launcher shows its
own resume and reconnection progress, including quick launch. Existing-directory launches proceed when ready; a fresh
checkout still requires reviewing the refreshed preview and launching again. A failed resume says the session was not
launched.

Recipe actions and their questions and errors stay visible in Add host and Settings. Settings opens the editor at its
top, keeps its normal Close control, and gives sandbox discovery a heading, progress and a checked time even when none
are found. Add host now has field help and a recipe summary, followed by the standard numbered setup confirmation with
size, billing and idle-limit information, and Back. The Suspend question and sandbox details also use the app's normal
presentation.

## Things you should know

- Lifecycle API calls, timeouts and the rule against resuming a sandbox automatically are unchanged. A repeated Resume
  on a connected, running sandbox preserves its viewers. Routine pending transitions do not create a host-attention
  problem; independent setup warnings still do.
- Only the launcher's own pending resume suppresses its unavailable message. A host another surface is changing remains
  unavailable to it. Fresh-checkout resume does not authorize a session create.
- Back discards the displayed creation offer. Continue obtains a new plan, and Cancel and Escape send no creation
  request. Only the acknowledged Tensorlake creation preference skips this question; SSH's preference cannot do so.
- The combined billing callout also appears on Tensorlake setup reruns. Ordinary SSH setup keeps its existing answers.
  Copy plan remains in the body and preserves the helm's exact text.
- Managed snapshot builds already install both Claude Code and Codex unconditionally, so the setup step naming both was
  accurate and was left alone. The experimental-warning typo and stale implementation-spec claim that the browser fake
  refused suspend/resume were corrected. Existing unreleased changelog fragments were retained; the lifecycle fragment
  was amended, and presentation polishing needed no additional entry.
- The Tensorlake sandboxes and Manage hosts guides match the new behavior. These were incidental documentation edits; no
  preview server was started.

## Open questions and possible follow-ups

No required decision remains. The new screens were inspected in Chromium and WebKit, in both palettes. Native Mac
rendering and real Tensorlake service behavior were not exercised; the browser cases use the existing fake or
intercepted plans.

## The PRs

- [#3048](https://github.com/scode/farhelm/pull/3048/changes): confirmed lifecycle states and honest launch-resume
  progress.
- [#3049](https://github.com/scode/farhelm/pull/3049/changes): standard Suspend confirmation and sandbox details.
- [#3050](https://github.com/scode/farhelm/pull/3050/changes): visible recipe actions and sandbox discovery feedback.
- [#3051](https://github.com/scode/farhelm/pull/3051/changes): structured Add host form and creation confirmation.

All four remain drafts in one linear stack. The stack was carefully rebased onto main after manual pin ordering landed;
each PR's patch was unchanged. The review covered the new pin positions and schema, session-list ownership and menu
focus, shared API types, styles and specification text. No design conflict or manual resolution was needed.

## Checks run, reused and skipped

Runtime commands below ran through `python3 scripts/record-test-run.py`, with pinned tmux 3.7c, zero retries, four Rust
slots or up to three browser workers as applicable. Counts name observations rather than treating a later pass as
erasing a failure.

The reused checks cover completed PR1 source at `3e6f1894a121c8f397e31cb6b80e1a10c96392f9`, PR2 at
`c182602ebb6c3578ebb80354794a3d1501b84019`, and PR3 at `cc2443d864fd15d393549c9e8f496cdf450334cc`. Runs were recorded
before committing, so their manifests also retain each working tree's source fingerprint. The final rebase preserved
every PR patch; the final integration checks below cover its shared-code seams.

These were the Rust child commands for the 184-case selection and its corrected two-case selection:

```sh
cargo nextest run -p farhelm-helm -p farhelm-ui --lib -E '(package(farhelm-helm) & (test(tensorlake::hosts::tests::lifecycle_) | test(phase_matches_the_serialized_tag) | test(tensorlake_lifecycle_intent_projects_without_claiming_suspension) | test(a_changing_sandbox_is_never_probed_for_update_or_setup) | test(suspended))) | (package(farhelm-ui) & (test(hosts::) | test(list::shared::) | test(list::create_form::tests::) | test(tensorlake_lifecycle_phase)))'
cargo nextest run -p farhelm-helm -p farhelm-ui --lib -E 'test(=tensorlake::hosts::tests::lifecycle_suspend_holds_routing_and_excludes_removal) | test(=hosts::attention_list::tests::tensorlake_transitions_preserve_independent_attention)'
```

Browser child commands, from `e2e/`, in the order of the coverage groups below: PR1 and PR2 full lifecycle runs; PR3
full affected files and final opening correction; PR4 new structure and existing regressions; final integration.

```sh
npx playwright test 'tensorlake-lifecycle\.spec\.ts'
npx playwright test 'tensorlake-(recipes|hosts|settings|tunnels)\.spec\.ts'
npx playwright test 'tensorlake-recipes\.spec\.ts' -g 'Settings keeps recipe answers sticky and isolated'
npx playwright test 'tensorlake-hosts\.spec\.ts' -g 'structures its form and creation plan'
npx playwright test 'tensorlake-hosts\.spec\.ts' 'tensorlake-settings\.spec\.ts' 'provisioning\.spec\.ts' -g 'adding a Tensorlake host|the Tensorlake form|inline reuse recipe|recipe deleted during confirmation|cancel retires a Tensorlake|rerun before first create|Tensorlake settings open with the experimental warning|host-setup-layout'
npx playwright test 'tensorlake-lifecycle\.spec\.ts' -g 'pending .* names its transition|launching in an existing folder|successful resume stays honest|another surface|quick launch shows preparation|failed resume refuses|delayed create after resume|fresh checkout resumes first'
```

- Reused PR1 Rust coverage: the focused Tensorlake lifecycle, provisioning refusal and UI-state selections covered 184
  unique cases. Run `59d32350-db3b-48ee-8179-c954f266321d` had 182 passes and two fixture failures; both corrected cases
  passed in `93aa84d3-87f3-4e07-afad-5ff9fba97a27`. The original run remains failed. Its product behavior was unchanged
  by the later presentation PRs and integration rebase.
- Reused PR1 browser coverage: 40 passed, 20 per engine, in `6c6f9d38-b402-4a26-a19e-728627f107e1`. An earlier run had
  one WebKit route-teardown failure in `20a74578-c65a-46b8-8d2c-f338745cf57c`; route callbacks now drain before cleanup,
  and the failed evidence was retained.
- Reused PR2 browser coverage: four focused visual cases passed in `6cf442ba-13f4-47ab-bdbb-54cafe1e4b2a`, then 44
  affected cases passed in `48c6a740-b1c6-4600-9d3a-bc6e83d6b152`, 22 per engine. All eight focused screenshots were
  inspected. Later changes preserve that dialog and lifecycle behavior.
- Reused PR3 coverage: 48 affected browser cases passed in `dfaafaa4-17e0-4b31-80ba-18ed4ec8fdaf`, 24 per engine. After
  the final editor-opening scroll correction, its four affected palette/engine cases passed again in
  `473e87e1-7718-4d7d-9ee2-4529b9e2e2bb`; the other 44 retain their coverage. Earlier Chromium geometry failures were
  retained in `c9b86fc5-1354-44d0-b901-ec79f7541373`, `ff799516-ef67-4552-a8fb-534a7f5f5770` and
  `d0d1815a-9778-484c-91c8-7f449bf79181`, then corrected without weakening bounds. Sixteen representative screenshots
  from the full run and all four final opening screenshots were inspected.
- PR4: four new dark/light form-and-plan cases passed in `a13e8ffe-3f20-44a2-8f66-10783e50a4ee`, and all eight actual
  screenshots were inspected. Twenty existing creation, rerun, skip-policy, cancellation, warning and SSH-layout cases
  passed in `441573ef-216e-4e9a-890a-72ea67d38fc5`, ten per engine. These also establish Back's fresh plan, no-create
  cancellation and exact Copy plan bytes. Both records are complete, with no skipped or interrupted cases or cleanup
  gaps.
- Each presentation unit passed the actual release web build and the UI's all-target desktop-feature Clippy check. The
  required test-delay source check inspected 412 delays with none missing a reason. Rust formatting, changed Markdown
  formatting and whitespace checks passed. The changed lifecycle fragment passed changelog formatting. Initial PR4
  compilation found an incorrect module path, which was corrected before the passing builds. Commands:
  `dx build --package farhelm-ui --platform web --release` in the UI crate,
  `cargo clippy -p farhelm-ui --features desktop --all-targets -- -D warnings`, `cargo fmt --all -- --check`,
  `python -B scripts/check-test-sleeps.py`, `dprint check` on the changed Markdown, `git diff --check`, and
  `python3 releasing/check-changelog.py format`.
- Final integration on the rebased stack at `6619cdcd`: the CLI build, actual release web build and all-target
  desktop-feature UI Clippy check passed. Twenty transition and launcher-resume browser cases passed in
  `60f123ac-827d-465a-9259-3e5f889789e7`, ten per engine, against the integrated binaries and bundle. The report, source
  identity, stream EOF and cleanup are complete, with no skips, failures or interruptions. These target the new list
  ownership and shared session-data seams; unchanged dialog and recipe coverage above was reused. The CLI command was
  `cargo build -p farhelm`; web build and Clippy commands were as above.
- Skipped: another full runtime battery, because unchanged per-unit coverage and the targeted integration selection
  cover the identified seams; native desktop runtime, installer and provisioning-distribution gates, because those
  implementations did not change; website build, because its prose edits changed no links, headings, frontmatter or
  executable examples; hosted CI, which is manual-only. No live installation was changed.

## The review gate's outcome

Every PR passed its required independent gpt-6.1-sol review at high effort. Findings about recipe-editor errors and
fractional clipping were fixed and reviewed again. A separate review at the executing model checked the editor's scope
and supported the smaller existing-owner design. No required finding remains unresolved or was rejected. Commit and PR
wording passed fresh cold reads, and changed files received a separate documentation pass.
