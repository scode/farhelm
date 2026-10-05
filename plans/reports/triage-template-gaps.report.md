### What this was about

Two review findings about launch templates, both triaged on 2026-10-05 as "fix code": templates were doing something
their spec already forbids.

- **A new template could silently replace another one** (`template-clobber.md`). The Templates panel refuses to save a
  new template, or rename one, under a name another template already has, because the save would replace that other
  template. But it checked the name against its list of templates, and read a list that was still loading or had failed
  to load as "no template has this name". So with a slow or failing list, saving a new template named like an existing
  one replaced the existing one, which the user never opened.
- **An agent's session could start on the wrong machine** (`template-successor.md`). A template names its host by the
  installation it was made for, so that a host entry pointed at a different machine stops matching it. When an agent
  creates a session from such a template (`farhelm agent create` with no `--host`), the helm checked that installation
  while resolving the template, but sent the create on whatever connection the host entry had at the moment it sent it.
  If the entry had meanwhile been pointed at another machine and the new installation adopted, the session started on
  the replacement machine.

What landed:

- The Templates panel saves a new template or a rename only once its list has loaded with no reload in progress. Until
  then the save is refused with a message saying why ("the template list is still loading …", or "… could not be loaded
  …; retry loading it above"), and a list that failed to load now shows a **retry** button. Saving the template you
  opened under its own name works as before.
- The helm now checks, just before sending an agent's create, that the host's connection still reaches the installation
  the template names, and refuses the create otherwise, starting nothing ("the host … that the template names now
  reaches a different Farhelm installation …"). A create that names its host with `--host`, and `farhelm spawn`, are not
  affected.

### Things you should know

- **A reload counts as loading.** Both reviewers (see Review gate) found that the first version of the panel fix still
  let a save through while the list was reloading after a save, a delete or a retry, because the panel keeps showing the
  previous list during a reload and that list can lack the template just saved. The fix treats a reload as "still
  loading", so right after saving one template the panel briefly refuses saving another new one until the reload
  finishes.
- **The wrong-machine window is narrow.** The triage assessment's example ("while the create waits for approval") is not
  quite how it happens: the approval already rechecks connections over the wait itself. The gap was an adoption of the
  host's new installation landing just before the approval starts or just after it ends. The changelog entry describes
  it that way. A connection whose installation is unknown is refused too.
- **One browser test was adjusted.** The Templates spec saved a new template without waiting for the list to load; under
  the new rule that save would be refused on a slow machine, so the test now waits for the list first.
- **Not covered: two windows.** If another window or client creates a template with the same name after this panel
  loaded its list, a save here can still replace it; closing that needs the helm to refuse the write itself, which would
  be an API change and was outside these outcomes.

### Open questions and possible follow-ups

- Should the helm refuse a template write that would replace another template, so the two-windows case is closed too?
  Options: leave it (it needs two windows editing templates at once), or add a "create only" mode to the template write
  that the panel uses for new templates and renames. Recommendation: leave it unless it comes up; the panel case that
  needed only one window is fixed.

### The PRs

- #1652 — fix: stop a template save overwriting another while the list loads (removes `template-clobber.md`).
- #1653 — fix: refuse a template's host once it reaches another installation (removes `template-successor.md`).

Each PR adds its changelog fragment, removes its review-queue item and index line, and records its execution in
`TRIAGE_OUTCOMES.md`.

### Checks run, reused and skipped

Run, all passing, on the PRs' code (the stack was then rebased onto main `d5200cf8`, whose changes touch none of these
files beyond the review queue's index):

- `cargo fmt --all -- --check`; `cargo clippy -p farhelm-ui --all-targets -- -D warnings`;
  `cargo clippy -p farhelm-helm
  --all-targets -- -D warnings`;
  `cargo check -p farhelm-ui --features web --target wasm32-unknown-unknown`.
- Rust tests through the recorder: the Templates panel's unit tests (run `964a448a`, 3 passed) and the helm's agent
  request tests, which include the new retarget test (run `8afff8ab`, 61 passed, with the pinned tmux).
- The Templates browser spec on Chromium and WebKit (run `e708f679`, 8 passed), with the web UI rebuilt for the panel
  change.
- `python -B scripts/check-test-sleeps.py` (0 unannotated), `python3 releasing/check-changelog.py format`, and
  `dprint
  check` on the changed Markdown.

Skipped: the full Rust battery and other browser specs. The changes are one save check in the Templates panel and one
check before an agent create is sent; the focused selections above exercise both, and nothing else changed.

### Review gate

Each PR got the two fresh-context reviewers you asked for, Claude Opus 5.5 and gpt-6-astra, both at high effort. Every
finding was addressed except these, each a recorded decision: the two-windows case above (out of scope); a suggestion to
drop the new record of which installation the template named, by reusing the template's own host field instead, which
was kept because it makes "the host came from a template" explicit where the check is made; and no second review round
for either PR, since both fixes followed what both reviewers named and are covered by the updated tests.
