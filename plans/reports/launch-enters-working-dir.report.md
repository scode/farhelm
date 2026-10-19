## What this was about

An existing session folder could pass Farhelm's earlier checks while refusing actual entry. tmux could then silently
start an agent or terminal shell in HOME, while Farhelm still showed the chosen folder. The maintainer required each
launch to enter its session folder or fail clearly naming it, with ordinary symlink resolution allowed.

Agent launches now enforce entry immediately before starting the requested program, after shell startup and
managed-checkout preparation, and supply a matching PWD. Create, retry, Restart and Replace share that launch builder.
Terminal tabs enforce entry before their login shell starts. Entry failures use the existing agent error record or
failed-tab-open refusal.

## Things you should know

An agent returns to the session folder even if its shell startup files change directories. A terminal tab keeps its
shell's ordinary startup-file cd behavior. Tab startup remains inside its existing process containment and still removes
inherited agent and private-tmux markers.

The working-directory field is optional for compatibility with older launch specifications. An already-written old
specification without it retains its inherited directory; newly composed launches always carry the folder. There is no
protocol or database migration. The specification and changelog record the new rule, and the feedback item is removed
with its execution recorded.

## Open questions and possible follow-ups

macOS privacy protection for Documents and Desktop as a routine cause remains unverified. Runtime validation ran on
Linux with real permission denial and pinned tmux; no Mac privacy or native-shell run is claimed. The tests canonicalize
fixture paths to account for systems where the temporary directory is a symlink.

## The PRs

- [#2363 — enforce the session folder at launch](https://github.com/scode/farhelm/pull/2363/changes); one draft PR for
  this outcome.

## Checks run, reused and skipped

All runtime commands used the recorder in development mode, nextest 0.9.148, four global slots, zero retries and
required pinned tmux 3.7c. They cover working change `kzzyxtwq` on main `81bcdc03` with source fingerprint
`9d7403800085b3803bec6d81aff2e1b5ae74229dd56bbdeb3099ea91d6362c1c`.

- `cargo nextest run -p farhelm --test e2e -E 'test(=terminal_tabs::a_tab_refuses_a_directory_it_cannot_enter)'`: one
  passed, run `148a5e25-c31c-49c3-abf4-4a8d29fed6de`. The fixture proves that directory metadata and canonicalization
  succeed while a real process cannot enter it, then checks the visible refusal and cleanup.
- `cargo nextest run -p farhelm --test e2e -E '<eight named Create/startup/tab contracts>'`: eight passed, run
  `78ed8c7f-e20b-4b66-bf2b-36df36f96517`. Covers Create denial without executing the requested program, agent-versus-tab
  startup order, normal tab cwd, dead-before-open errors, startup-file refresh, opposite marker removal and
  ordinary/scoped daemon cleanup.
- `cargo nextest run -p farhelm-supervisor --lib -E '<twelve named launch contracts>'`: twelve passed, run
  `bcef7b92-652b-4e97-8679-483c9ce261dd`. Covers cwd/PWD configuration, the shared builder, older-spec parsing, exact
  helper/scope argv, durable exec failures, inherited environment scrubbing, preparation order, real Git checkout and
  ready-checkout restart.
- Complete manifests and JUnit reports were inspected, with successful child and recorder exits and no runtime SKIPPED
  messages. Setup passes are separate from the 21 selected tests; other cases were selected out.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, targeted `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`, and the isolated interpreter's
  `python -B scripts/check-test-sleeps.py`: all passed. The sleep checker inspected 304 delays with no missing
  rationale.

Earlier attempts found two compilation mistakes in the new attachment test and a path assertion that did not allow
terminal line wrapping. These were corrected; they are not successful evidence or latent-flake claims. Their private
records were lost during an environment interruption, so the focused runtime commands above were run again. No earlier
task's runtime evidence is reused.

The stack was rebased cleanly onto `81bcdc03` after inspecting intervening prompt-validation and separate
specification/bookkeeping changes. Those changes do not alter cwd enforcement. The final execution-link amendment
changes bookkeeping only, so successful source checks remain applicable.

Broader Rust, browser, JavaScript, desktop, installer and provisioning suites were skipped: no UI, installation or
protocol behavior changed, and the focused process tests cover the launch and containment boundary. Separate
Restart/Replace denial runs were not needed because they use the tested shared builder and error path. No live
installation was changed.

## Review gate outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort reviewed correctness, design and idiomatic code
against the full goal, outcome and test-authoring checklist. Its one finding concerned logical versus physical
temporary-directory spellings on macOS. The fixture was corrected and the same reviewer confirmed no findings remained,
including the attachment API corrections.

After the environment interruption, the reviewer service could not resume that conversation. The only later source
change normalized captured newlines for the whitespace-free denied-folder fixture's path assertion, to allow tmux
wrapping; the parent checked it and its exact runtime test passed. No second fresh reviewer or swarm was substituted.
Commit and PR wording passed their separate fresh-context cold read. A documentation pass covered all touched files.
