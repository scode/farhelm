# Putting harness-specific logic behind per-harness boundaries

NOTE: this is a proposal, not a decision to do the work. It records, on 2026-09-29, how the code at `7c451b54`
represents agent harnesses, where harness-specific branching lives, what two independent reviews of a first-draft
refactor proposal found, the maintainer's answers to the questions they raised, and the proposal that came out of all
of that. Line numbers and counts are as of that commit; counts were taken by grep and are rough. It answers the TODO
entry "Consider refactoring how harnesses are represented".

## How harnesses are represented today

Four enums name harnesses, each on its own axis:

- `LaunchHarness` (`crates/farhelm-proto/src/launch.rs`) is what the user picked: Cursor, Codex, Claude, Muse, Goose,
  Pi, OMP, OpenCode, Grok. It already centralizes a few properties as exhaustive methods: `agent_kind()`,
  `offers_workspace_trust()`, and `offers_permission()`.
- `AgentKind` (`crates/farhelm-proto/src/lib.rs`) is the supervisor's integration once the process runs: Claude,
  Codex, Goose, Pi, OMP, Grok, Generic. Muse, Cursor, and OpenCode map to Generic.
- `ReportVendor` (proto) is the discriminator on a conversation report from `farhelm internal hook`.
- `LocatorVendor` (`crates/farhelm-supervisor/src/agent_kind/mod.rs`) is the typed-locator prefix for Pi and OMP.

The `LaunchHarness`/`AgentKind` split is a real distinction ("what the user chose" versus "what integration runs") and
nothing here proposes merging them.

Two supervisor seams are already trait-shaped. `AgentIntegration` (via `integration_for(kind)`) owns each kind's resume
template, record scanning and parsing, and hook argv. `ScreenReader` (via `reader_for(kind)`, in
`agent_kind/screen_reader.rs`) owns status reading; Claude and Codex have dedicated readers and everything else gets
the generic change-counting reader. The ticker and status code call only `reader_for` and never compare kinds, so the
status-tracking work that landed in #1254 is the cleanest part of the picture, not the problem.

## Where the harness-specific branching lives

Helm, `crates/farhelm-helm/src/launches.rs` (about 45 sites). `compile()` builds the argv with a few
`if selection.harness == Goose/Grok/Omp` checks plus one exhaustive match per flag (model, effort, yolo, trust).
`program()` maps harness to binary name, four arms are `unreachable!()` because a harness has no such flag, and Grok's
resume template is built inline. The `*_EFFORTS` constants beside `CATALOG` are per-model release catalog data; the UI
never reads them directly and instead derives a harness's efforts from the catalog the helm serves
(`compatible_efforts`, `launch_composer.rs:1267`).

Supervisor, `crates/farhelm-supervisor/src/service/core.rs` (about 24 sites), all integration behavior that lives
outside `AgentIntegration`:

- `with_hook_argv_using` (2530 onward) is a run of `if snapshot.kind == Goose/Omp/Pi/Claude/Codex` blocks. They do not
  share one check order: Goose, OMP, and Pi check the invocation's shape before the `FARHELM_AGENT_HOOKS` opt-out, and
  Goose, when resuming with hooks disabled, still inserts `session` into the argv and writes environment controls that
  turn the persisted reporter off (2550–2598).
- The Pi/OMP extension asset is materialized at spawn (13235–13258) before the injection decision (13260), so a Pi
  utility command writes the asset even though nothing gets injected. OMP's launch provenance is recorded even when
  injection is skipped (around 13343).
- Report-only capture refresh (`matches!(kind, Codex | Grok)` and a `_ => Ok(true)` arm, around 6423), the
  Codex-only resume refusal (10351), the `matches!(Pi | Omp | Grok)` verification gate (10419), Grok resume
  verification and the Pi/OMP → `LocatorVendor` match (around 6602), the foreground-report dispatch over Codex, Grok,
  and OMP with a documented deny-by-default `_` refusal (14183–14212), and Claude's kernel-attributed peer check (14764).

Supervisor, elsewhere:

- `agent_kind/mod.rs` has about 70 production variant references, the most on the supervisor side, and they are the
  resume-target logic: `IntegrationSnapshot::resolve` rejects ambiguous OMP and Grok resume templates (1859, 1865),
  `restart_offer` branches on Codex, Grok, Pi, and OMP (1925–1953), and `filled_resume_argv` (2004–2030) ends in a `_`
  arm that treats any other kind's conversation as a bare id. `ownership_proof_implemented` (2060) is already an
  exhaustive central policy function.
- `service/capture.rs:790` gates the report-only refresh pass on a hand-written `matches!` list of every integrated
  kind; a new kind is silently skipped.
- `service/handlers.rs:3429` and `3447` check Codex and OMP foreground-source vocabulary at the doorway.
- `procs.rs` recognizes agent runtimes by byte-string executable names (`is_other_session_runtime`, around 466) and
  holds distinct Codex, Claude, Grok, and OMP process-ancestry policies (around 486, 563, 623, 951). None of that uses
  the enums, so an enum-based search does not see it.

UI:

- `launch_composer.rs`: display names, four `harness == Grok` checks that all say "Grok has no model or effort", and
  `normalized_permissions`.
- `launch_controls.rs`: a separate hardcoded "no effort" list (`matches!(OpenCode | Cursor | Grok)`, 275), a Grok check
  (154), Pi's permission control (306), and Muse/Codex trust help text (371–375).
- `session_view.rs:84` (Muse, Cursor, and OpenCode cannot resume), `create_form.rs:2046` (a Cursor notice), and the
  glyph mapping in `list/row.rs`.
- Three hand-written harness arrays: `launch_composer.rs:329` (deliberately without Grok), `launch_composer.rs:670`,
  and `create_form.rs:4344`. They are the worst drift hazard in the codebase: a new harness is silently missing from
  the picker, search, and model list with no compile error. `LaunchHarness` has no `ALL`; `LaunchPermission` does.
  The two full arrays also list harnesses in different orders, so display order is a decision, not an accident.

Pi's permission rule is not one rule. The helm's `normalize_selection` (`launches.rs:326`) fills in YOLO only when the
permission is omitted and then rejects unsupported explicit choices; the UI's `normalized_permissions`
(`launch_composer.rs:74`) and `row.rs:528` force YOLO whatever was stored. A single shared function would change
behavior in one place or the other.

`crates/farhelm/src/hook.rs` has about 12 production `ReportVendor` references, with Grok's three-event protocol
isolated in `parse_grok_payload`. It is already contained.

Most of the harness matches are exhaustive, so a new variant produces compile errors. The holes are the `==` special
cases, the `_` arms, the `matches!` allowlists, the hand-written arrays, and the byte-string lists in `procs.rs`: in
all of those, a new harness silently takes the "not that one" branch or is left out.

## The first draft, and what the reviews found

The first draft proposed five items: a proto `HarnessSpec` data record per harness (absorbing display names, program
names, effort tables, and capability flags); per-harness argv builders behind a generic `compile()`; moving supervisor
decisions into integration methods, with hook injection returning a `Leave(reason) | Inject { argv tail, env, asset }`
plan that `core.rs` applies generically, capture behavior declared as one per-integration "capture mode", and the
async per-vendor bodies moved to `service/vendor/<kind>.rs`; splitting `hook.rs` per vendor; and a source-check script
rejecting specific-variant comparisons outside designated modules. It estimated medium-high effort.

Two reviewers with fresh context (Claude Opus 5.5 and GPT-6 Astra at high effort, run independently) both concluded the
refactor is worth doing only in reduced form. They agreed on these points, and I spot-checked each against the code:

- The survey missed the resume-target logic in `agent_kind/mod.rs`, the `capture.rs` allowlist, the `handlers.rs`
  doorway checks, several UI sites, the hand-written arrays (Opus), and the `procs.rs` byte-string policies (Astra).
  All of that is folded into the survey above.
- The generic hook plan does not fit. Goose's disabled-resume rewrite cannot be expressed as `Leave` or `Inject`, the
  opt-out cannot be applied generically without reordering checks and changing logged skip reasons, and returning the
  asset from the plan would reverse the materialize-then-decide order.
- Capture is several independent policies (scan eligibility, report admission, readiness refresh, resume verification,
  provenance), not one mode, and the claimed/unclaimed entry points exist because the capture mutex is not reentrant.
- Effort tables are release catalog data and belong in the helm. Display labels are UI vocabulary.
- Moving the async bodies into `impl Supervisor` blocks in other files is file organization, not a boundary: they keep
  full access to supervisor internals.
- `hook.rs` should be left alone.
- The source-check script as specified would be noisy (legitimate re-checks under a claim, the documented deny-by-default
  arm) and would still miss `ReportVendor`, the byte-string lists, and vendor-specific function calls.

They disagreed on the helm compiler. Opus judged per-harness builders not worth it: every per-flag match is already
exhaustive, and a builder interface would need about seven hooks (env prefix for Goose, post-program subcommand for
Goose's `session` and Grok's `--no-leader`, model, effort, OMP's approval mode, yolo, trust, resume template) to cover
nine harnesses. Astra judged it worthwhile if each harness owns its whole argv construction through one plain
function, with validation and quoting shared, rather than through callbacks.

## The maintainer's answers

After the reviews, the maintainer settled the open questions (2026-09-29):

- The goal is a stronger boundary plus compile errors. Concretely: it should be possible to look in a small number of
  places and see all of, say, the Codex-specific behavior, and an agent adding or changing harness behavior should not
  introduce drift because it failed to find one of many scattered special cases.
- The helm compiler: whichever is simplest, erring on less refactoring for now. That means leaving `compile()` as it is.
  Its per-flag matches are exhaustive and all live in one file, so the helm's Codex behavior is already in one place.
- No new harness is intentionally planned, but more per-harness functionality is likely. The screen readers that landed
  in #1254 are the example: a new per-harness capability that had to find a home.

The first answer changes the reviews' verdict on one point. Both reviewers called moving the supervisor's async
per-vendor bodies into per-kind files "file organization, not a boundary", and that is accurate as far as encapsulation
goes: the bodies keep full access to supervisor internals. But findability is exactly what the maintainer asked for,
so that move is back in scope. The third answer argues for giving new per-harness capabilities a documented home, the
way `screen_reader.rs` became one, so the next one does not land as `if kind == X` in `core.rs`.

## The proposal

In order, each step behavior-preserving, with the option of stopping after any of them:

1. Enumerations. Add `LaunchHarness::ALL` and `AgentKind::ALL`, replace the three hand-written UI arrays (with an
   explicit display order where the UI wants one different from declaration order), and make `capture.rs:790` ask
   `integration_for(kind).is_some()` instead of listing kinds. Small, and it closes the silent-omission holes.
2. Launch predicates in proto, as exhaustive `const fn`s beside the existing ones rather than a data struct: roughly
   `offers_model()`, `offers_effort()` where the catalog does not already answer it, and Pi's implied permission as a
   fact that the helm (fill an omission) and the UI (force it) each apply with their current semantics. Route the Grok
   checks, the `launch_controls.rs` lists, `session_view.rs`'s resume list, and `row.rs` through them. The yolo-guard
   TODO needs the implied-permission fact to decide what counts as a yolo launch.
3. Exhaustive resume dispatch in `agent_kind/mod.rs`: replace the `_` arms in `filled_resume_argv`, `restart_offer`,
   and related locator logic with explicit per-kind arms (or integration methods), so a new kind is a compile error on
   the path where a mistake resumes the wrong conversation.
4. Hook injection per kind. Move each kind's block from `with_hook_argv_using`, together with its shape helpers
   (`goose_launch_shape`, `pi_interactive_invocation`, `codex_invocation_configures_hooks`), into
   `agent_kind/<kind>.rs` as a pure whole-argv transform that receives the already-resolved policy (opt-out,
   executable path, extension, instructions), reached through one exhaustive match. `omp::omp_injection_decision` is
   the precedent. Each kind keeps its own check order and skip reasons, and asset materialization stays where it is.
   The roughly 37 existing `with_hook_argv_using` tests are the characterization suite.
5. Per-kind homes for the rest of the supervisor's vendor code, as pure code motion:
   - The async bodies in `core.rs` (`refresh_codex_capture_claimed` at 6456, `refresh_grok_capture_claimed` at 6531,
     `report_codex_conversation` at 14221, `report_grok_conversation` at 14374, `report_omp_conversation` at 14532,
     Grok resume verification, and similar) move to one file per kind under the service module, as `impl Supervisor`
     blocks. The call sites become one exhaustive dispatch per operation, with an explicit refusal arm for each kind
     that has no such behavior in place of today's `_` arms (the documented deny-by-default stays deny-by-default).
   - The per-vendor process-ancestry corridors in `procs.rs` (`codex_corridor`, `claude_corridor`, `grok_corridor`,
     `omp_corridor` and their image and launcher helpers, about 600 lines) are already pure functions over a process
     chain; they move to per-kind modules, and `is_other_session_runtime`'s byte-string list is derived from one table
     of runtime names.
   - The Codex and OMP vocabulary checks in `handlers.rs` move next to the rest of each kind's report admission.
   Whether the pure halves (`agent_kind/<kind>.rs`) and the supervisor-bound halves share a directory per kind or sit in
   two parallel per-kind trees is an implementation choice; either way it is two predictable files per kind, not a
   search.
6. Make the boundary discoverable and enforced:
   - A map of where harness-specific behavior lives, in each layer, and where new per-harness functionality goes, in
     the `agent_kind` module doc and as a short section in the repo's agent instructions (the same shape as the
     existing "Harness marks in the sidebar" section). This is the part that stops an agent from adding a new special
     case in `core.rs` because it did not know where the others are.
   - A scoped `#[warn(clippy::wildcard_enum_match_arm)]` on the dispatch modules, so a new `_` arm there fails the
     Clippy gate.

Deferred or dropped: per-harness argv builders in the helm (the maintainer chose the simplest option), a single
"capture mode", the `hook.rs` split, and the source-check script. The script can be reconsidered later in a narrower
form if drift shows up anyway. Before any step changes capture decisions rather than moving them, Astra's suggestion
applies: write down the five capture policies (scan eligibility, report admission, readiness refresh, resume
verification, provenance) per kind first. None of the steps above is meant to change a decision, only where it lives.

Rough effort: medium. Steps 1, 2, and 6 are mechanical. Step 3 and step 4 sit on the conversation-identity path and need
care, with existing tests to lean on. Step 5 is large but mechanical code motion; the risk is in keeping the capture
claim discipline intact (the claimed/unclaimed entry points exist because the capture mutex is not reentrant), not in
the logic.

The host-kind capability TODO applies the same pattern (named predicates instead of kind comparisons) to `HostKind`, so
the two can share conventions.
