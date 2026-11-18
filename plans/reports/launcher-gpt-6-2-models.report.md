## Since the last review

Draft PR #4099 now includes the saved-template compatibility decision recorded after the first review. The review caught
a case where the desktop launcher dropped the inferred agent. The updated draft applies both the agent and the saved
model, and includes a desktop regression test.

## What this was about

The session launcher replaces its `gpt-6.1-sol` and `gpt-6-astra` suggestions, including provider-prefixed forms, with
GPT-6.2 Sol and Astra in Codex, Goose, Pi, OMP and OpenCode. In Codex, the new Astra offers Low, Medium and High effort.

## Things you should know

Older templates that named only a model relied on the suggestions to supply an agent. When the template and launcher
both omit the agent, bare `gpt-6.1-sol` and `gpt-6-astra` still pick Codex and `opencode/gpt-6.1-sol` and
`opencode/gpt-6-astra` still pick OpenCode. Farhelm passes along the saved model ID as a custom model. This works in the
desktop launcher and through the CLI's template option. Recent launches and templates that already specify an agent
continue using their saved model through the custom-model path.

## Open questions and possible follow-ups

Actual availability of the new OpenRouter and OpenCode Zen IDs depends on the provider. As decided during planning,
validation covered Farhelm's suggestions and launch handling. It did not make vendor requests.

## PRs

- #4099 — Update the launcher model suggestions and preserve saved templates.

## Checks

- Ran the selected helm launch, session-route and shared launcher tests through nextest. All 218 passed, including the
  catalog route and the shared saved-template tests. Recorder run `f2491515-c59f-4743-8329-b1817581c2dc` covered
  snapshot `50e9180f`. The final changes after that snapshot touched only spec prose and desktop code, so this evidence
  still covers the helm and shared launcher behavior.
  `cargo nextest run --locked -p farhelm-helm -p farhelm-proto --lib -E 'test(launches::tests::) | test(sessions::tests::) | test(/^launcher::/)'`
- Ran the desktop template application tests through nextest. All 39 passed, including the regression for bare Codex and
  OpenCode Zen IDs. Recorder run `af56420b-875b-4bbd-8794-8ac9d7d3272e` covered the pushed source tree.
  `cargo nextest run --locked -p farhelm-gui --lib -E 'test(templates::apply_tests::)'`
- Ran Clippy for `farhelm-helm`, `farhelm-proto` and `farhelm-gui`, all targets with warnings denied. Passed at
  `b4f1d53e`, recorder run `e9943477-07a3-423d-bc06-5c1a7d8db894`.
  `cargo clippy --locked -p farhelm-helm -p farhelm-proto -p farhelm-gui --all-targets -- -D warnings`
- Ran Rust formatting, dprint on changed Markdown, the changelog format check, the Rust test-sleep checker and the diff
  whitespace check. All passed. The full workspace runtime suite and screenshot goldens were outside the affected
  behavior. The template tests cover the change to saved choices in the existing controls.

## Review gate

The required gpt-6.2-sol review at high effort found that desktop replay could drop the agent inferred by template
application. The draft now applies the resolved agent with the saved model and has direct desktop coverage. The reviewer
confirmed the fix and reported no remaining findings. A cold reader reviewed the commit title.

### Landing

#### What landed with it

Since the PR was built, main gained the brick-test recovery and release-candidate discovery changes (#4103, #4105) and
the release gate's new run of the GPUI end-to-end smoke (#4092). None of them touches the launcher's model catalog,
templates or the specification passages this PR edits, and none interacted with it. The rebase applied cleanly.

An independent reviewer read the PR against the specification and the code it changes. It found nothing that had to
change before landing. It confirmed three things about the saved-template exception:

- Only saved templates reach it, because every way of saving a template refuses an empty name, and the CLI's own flags
  are applied as an unnamed template.
- The desktop app's change to how it replays a template's model applies no existing template differently. The only new
  behavior is the retired model picking its former agent.
- Nothing still suggests the old model IDs.

#### Fixes made while landing

- **A helm test still relied on Astra being high-only.** The test that an agent's clone of a session into a new
  environment keeps the source's stored launch, even one today's catalog would refuse, used the old Astra ID at Medium
  effort as its example of a refused launch. With Astra offering Low, Medium and High, that launch is no longer refused,
  so the test failed on its own premise. The executor's test selection did not include the agent-request tests, which is
  why its report shows every test passing. The example is now a Codex launch at Extra High effort, which Codex never
  offers, on the current Astra ID; what the test checks is unchanged.
- **Wording and cross-references from the review.** SPEC_impl.md said the CLI's unnamed template was chosen so that only
  the current catalog infers agents for flags, a reason that code (older than this PR) was never written for. It now
  says what makes the exception safe: saved templates always have a name. The helm's model catalog now says that
  replacing a model retires its ID, and points at the short table of retired IDs the shared template code keeps. That
  table points back at the catalog, and the CLI's unnamed template says why its empty name matters now. Without these, a
  future swap (6.2 to 6.3, say) would silently break model-only templates that name the 6.2 IDs.

#### Things you should know that the report does not say

- **The next model swap needs the same treatment by hand.** The retired-ID table lives in the shared template code, not
  beside the catalog, because the desktop app receives the catalog over the wire. Nothing enforces adding to it; the
  comments added at landing are the only reminder. `gpt-6-sol`, retired in an earlier swap, never got an entry, which
  fits the decision's narrow scope. Moving retired models into the catalog the helm serves would make this automatic; it
  would change the catalog's wire format.
- Whether a template is saved or comes from CLI flags is told apart by its empty name. That holds today; an explicit
  marker would be sturdier if more callers start applying unnamed templates.

#### Checks

The change is the helm's model catalog, the shared template code and the desktop app's template replay, so the landing
ran the lints and the focused tests of those three places, locally. Nothing drawn changes, so no screenshot goldens or
end-to-end runs.

On the PR rebased onto main:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `cargo clippy --all-targets -- -D warnings` and
  `cargo clippy -p farhelm --bins -- -D warnings`: passed.
- The check that a GUI change modifies the app's shared foundations only where they allow it
  (`scripts/check-foundation-paths.py`, from the base revision): passed.
- `cargo nextest run -p farhelm-proto`: 277 passed (`1f89df51-7a83-4224-92c7-42c7dd358506`).
- `cargo nextest run -p farhelm-helm --lib` for the catalog, templates, agent launches, agent requests and the composer
  routes: 191 passed, 1 failed (`1c51f9b3-7b05-44ae-a610-8969ae322ec6`), the environment-clone test described above.
- `cargo nextest run -p farhelm-gui --lib` for templates and the launcher: 794 passed
  (`a14e5071-aa50-47e7-a0bf-905095b05c3f`).

After the landing fixes (that test's example launch, comments, and SPEC_impl.md wording): `cargo fmt --all -- --check`,
`python -B scripts/check-test-sleeps.py`, `dprint check` and
`cargo clippy -p farhelm-helm -p farhelm-proto --all-targets -- -D warnings` passed, and the same helm selection passed
in full, 192 tests (`1cb21076-3818-41ea-b606-0f268741da46`). The proto and GUI runs above were reused for the final
commit, since the fixes changed only comments in those crates.

Skipped: the rest of the Rust workspace, the desktop process's tests, the screenshot goldens, the end-to-end smoke and
the Mac checks, since the change touches only which models are suggested and how a template picks its agent.
