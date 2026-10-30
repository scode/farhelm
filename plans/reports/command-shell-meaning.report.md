# Commands and wrappers with Bash meaning

## What this was about

A command beginning with `~/` failed because Farhelm treated the program name as literal text. The maintainer chose full
Bash interpretation for custom commands and their resume commands, plus agent wrapper prefixes on hosts and inside
containers, with a breaking change and no compatibility mode or migration. Only agent launches can be containerized;
their wrapper text is interpreted inside the container.

The two draft PRs implement that choice. Commands retain their original comments, newlines and quoting except where
Farhelm safely fills placeholders. Wrapper editors retain typed quotes and escapes; templates, remembered prefixes,
approvals, CLI listings and retry identity carry the same words. Farhelm's composed agent arguments, model names,
conversation values and initial prompts remain literal data. Ordinary agent launches without a wrapper and legacy
sessions retain direct execution. The final PR removes the original TODO entry and extends the same breaking fragment.

## Things you should know

- Existing custom commands and saved wrappers acquire shell meaning. `$VARIABLE`, `*`, `;`, `>`, `&`, `|` and
  backslashes can therefore change what an existing launch does. There is deliberately no migration or compatibility
  detection.
- Command placeholders must be separate, unquoted words. Native save/admission and launch refuse quoted, escaped or
  glued forms. Older rows remain readable, but an unsafe old command is refused when it next starts. Placeholder values
  and the agent arguments appended to a wrapper are safely quoted.
- A program missing inside a Bash command exits normally, usually with code 127 and Bash's terminal diagnostic. It does
  not become Farhelm's launch Error. A missing Bash, or a missing program in an unchanged direct launch, still uses the
  existing launch-failure diagnosis.
- Plain noninteractive Bash inherits `BASH_ENV` and exported Bash functions. The plan's blanket statement that functions
  remain unavailable was inaccurate; execution follows the explicit `bash -c` choice without adding environment
  scrubbing or changing the launch shim. Ordinary interactive/login startup files are not read again, and ordinary
  unexported functions and aliases are not inherited.
- A compound command or prefix can leave Bash running above a resident wrapper. Claude, Muse and Codex tolerate that
  extra process; Grok and OMP can stop following the conversation, leaving Restart unavailable. The specs and wrapper
  guide document this existing process-tracking limit; no new attribution machinery was added.
- The native placeholder check uses a Bash syntax tree to locate safe source spans, including actual nested backtick
  parsing stages. It keeps the existing literal argument projection for conversation tracking and Restart. The parser is
  excluded from the browser build. No storage format or shim responsibility changed.

## Open questions and possible follow-ups

No required decision or omitted acceptance criterion remains. The intentional breaking behavior and
conversation-tracking limits above are the main points to review before using existing commands and wrappers with this
change.

## The PRs

- [#2847: custom commands through Bash](https://github.com/scode/farhelm/pull/2847/changes), first draft, head
  `12119a867110a3dc2d36082de35f468c4da4c7a6`.
- [#2848: wrapper prefixes through Bash](https://github.com/scode/farhelm/pull/2848/changes), second draft, head
  `5c2fe13addd44f7538ccf436ffe470bfb366a5ab`, based on #2847. It includes the final TODO removal.

## Checks run, reused and skipped

Runtime commands use the recorder, pinned nextest, four slots and zero retries, with pinned tmux where needed. Browser
validation uses both configured engines, three workers and zero retries. IDs name retained private recorder records;
selected-out tests are not coverage.

- PR1 helper and command-shape selection:
  `cargo nextest run -p farhelm-proto --lib -E 'test(command_shell::) or test(command_launch_rules_hold)'`. Final run
  `2e9803be-f179-4af9-bd72-dbac003885d0` passed 7 cases, 194 selected out. Source fingerprint
  `1a96182779a95ea54c732907f517643c25ca0b7a1d14af198f2adaa5ca065e1b`, based on
  `5285ad9b700b3dd57680af6c16c1612f11e2b148`. Reused after the metadata-only rebase because the helper behavior was
  unchanged.
- PR1 command execution and direct-launch sentinel selection:
  `cargo nextest run -p farhelm --test e2e -E 'test(command_shell::) or test(launch_sentinel_error_status::) or test(=restart_with_resume::a_restart_clears_a_previous_launch_error) or test(=session_lifecycle::create_attach_and_roundtrip_input)'`.
  Run `18059f0b-c26d-4068-88df-365651d13e13` passed 17 tests and one fixture setup, 397 selected out. Source fingerprint
  `01707c95c13c4d50f833ab6abe525b17d4112cc5047484127b3da98b2275ce38` on the same base. This proves original source
  expansion, hostile directory values, inherited Bash startup inputs, missing-inner-program exit 127, Restart and
  Restart with, and unchanged direct-launch errors. Reused after a later refusal-only marker-discovery correction and
  the rebase: these valid execution paths were unchanged; the final helper selection covered the new refusal cases.
- PR2 raw-wrapper units:
  `cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-supervisor -p farhelm-ui --lib -E 'test(wrapper) or test(permission_and_word_facts_follow_valid_submitted_values) or test(ordinary_owner_gates_all_entries_and_keeps_equal_choice_completion) or test(request_assembles_host_raw_placement_and_structured_one_time_inputs) or test(command_shell_spec_preserves_source_and_quotes_filled_values)'`.
  Run `0b6e62f6-dc6c-425c-964f-4c87d3f50af6` passed 40 cases, 3712 selected out. Complete source fingerprint
  `9e81d8b2fa23efc9de89d5e9a0211bb2b3fc4d42d6d77dbd9c40f1233cbe0a44`, based on the first draft head. Later comments and
  CLI/e2e-only changes leave this library coverage intact. It includes actual Bash suffix quoting, raw-word
  storage/display, approvals, retry identity, OMP provenance and the paired command launch spec.
- Combined-tip command units:
  `cargo nextest run -p farhelm-proto -p farhelm-supervisor --lib -E 'test(command_shell) or test(session_launch::tests::) or test(farhelm_args) or test(an_old_quoted_command)'`.
  Run `ed18832c-a192-4ee0-9fc5-9914e265a3a9` passed 16 cases, 1576 selected out. Complete source fingerprint
  `13144b87b9ddc8eb6aa9bc6cb616fa969fcc54371d394196804c01ca72457b2c`, based on the first draft head; complete
  report/output and normal cleanup, with no runtime `SKIPPED` messages. This clean command replaces the initial failed
  combined-unit gate and checks old-row loading, hook arguments and command launch values at the combined tip.
- Host wrappers and CLI:
  `cargo nextest run -p farhelm --bin farhelm --test e2e -E 'test(wrapper_launch::) or test(=tests::wrapper_flag_is_a_shared_launch_edit) or test(=render::tests::a_templates_sets_cell_reads_as_its_flags)'`.
  Run `93b50615-92f2-4ea2-961d-e11cd18ae2ed` passed 8 tests plus one fixture setup, 594 selected out.
- Container wrapper:
  `cargo nextest run -p farhelm --test e2e -E 'test(=containerized_sessions::podman_wrapper_expands_inside_the_container_and_quotes_the_agent_suffix)'`.
  Exact repetition `88bb1abb-1be2-4cfe-82c8-07a3c191aaad` passed one test plus fixture setup, 414 selected out. It
  exercised real rootless Podman and the offline Ubuntu image, verified container HOME/token expansion, exact literal
  model/prompt arguments and owned Stop/Delete cleanup. Both execution runs have complete source fingerprint
  `13144b87b9ddc8eb6aa9bc6cb616fa969fcc54371d394196804c01ca72457b2c`, based on the first draft head, complete
  reports/output and normal cleanup, with no runtime `SKIPPED` messages.
- Browser run `fe8c3943-2a16-4513-b06b-d7a981f8d791` passed 20 cases: 10 Chromium and 10 WebKit, with no skips,
  failures, retries or unstarted cases. It selected the five files below, filtered to native quoted command-placeholder
  refusal, launcher tabs/raw wrapper entry, remembered wrappers, mixed template shortcuts, wrapper quick launch and
  partial Restart with templates:

  ```sh
  npx playwright test 'launch-refusal\.spec\.ts|launcher-tabs\.spec\.ts|launcher-template-shortcuts\.spec\.ts|quick-launch\.spec\.ts|restart-with\.spec\.ts' -g 'quoted command placeholder|New opens on the agent tab|agent wrapper input|dormant invalid wrapper|agent switch loads|arrow keys move|search on the command tab|quick launch submits the wrapper|typed launcher words mix|restart with applies an allowed partial agent template'
  ```

  Source fingerprint was the same final `13144b87...` working tree, with complete source, reports, three-worker policy,
  output and normal cleanup. All three owned stacks shut down.
- Current static checks passed: `cargo check -p farhelm-supervisor -p farhelm-helm -p farhelm-ui --tests`,
  `cargo fmt --all -- --check`, `dprint check` on changed Markdown, `python3 releasing/check-changelog.py format`, and
  the isolated interpreter's `python -B scripts/check-test-sleeps.py` (327 delays, zero unannotated).
- PR2
  `cargo clippy -p farhelm-proto -p farhelm-supervisor -p farhelm-helm -p farhelm-ui -p farhelm --all-targets -- -D warnings`
  passed. The release web build with pinned Dioxus 0.7.10 passed in 221 seconds. Shipped-binary Clippy
  (`cargo clippy -p farhelm --bins -- -D warnings`) and `cargo build -p farhelm` also passed. Both application builds
  used the same frozen source as the final execution checks; no build writes ran during browser validation.
- PR1 `cargo check -p farhelm-proto --target wasm32-unknown-unknown --no-default-features` passed, proving the native
  parser is excluded. The final web build also compiled the changed UI and proto paths for WASM.

Retained development failures remain distinct from passes. Initial combined command-unit run
`5d20196d-567c-4be0-92e1-9d97e3d2fe1f` had 15 passes and one failure: native syntax validation obscured the existing
specific program-placeholder refusal. Restoring shape checks before placement checks fixed it; exact run
`233d6921-0fc5-434a-a154-ca0e63a692f2` passed 2 cases, followed by the final 7-case helper selection. The failed command
is not a passing reuse gate. First combined host/container/CLI run `c8b27ec8-9f8b-44a7-b081-e23510f513c6` had 8 test
passes and one failure before the container agent started: the new fixture's HOME override sent Podman to a different
rootless image store. Removing that fixture override preserved the host image store; the distinct token still proves
expansion happens inside the container. The exact container repetition and separate clean host/CLI gate above passed. An
earlier source-only sleep check hit its 60-second budget during compilation; subsequent checks passed. A PATH collision
selected an unrelated `dx` before any web build; rerunning with the verified pinned Dioxus executable passed. These
same-session corrections were retained privately rather than recorded as latent flakes.

No full test battery, hosted CI, desktop/installer/release runtime, website preview, deployment or live-install action
was needed. Website edits are incidental prose with unchanged links and headings. Targeted execution and browser checks
cover the launch behavior and edited UI paths; native parser exclusion is covered by the WASM check and web build.

## Review gate outcome

Each PR used one fresh `gpt-6.1-sol` reviewer at high effort, followed by reassessment by that same reviewer. PR1's
placeholder-discovery and environment-documentation findings were accepted and fixed. PR2's three CLI omissions and
stale spec summaries were accepted and fixed, including the final agent-help paragraph. Both source reviews passed;
there were no rejected findings. Own-model scope checks constrained the native parser adapter and removed heuristic
marker discovery rather than growing a separate shell parser. Commit/PR wording passed fresh cold reads. The final
report passed an independent cold read.
