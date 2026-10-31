# Commands and wrappers with Bash meaning

## Since the last review

Both existing draft PRs changed; none were added or dropped. The custom-command PR (#2847) corrects the two supervisor
tests that still expected direct execution, makes a bad resume command's error name the resume command, and fixes the
CLI help. The wrapper PR (#2848) now explains the consequences for saved wrappers and the container image's Bash
requirement. The original launch design and breaking-change decision are unchanged.

The requested whole-supervisor, launch/Restart/container and 206-case both-engine browser checks passed, as did static
Linux and Bash 3.2 compatibility. Upstream then landed the navigation rail, template Save controls and a docs refresh.
The rebase preserved the native source exactly and kept both sets of UI and documentation changes. All eight fresh
browser interaction cases also passed on the rebased UI.

## What this was about

A command beginning with `~/` failed because Farhelm treated the program name as literal text. The maintainer chose full
Bash interpretation for custom commands, their resume commands and agent wrapper prefixes, on hosts and inside
containers, without a migration or compatibility mode.

The two PRs implement that choice. Commands keep their original comments, newlines and quoting except where Farhelm
safely fills placeholders. Wrapper editors retain typed quotes and escapes, including in templates, remembered setups
and approvals. Farhelm's composed agent arguments, model names, conversation values and initial prompts remain literal
data. Ordinary agent launches without a wrapper and legacy sessions keep direct execution. The second PR removes the
original TODO entry.

## Things you should know

- Existing commands and saved wrappers acquire shell meaning. Previously saved wrappers lost their quotes and escapes:
  `--profile 'work space'` was stored without the quotes and now passes `work` and `space` separately. Saved `$`, `*`
  and quote characters gain Bash meaning; a word starting with `#` comments out the rest of the agent command. Saved
  wrappers may need checking and re-entry. This includes templates already approved for agents, remembered setups and
  existing wrapped sessions on their next start or Restart. There is no automatic conversion or compatibility detection.
- Wrapped container launches need `bash` in the image, even without opening a terminal tab. The breaking fragment and
  Containerized sessions page now say so. Command launches themselves cannot be containerized.
- Command placeholders must be separate, unquoted words. Native save/admission and launch refuse quoted, escaped or
  glued forms. Older rows remain readable, but unsafe commands are refused when next started; Restart checks before
  stopping the old agent. Filled values and composed agent arguments are safely quoted.
- A program missing inside the Bash line exits normally, usually with code 127 and Bash's terminal message, rather than
  becoming Farhelm's launch Error. A missing Bash or a failed unchanged direct launch still uses the existing diagnosis.
- Noninteractive Bash inherits explicit `BASH_ENV` and exported Bash functions. The plan's blanket no-functions wording
  was inaccurate; the explicit `bash -c` decision was kept without adding environment scrubbing. Ordinary login/startup
  files are not read again, and unexported functions and aliases are not inherited.
- A compound command or prefix can leave Bash above a resident wrapper. Claude, Muse and Codex tolerate it; Grok and OMP
  can stop following the conversation and leave Restart unavailable. No new process-attribution mechanism was added.
- The native placeholder check uses a Bash parser while conversation tracking and Restart retain literal arguments. The
  parser is excluded from the browser build. No storage format or launch-shim responsibility changed.

## Open questions and possible follow-ups

No required decision or omitted acceptance criterion remains. Native macOS execution and a full CentOS provisioning run
were not performed; the compatibility checks below describe their narrower coverage.

## The PRs

- [#2847: custom commands through Bash](https://github.com/scode/farhelm/pull/2847/changes), first draft, head
  `b2a6a444eb104a39128077b778d910d5a478fdca`.
- [#2848: wrapper prefixes through Bash](https://github.com/scode/farhelm/pull/2848/changes), second draft, head
  `74220d6b2336e02ae2f5f63d7cdafb0b045f468d`, based on #2847 and including the final TODO removal.

## Checks run, reused and skipped

Rust runtime selections use the recorder, pinned nextest 0.9.148, four slots and zero retries, with pinned tmux 3.7c
where needed. Browser runtime uses both engines, three workers and zero retries. Recorder IDs identify retained private
records; selected-out or runtime-skipped tests are not coverage.

- Full `cargo nextest run -p farhelm-supervisor`, run `2cd217ff-371a-4b01-8183-ce83e86b55e7`, passed all 1,385 executed
  cases, including the two returned assertions and the Start/Resume error-label regression; five subprocess helpers were
  intentionally ignored. Complete report and source fingerprint
  `bfd57a102f03616554c891cd2d4b1e79ad640c58c30315faefc9c79a917bb907`, based on rebased head
  `a9a8204e07dd5059b99727b681b2f3b60da65cc2` plus this round's corrections; output EOF, no truncation, normal cleanup
  and no runtime `SKIPPED` message.
- `cargo nextest run -p farhelm --test e2e -E 'test(/command_shell::|structured_launches::|wrapper_launch::|launch_sentinel_error_status::|session_lifecycle::|restart_with_resume::|restart_under_concurrency::|containerized_sessions::/)'`,
  run `49544beb-227c-4c51-a7ee-89e4ce91b88f`, passed 163 tests plus fixture setup, with 252 selected out. Real rootless
  Podman cases executed, including wrapper expansion inside the container and owned Stop/Delete cleanup. Command
  expansion, launch failures, host wrappers, Restart and Restart with also passed. Complete report and the same complete
  `bfd57a10...` source, output EOF, no truncation, normal cleanup and no runtime `SKIPPED` message.
- `npx playwright test 'quick-launch\.spec\.ts|launcher-enter\.spec\.ts|replace\.spec\.ts'`, run
  `42a1cc5b-7a9b-452f-b9ec-229822917c54`, passed 206 cases: 103 Chromium and 103 WebKit, with no failures, skips,
  retries, interruptions or unstarted cases. Complete same-source `bfd57a10...` identity, report and validated policy,
  output EOF, no truncation, normal cleanup and all three owned stacks shut down. This full command ran before the
  navigation-rail/template-controls rebase. It remains evidence for unchanged launch/request behavior; the fresh
  interaction selection below checks the new UI rather than claiming all 206 cases ran again.
- After the fresh web build:
  `npx playwright test 'templates\.spec\.ts|quick-launch\.spec\.ts|launcher-enter\.spec\.ts|launcher-tabs\.spec\.ts' -g 'template Save follows unsaved changes and retains focus after saving|quick launch submits the wrapper prefix saved in its agent template|valid wrapper spacing preserves raw text and retry words|agent wrapper input'`.
  Run `2874b207-2036-4b02-af9e-7812e02bf520` passed all eight cases, four per engine, with no failures, skips, retries
  or unstarted cases. They cover the new Save equality guard with the raw wrapper editor, quoted-word entry/refusal,
  quick launch and ordinary focus/retry handling. Complete source at final head `74220d6b...`, fingerprint
  `e91dd84941b787819d4e968acbeb97a74236e937a1b1c3b697f4b2c3782529ff`, complete report/policy, output EOF, no truncation,
  normal cleanup and all three stacks shut down.
- The exact static payload command from the CentOS gate,
  `cargo build -p farhelm --target x86_64-unknown-linux-musl --locked`, passed in run
  `7701f47c-3523-4880-a585-c3af94ab56e2`, compiling tree-sitter 0.27.1 and tree-sitter-bash 0.25.1. Its static-ELF
  check, `--version` and an owned Bash launch through the real binary's shim passed in
  `df8c6d85-fe25-4c38-8cfd-4b242d0513a8`. Both have complete source identity, output EOF, no truncation and normal
  cleanup. They cover the unchanged parser/dependencies at rebased head `a9a8204e07dd5059b99727b681b2f3b60da65cc2`,
  fingerprint `d5f9f2330e8a3174fdf58b556f52722b79aca81316fea9e0a648b647b8e7d792`; later corrections change diagnostic
  labels, test expectations, CLI help and prose only. This is the gate's development-profile static payload, not an
  optimized release build or full CentOS SSH/provisioning run. The shim smoke does not separately prove parser
  validation.
- Real renderer and shared quoting APIs passed 37 executable command/resume/wrapper cases under GNU Bash 3.2.0 on Linux,
  run `3c387661-1a13-42a2-8566-163b6d15a6c7`: tilde program/argument expansion, variables, globs, operators, newlines,
  comments, every placeholder, hostile literal values and nested substitutions/backticks. The raw wrapper join mirrors
  the private supervisor helper while using the real splitter and quoting API; application e2e tests prove its wiring.
  The frozen source copies were byte-identical to the combined stack's proto files. Complete output, EOF, no skips,
  normal cleanup and the same complete parent-source fingerprint as the supervisor run. The signed original GNU Bash 3.2
  build passed separately in `cce8dcc5-56ef-4171-91b1-ec5a3402a184`. No generated syntax in these paths needs newer
  Bash; syntax the user types still depends on the installed Bash version. This does not claim macOS execution or a
  macOS build of the parser library.
- The pinned Dioxus 0.7.10 release web build passed in 219 seconds; the browser parser exclusion also compiled. The
  explicit `cargo build -p farhelm` also passed. Format, changed-doc dprint, changelog format and source-only sleep
  checks passed (327 delays, none unannotated).
  `cargo clippy -p farhelm-supervisor -p farhelm --all-targets -- -D warnings` and
  `cargo clippy -p farhelm --bins -- -D warnings` both passed, including the shipped configuration without test seams.
  The actual `farhelm agent create --help` output also describes Bash expansion and operators, run
  `ad5d0bf9-110a-4bd0-b22d-3c1be5482c86`; complete same-source identity, output EOF and normal cleanup.

The careful rebase inspected the full intervening diff onto `8b70e46f7dbdf126c0f4e50adee36d6db5952305`. The only text
conflict was adjacent TODO removals; both upstream's completed-template removals and this plan's completed tilde entry
were removed. Native supervisor/helm/proto/CLI files, Cargo manifests and lockfile compare byte-for-byte equal to the
tested source. Each new supervisor, e2e, Bash, static-build/run, native lint and CLI-help command above therefore
remains applicable. The server's build version is unchanged too, so the matching native executable is reused. The web
bundle was rebuilt successfully in 164 seconds for the changed rail and template controls. The template editor already
compares raw wrapper words and unfinished input with its saved baseline; upstream's new Save guard uses that same
equality. Both original reviewers reassessed the rebase and found no conflict.

Main's later change through `e9095ef8463dbd438a4a36d5617a3138a564b364` only adds an optional Tensorlake test harness and
its instructions. Its complete diff was inspected; no product, dependency or input to these local checks changed, so it
called for no further rebase or runtime repetition.

Successful earlier commands reused individually:

- `cargo nextest run -p farhelm-proto --lib -E 'test(command_shell::) or test(command_launch_rules_hold)'`, run
  `2e9803be-f179-4af9-bd72-dbac003885d0`: seven passes, 194 selected out, fingerprint
  `1a96182779a95ea54c732907f517643c25ca0b7a1d14af198f2adaa5ca065e1b` on base `5285ad9b700b3dd57680af6c16c1612f11e2b148`.
  The final parser/helper behavior is unchanged by this round or the rebase.
- `cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-supervisor -p farhelm-ui --lib -E 'test(wrapper) or test(permission_and_word_facts_follow_valid_submitted_values) or test(ordinary_owner_gates_all_entries_and_keeps_equal_choice_completion) or test(request_assembles_host_raw_placement_and_structured_one_time_inputs) or test(command_shell_spec_preserves_source_and_quotes_filled_values)'`,
  run `0b6e62f6-dc6c-425c-964f-4c87d3f50af6`: 40 passes, 3712 selected out, fingerprint
  `9e81d8b2fa23efc9de89d5e9a0211bb2b3fc4d42d6d77dbd9c40f1233cbe0a44` on base `12119a867110a3dc2d36082de35f468c4da4c7a6`.
  Raw wrapper storage/display, approvals, retry identity, literal suffix quoting and OMP provenance are unchanged. The
  newly extended command-spec error assertion also passed in the fresh full-supervisor run.
- `cargo nextest run -p farhelm --bin farhelm --test e2e -E 'test(wrapper_launch::) or test(=tests::wrapper_flag_is_a_shared_launch_edit) or test(=render::tests::a_templates_sets_cell_reads_as_its_flags)'`,
  run `93b50615-92f2-4ea2-961d-e11cd18ae2ed`: eight tests plus fixture setup passed, 594 selected out, fingerprint
  `13144b87b9ddc8eb6aa9bc6cb616fa969fcc54371d394196804c01ca72457b2c` on that same first-PR base. CLI parsing and
  template rendering are unchanged; the current CLI edit changes help prose only. Host wrappers are also covered by the
  fresh e2e selection.
- The earlier 20-case browser command, run `fe8c3943-2a16-4513-b06b-d7a981f8d791`, passed ten Chromium and ten WebKit
  cases at that same `13144b87...` source. It selected command-placeholder refusal, launcher tabs, raw wrapper entry,
  remembered wrappers, template shortcuts, wrapper quick launch and partial Restart with templates across
  launch-refusal, launcher-tabs, launcher-template-shortcuts, quick-launch and restart-with specs. Those UI paths are
  unchanged; upstream navigation-rail wording, this round's resume diagnostic, CLI help and prose do not invalidate
  their coverage. The fresh requested three-spec run is reported separately above.

All four reused records still have complete source/reports, output EOF, no truncation, normal cleanup and no runtime
`SKIPPED` marker. Selected-out tests are not passes. The new round's complete source and the old successful commands
were checked individually; failed overall commands are not reused as gates.

Three private compatibility setup failures were retained: `981d8dc1-54ff-494b-a56d-b59d6e141a58` (launcher argument
splitting), `c40aa626-7f94-407e-979b-f3e6412127b1` (patched Bash archive needed unavailable yacc), and
`75d0b811-f4cd-4dff-819e-da79819f7fa9` (private Bison configure needed unavailable M4). Correcting the launcher and
using the signed original Bash 3.2 release led to the successful build and probe above. These were setup failures, not
Farhelm regressions. Original implementation failures also remain retained: `5d20196d-567c-4be0-92e1-9d97e3d2fe1f`
exposed a validation-order error corrected before the final helper selection, and `c8b27ec8-9f8b-44a7-b081-e23510f513c6`
exposed a test HOME override selecting the wrong Podman store; removing that override preceded clean execution gates.

No whole-workspace battery or full browser suite was repeated: the latest Decision requires the supervisor package,
launch/Restart/container modules and the three named browser specs. The earlier landing-wide Rust command,
`cargo nextest run --workspace --exclude farhelm-desktop`, run `7ec0376d-1c10-4848-877a-ec3ea8d4c44c`, failed the two
supervisor assertions that still expected direct execution. Those assertions are corrected and passed in this round's
complete supervisor run. The earlier full browser run, `afafdf3a-2c2c-4d7f-8ef0-28e9e3706ad0`, had 11 failures while the
machine was heavily loaded, including quick-launch, launcher-enter and Replace cases. This plan's latest Decision
specifically required those three complete specs on both engines; all 206 cases passed, followed by the eight-case
post-rebase UI check. That does not diagnose or clear the unrelated failures from the earlier full-suite observation,
and this report makes no claim that those other cases were rerun. Neither failed overall command is treated as a pass.
Hosted CI, desktop/installer runtime, full CentOS provisioning and native macOS checks were skipped because they add
different coverage from the requested checks. No website preview, deployment or live-install action ran. The website
change is incidental prose with unchanged links and headings; the Containerized sessions page is the changed page.

## Review gate outcome

Both fresh single `gpt-6.1-sol` high-effort PR reviews and their post-rebase reassessments passed with no actionable
findings. The original corrected trees matched the amended commits exactly; the rebased native files are unchanged, and
the sole TODO conflict preserved both plans' completed-entry removals. Existing commit/PR wording is unchanged from the
previously cold-read text. The report passed an independent cold read after clarifying the earlier landing failures and
their disposition.
