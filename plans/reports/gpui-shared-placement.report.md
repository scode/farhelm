### What this was about

The desktop launcher and template editor each drew their own Runs on, Recipe and Container host controls. The maintainer
asked to consolidate them during the GPUI migration. The two screens now draw those controls from the same code, which
removes 123 lines overall.

### Things you should know

When saving a template, you can still choose a container host whose Podman check failed or a sandbox recipe that has no
snapshot yet. The editor defaults to the first listed container host. At launch, Farhelm checks readiness, prefers the
last container host you used while it remains usable, and shows the recipe's build status for the selected container
host. The existing control labels, saved choices and keyboard handling carry over.

### Open questions and possible follow-ups

None.

### PRs

- [#4068](https://github.com/scode/farhelm/pull/4068/changes) shares the placement picker between the launcher and
  template editor.

### Checks

- Passed on the final change: `cargo fmt --all -- --check`,
  `dprint check TODO.md docs/gpui/parity/templates-and-quick-launch.md`, and `git diff --check` before committing. The
  base's foundation check
  (`git show 7b2dc77e:scripts/check-foundation-paths.py | python3 -I - --repo . --base 7b2dc77e --head b7471ab0`) passed
  and reported the one expected launcher export.
- Passed on `e34951bc`: `cargo clippy -p farhelm-gui --all-targets -- -D warnings` and all 793 selected tests from
  `cargo nextest run -p farhelm-gui --lib -E 'test(/^launcher::/) | test(/^templates::/)'` (run
  `da7ac312-5e33-4239-84d2-298e72b812ef`). Those tests include default container hosts, unusable options and retained
  template choices.
- Passed on `e34951bc`: `python3 scripts/check-gui-goldens.py` compared the existing light and dark Linux captures pixel
  for pixel (run `67263931-0ee2-4b6f-85cd-32d95c654b42`). Reused Clippy, nextest and golden results for the final PR
  revision because only Rust comments changed after that snapshot.
- Skipped the full workspace suite because the change is confined to desktop picker wiring. Used the existing golden
  suite for visual checks. The live-app screenshot scenes and golden references were left untouched, and no separate
  screenshot batch ran.

### Review gate

The independent GPT-6.2 Sol high reviewer found no behavior regressions. Its documentation finding led to clarifying how
each screen picks a default container host. The commit wording passed its cold read.
