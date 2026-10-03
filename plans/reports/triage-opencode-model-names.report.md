### What this was about

OpenCode accepts a model name with or without the `opencode/` prefix. Farhelm's model catalog lists OpenCode's models
only with the prefix, and several bare names (`gpt-6-luna`, `gpt-5.6-terra`, `gpt-6.1-sol`, `gpt-6-astra`) are also
Codex models in that catalog. Every model lookup compared the text exactly as typed, so two things went wrong:

- An OpenCode launch with a bare name like `gpt-6-luna` was refused as "belongs to a different harness", although
  `opencode/gpt-6-luna` worked.
- In the new-session dialog, typing `gpt-6-luna` with OpenCode selected and pressing Enter quietly switched the launch
  to Codex, a different agent with its own configuration and credentials. This was broader than OpenCode: typing any
  other harness's model name switched to that harness.

Two review findings reported this. In triage (2026-10-02) you decided:

- Choosing a model name should never switch the harness, and users must not have to type `opencode/`.
- An explicit pick of another harness's row in the model list's "Show all" view still switches.
- With no harness selected yet, a bare `gpt-6-luna` picks Codex, the primary harness, with no extra question.

What the plan did:

- Both the helm and the dialog now look a typed model up the way the selected harness spells it. For OpenCode, a bare
  name means `opencode/<name>`. One shared rule serves both, and the launch keeps the spelling the user typed.
- With a harness selected, Enter never switches it. A model that only other harnesses offer shows "`<name>` is offered
  by `<Harness>`; choose `<Harness>` to use it" and changes nothing. A bare name in a different letter case is corrected
  to the catalog's spelling, as every other harness's names already are.
- SPEC.md's rule that "a known model identifies its owning harness" now applies only when no harness is selected.

### Things you should know

- **"Restart with" changed slightly too.** It uses the same Enter logic and cannot change harness. Typing another
  harness's model there used to do nothing at all. It now shows that dialog's own existing "choose a model for this
  harness" message, not the new "offered by" one.
- **Search in command mode is unchanged.** While "other / command" is active, accepting a model from the dialog's search
  box still activates the structured launch that model belongs to, as SPEC.md already specifies. Only typing in the
  model field is affected by the no-switch rule.
- **Switching harness with a model already chosen.** Moving a Codex `gpt-6-luna` to OpenCode now keeps it, since
  OpenCode offers the same model under that name. Moving a model that only the old harness offers, such as Claude's
  `claude-fable-5`, is still cleared with the usual notice. The review gate caught a first version that kept such models
  as custom OpenCode names; that was fixed before delivery.
- **Typed name kept versus corrected.** With OpenCode selected, typing the exact bare name keeps it (`gpt-6-luna`).
  Typing it in another letter case stores the catalog's `opencode/gpt-6-luna` instead, because the helm compares names
  exactly and would otherwise treat the odd-case name as an unknown model.
- **Changelog.** #1484 adds a `fixed` changelog fragment describing both behaviors for users.
- **No docs page changed.** The docs website's OpenCode page is still a stub.

### Open questions and possible follow-ups

None.

### The PRs

- #1484 (draft): fix: keep bare OpenCode model names on OpenCode.
- #1485 (draft, on top of #1484): docs: close the review item on Enter switching OpenCode to Codex. Queue and ledger
  bookkeeping only.

### Checks

Run now:

- `cargo fmt --all -- --check`, `cargo clippy` with `-D warnings` on the proto, helm and UI crates (all targets), and
  `cargo check -p farhelm-ui --features desktop`: clean, after the review fixes.
- Recorded nextest runs c309c87c and d59c86d7 (after the review fixes). Each ran the helm's launch tests, the composer,
  new-session dialog and "Restart with" unit tests, and the proto launch tests: 107 of 107 passed. These include the new
  tests for both spellings of every overlapping name, on the helm and on Enter.
- Recorded browser runs bee5c818 and 9e2e5ba9 (after the review fixes), on Chromium and WebKit: 52 of 52 passed. They
  covered the OpenCode, "Restart with" and model-default specs, plus the sidebar's model-field tests. They include a new
  test that drives the bare name under OpenCode and the refusal under Codex against the real catalog, and the reversed
  test that used to expect a harness switch.
- The test-sleep check (zero unannotated delays), `dprint check` on the changed Markdown, and the changelog format
  check.

Skipped:

- The workspace Rust battery, doctests, installer and desktop runtime checks. The change is confined to model lookup in
  the helm's launch validation and the launch dialogs, and those areas' tests were run.
- Any runtime check for #1485, which changes only Markdown queue files.

### Review gate

A fresh-context Opus 5.5 reviewer at high effort reviewed #1484. Its findings, all fixed:

- **High:** switching the harness to OpenCode kept another harness's catalog model as a custom OpenCode name, and Launch
  stayed enabled. Ownership is now checked in both harnesses' spellings, with tests.
- **Medium:** choosing a recent setup by clicking it and choosing it through search recorded different model owners.
  Both now share one rule.
- **Low:** a bare name in another letter case was kept uncorrected.
- **Low:** one test assertion could not tell the old lookup from the new one.
- **Low:** the SPEC wording called the "Show all" pick "the one way" a model choice switches the harness, which
  overlooked command-mode search.
- **Nits:** two small code tidy-ups.

None were declined. #1485 is bookkeeping and had no review gate, as planned.
