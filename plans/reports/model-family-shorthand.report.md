## What this was about

Typing a word such as `sol` in the session launcher used to filter the model list. Enter can now pick that family's
latest model. The launcher stores the selected model's full ID, so a saved setup keeps the selected version when a newer
model arrives.

## Things you should know

In the model field, an exact family word highlights the latest model for the selected harness, ignoring ASCII letter
case. As agreed, the field asks for a harness when several offer that family and none is selected. A family offered by
one harness selects that harness and its latest model. Launcher search preselects the first matching harness's latest
model when none is selected. A family shortcut waits for the catalog to load and the next press of Enter. Full IDs and
partial text follow their ordinary search paths.

The words are `astra`, `fable`, `glm`, `luna`, `sol`, `spark` and `terra`. `glm` picks the full GLM model. GLM Flash
remains available by its model ID. `grok` still means the Grok harness, and `grok-4.7` is model text. The CLI and
template editor take model IDs literally.

## Open questions and possible follow-ups

None.

## PRs

- Model family shorthand in the session launcher [#4198](https://github.com/scode/farhelm/pull/4198/changes).

## Checks

- Recorded `cargo nextest run -p farhelm-gui -p farhelm-helm -p farhelm-proto --lib` run
  `5b7806a8-5b56-48a7-bb78-0d664e6e65cf` passed all 107 selected model field, search, catalog-route and protocol tests,
  including a reply held until after Enter. The rebase from `4b900c6e` carried an unrelated TODO addition and wording in
  the agent instructions. The same feature diff applies on both bases.
- The earlier recorded run `be1025f6-a31d-4361-a3c9-592ac5513179` passed 231 tests, including broader launcher and
  template coverage. The final run rechecked the model and search behavior affected by the catalog-loading fix.
- `cargo clippy --locked -p farhelm-proto -p farhelm-helm -p farhelm-gui --all-targets -- -D warnings`,
  `cargo fmt --all --check`, scoped `dprint check`, `python3 releasing/check-changelog.py format` and
  `python -B scripts/check-test-sleeps.py` passed.
- Skipped the full release battery, desktop smoke and goldens. The focused GPUI tests exercise the visible selection and
  Enter in the existing controls.

## Review gate

The independent source review caught an early Enter that could store the family word before the catalog replied and a
shortcut that could be missed when a family gets a new model. The final patch keeps the shortcut in the field while
loading, preserves explicitly selected rows, and derives the shortcut from the stable family key. The commit title and
message passed their cold read.

### Landing

#### What landed with it

It landed in a round of three plans: shift-drag-marking, model-family-shorthand and unusable-folder-refusals. Since its
base, main gained earlier rounds' plans (session group dragging, model families, the covered-link refusal, the demo
video recorder, the Replace with checkout hint and others) and bookkeeping. It builds on the earlier model-families
plan: typing a family word such as "sol" in the launcher picks only that family's newest model. The reviews found no
interaction with the other plans.

At your request this landing was reviewed with extra skepticism, by me and by independent reviewers, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. Family words work only in the
launcher; templates, the command line and Restart treat them as before, as the plan intended, and removing the helm's
duplicate catalog function was a real simplification.

#### Fixes made while landing

- **Nothing said where the family words live.** SPEC_impl.md now names the table of family words. It says a new or
  renamed model family needs a decision about its word, and that a test fails if the table names a family the model list
  no longer has.

#### Things you should know that the report does not say

- **The family word travels twice.** The word table is compiled into both the helm and the app, and the helm also sends
  each row's word to the app in the catalog. The app only ever talks to its own build's helm, so the wire copy adds
  nothing but a protocol field and about 35 test lines. Possible follow-up: drop the field and compute the word in the
  app.
- **A family word on the wrong agent gets a long refusal.** With Claude chosen, typing "sol" and Enter says "sol is
  offered by Codex or Goose or Pi or OMP or OpenCode; choose Codex or Goose or Pi or OMP or OpenCode to use it". Before
  this plan, "sol" became a custom model. The message is accurate but clumsy, untested, and not in SPEC.md. Possible
  follow-up: "Claude has no sol model; choose an agent that offers it."
- **If the model list failed to load, Enter on a family word does nothing and says nothing**, while the search box
  suggests typing a custom model ID in that same field. Possible follow-up: say the models did not load.
- SPEC.md does not list the family words (astra, fable, glm, luna, sol, spark, terra), and the app does not show them
  anywhere.

#### Checks

The three plans change the desktop app's session list and launcher, the helm, the shared protocol, the supervisor and
its end-to-end tests. The landing ran the lints, the whole Rust workspace suite, the desktop process's tests, the app's
end-to-end smoke and the reference screenshots (goldens). Builds ran locally; the workspace suite and the smoke ran on
boat sandboxes (cloud VMs), from binaries built locally.

On the plans as delivered, stacked on main as it stood when the round started:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`,
  `python3 -B scripts/test-gui-e2e-smoke.py`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, and `cargo clippy -p farhelm-desktop --all-targets -- -D warnings`
  once without and once with `--features agent`: passed.
- `cargo nextest run --workspace --exclude farhelm-desktop`, from a locally built archive, split in three across boats:
  2761, 2658 and 2636 tests, 8055 in all (`cc2d7e45-c34d-471b-9521-e292a5d3eabc`,
  `ad12af75-4c3f-479f-9c87-e8df09421a5a`, `89433db2-2d08-49cc-a1af-a5b8696a6edb`). One failed: the macro
  compile-contract test, which builds a small crate offline from this machine's cargo cache, which a boat does not have,
  so it fails there every time. It did not run anywhere this round; nothing in this round touches it, and it passed
  locally in an earlier round today. One test skipped itself at runtime because it needs a recreated file to get a new
  inode number and the boat's filesystem reused the old one; it did not run anywhere else this round.
- `cargo nextest run -p farhelm-desktop`, locally: 176 passed (`6610603b-7df3-43e5-ab7c-cf4756a7b544`).
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on a boat: 49 of its 50 steps passed and the clipboard
  paste step was skipped, as always on Linux (`a05cf1f2-6da9-4972-988a-f814bff20a5e`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`1c6c16a9-9ed6-4d42-9ce3-0399f6a2bc61`).

On the fixed tree (the Shift sweep's start threshold, the folder check's access call, comments and SPEC_impl.md):

- `cargo fmt --all -- --check`, `dprint check` and
  `cargo clippy --locked -p farhelm-gui -p farhelm-supervisor --all-targets -- -D warnings`: passed.
- The session list's whole test module, locally: 516 passed (`69a3bcc4-c0af-480b-9732-b36393d20617`), the sweep and grip
  drag tests included.
- The supervisor's session lifecycle and terminal tab end-to-end modules, which exercise the folder check on create,
  restart and tab open, locally: 96 passed (`a83797de-4468-4cb7-8c6d-df759c257c5b`).

The boat suites, the smoke, the desktop tests and the goldens were reused for the fixed tree: after them only the two
small code changes above and comments changed, and the focused runs above cover both.

Skipped:

- The Mac-only checks: no Mac-specific code changed.
