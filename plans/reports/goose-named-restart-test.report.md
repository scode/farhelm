### What this was about

After restarting a Goose session into a conversation chosen by the user, plain Restart must stop offering the previously
captured conversation. If that attempt definitively fails before starting, the previous conversation must remain
available. Existing tests checked the database, leaving the supervisor's live capture and published session list
unchecked. This plan adds that missing regression coverage and removes the covered TODO. No product behavior changed.

### Things you should know

The test fails a named restart after its durable generation advances, checks that the old conversation is restored in
memory and storage and that the session list offers Resume, then succeeds and checks that both captures are empty and
the list offers Not captured. The advanced generation distinguishes rollback from an admission refusal.

The fixture uses a private tmux server, a live launch-shim stand-in and the existing conversation-reporting test seam.
It does not run an installed Goose or prove the vendor's resume behavior. Host systemd is disabled through the existing
fixture seam. No new harness, environment mutation, delay or product fault hook was needed.

### Open questions and possible follow-ups

None identified within this test-only scope.

### PRs

- [#2033](https://github.com/scode/farhelm/pull/2033/changes): test Goose restart conversation state; draft against
  main.

### Checks run, reused and skipped

- The exact new supervisor unit test passed through the recorder with pinned nextest and tmux, four available slots, one
  selected test and zero retries. Development run `5aad5d4a-be3e-4d30-ad6b-e51de895e4cf`; three fresh-process repetition
  runs `51f3fcfc-e39b-49af-bbcc-ec8dc6f0f717`, `b3bcd853-8a83-4941-8e5f-0524e4c2f43d` and
  `344bce1f-4c74-4b06-8e9f-b0528e033767`. Each passed one test with 1117 filtered out; no runtime substrate skip.
- `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`, `cargo fmt --all -- --check`,
  `dprint check TODO.md` and isolated `python -B scripts/check-test-sleeps.py` passed. The delay checker inspected 299
  delays with zero missing reasons. A separate documentation pass covered both touched files.
- Reused those results after rebasing from `e24b410d` onto `4f4de460`: the complete incoming diff contained only queue
  claims, state changes and reports. It changed no supervisor behavior, fixture or specification contract.
- Skipped the full workspace Clippy and runtime suites, browser, desktop, installer, website and release gates. Only one
  supervisor test and its TODO entry changed; the affected crate's complete Clippy targets and four focused runs cover
  the material risks. No earlier plan's test results were reused, and no runtime check failed. A repetition helper
  initially refused a missing required timeout before execution; three direct recorded runs replaced it.

### Review gate outcome

The required fresh-context gpt-6.1-sol reviewer at high effort reviewed correctness, design, idiomatic Rust and test
quality with the full plan and verbatim test-authoring contract. No findings. Static review and runtime evidence are
separate above. Commit and PR wording passed a separate fresh-context cold read. The PR touches no `plans/` files and
remains a draft for the landing monitor.

Implementation stayed in the main session under the no-workhorse requirement. Required source review used codex exec;
wording readers used gpt-6.1-sol at medium effort. Review decisions and external per-turn usage are retained in the
private galaxy-brain session record; exact reported model attribution, native-reader usage and main-session usage are
unavailable. The delivery report also receives the required native cold read.

### Landing

Landed as #2033, after settings-lockout-scope (#2032) in the same round; the two share nothing but TODO.md, where each
removed its own entry and both removals were kept in the rebase. Main had also gained a change to the release brick-test
tooling, which does not touch the supervisor.

#### Review

An independent fresh-context review found nothing blocking. The test checks the live, in-memory capture and the restart
offer the session list publishes, not only the stored rows: Resume after the definitive failure, Not captured after
success. It would fail if the rollback were removed or if the published entry kept the old conversation after success.
The injected failure is a genuinely definitive pre-start failure after the generation advanced, and the test confirms
that. One nit left as is: its docstring calls the restart path "public", though it is crate-internal.

#### Checks

- Run now, on the rebased tree: `cargo fmt --all -- --check`, `dprint check TODO.md`, the sleep checker (299 delays,
  none missing a reason), `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`, and the new test through
  the recorder with pinned tmux, run `f47fe52d-a2b8-4706-b4a7-804adfa4e456`: 1 passed. The 1117 skips are other tests
  outside the selection.
- Reused: the executor's four recorded runs of the same test.
- Not run: anything wider; only this test and TODO.md changed.
