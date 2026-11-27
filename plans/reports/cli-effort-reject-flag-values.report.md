### What this was about

When an agent or user launches a session or saves a launch template from the command line (`farhelm agent create`,
`farhelm spawn`, `farhelm agent template create`), they can pick the model with `--model` and the reasoning effort with
`--effort`. If the value was left out, the command line took the next option as the value instead of refusing the
command. `--effort --yolo` started a custom agent with `--yolo` as its effort and without the YOLO setting the caller
meant. Built-in agents were safe, because the helm refuses an effort they do not know and any model that starts with a
hyphen, but custom agents had no such check. `--model` has worked this way since before the last stable release.
`--effort` started working this way after a recent change that let a custom agent's effort start with a hyphen
(`--effort -fast`); that change has shipped only in the 0.46.0 release candidates.

You decided:

- both options refuse a separate value that starts with a hyphen;
- a custom agent's hyphen-leading model or effort still works written with `=` (`--effort=-fast`);
- the custom-agent contract itself stays as it is.

What was built: exactly that. `--effort --yolo`, `--model --yolo`, `--effort -fast` and `--model -fast` are now refused
as usage errors, naming the option that has no value (or the unknown `-f`). `--effort=-fast` and `--model=-fast` still
reach the launch unchanged. The options' help text says so. SPEC_impl.md's command-line section is updated and the TODO
entry is removed.

### Things you should know

- **The release note.** Compared with the last stable release, only `--model` changes: `--effort` never took a separate
  hyphen-leading value in a stable release. So the earlier change's unreleased changelog entry (which advertised
  `--effort -fast`) was rewritten into a fix entry about `--model`, with a note for whoever writes the release notes
  that the `--effort` behavior from the release candidates is reverted. The plan expected no release note; because
  `--model` changes for stable users, the entry was kept as a fix. Anyone who used `--effort -fast` on a 0.46.0 release
  candidate needs `--effort=-fast` instead.
- **One narrow edge remains on the template commands.** When saving a template, a template's name may itself start with
  a hyphen, so before the name is given the command line still takes a separate hyphen-leading value that is not a known
  option (`farhelm agent template create --effort -fast mytemplate` saves `-fast` as the effort). A known option such as
  `--yolo` is refused there too, so the case this plan was about is closed everywhere. Changing how the template name is
  read was outside the plan; SPEC_impl.md records the edge.

### Open questions and possible follow-ups

None.

### The PRs

- #5129 `fix: stop --model and --effort from taking the next flag`: the change, its tests, SPEC_impl.md, the changelog
  entry and the TODO entry's removal.

### Checks

Run now, on the pushed change (main has moved since only by queue bookkeeping):

- `cargo clippy -p farhelm --all-targets -- -D warnings`: clean.
- Through the recorder (run b7b93e65-76d6-4ab7-9f1d-046cfc28f760, 195 passed): the command line's own tests and its
  agent command tests:

  ```
  cargo nextest run -p farhelm --bin farhelm --test agent_cli
  ```

  The new tests check that each of the four forms above is refused with the expected error on `agent create`, `spawn`
  and a template save, and that the `=` forms reach the launch unchanged.
- `cargo fmt`, the test-sleep check (no unannotated delays), `dprint check` on the changed Markdown, and the changelog
  format check.

An earlier run of the same selection before the review's fixes (46b790da-6c99-4e13-abf8-3846e36ce33a, 195 passed) is
superseded by the run above and not counted.

Skipped: the workspace-wide Rust run and the end-to-end suites, since the change is confined to how the command line
reads two options, which its own tests cover.

### Review gate

Reviewed by a fresh-context Opus 5.5 agent at high effort. It found no bugs and confirmed the command-line parser's
behavior in a small standalone test program. Its findings were:

- the changelog entry described `--effort` as changed, when compared with the last stable release only `--model` is;
- the test accepted either error kind for every case, so an unrelated parse failure could have passed it;
- several comments, the help text and a test description said more than the code does;
- the template-name edge above, which the spec now records.

All were fixed, and a verification pass by the same reviewer confirmed them.
