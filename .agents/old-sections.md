# Old root section names

Until 2026-10, the root `AGENTS.md` held every instruction in one 76 KB file, and older text (landed plans, reports,
triage records, code comments) still cites its section names. Codex injects only the first 32 KiB of that file, so it
was split: the root keeps the rules every task needs and a map, and the procedures moved to the files below. When a
citation names a root section that no longer exists, find it here.

| Old root heading                                                                   | Where it lives now                                                                                                           |
| ---------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Farhelm agent instructions                                                         | `AGENTS.md` (opening paragraphs)                                                                                             |
| Release brick tests                                                                | `releasing/mac-vm-test/BRICK-TEST.md`                                                                                        |
| Talking to the user                                                                | `AGENTS.md`, "Talking to the user"                                                                                           |
| Conventional Commits                                                               | `AGENTS.md`, "Conventional Commits"                                                                                          |
| Finishing work                                                                     | `AGENTS.md`, "Tests and validation" (the principles); `.agents/validation.md` (the full text, including the check inventory) |
| Careful rebase                                                                     | `.agents/careful-rebase.md`                                                                                                  |
| Vercel deployments                                                                 | `.agents/vercel.md`                                                                                                          |
| Trying a PR or branch on the maintainer's Mac                                      | `.agents/devbuild.md`                                                                                                        |
| Releases and the changelog                                                         | `AGENTS.md`, "Changelog fragments"; `releasing/AGENTS.md`                                                                    |
| Reproducing failures: narrow tests first                                           | `.agents/test-evidence.md`; `AGENTS.md`, "Tests and validation"                                                              |
| FLAKES.md                                                                          | `AGENTS.md`, "TODO.md and FLAKES.md"; `.agents/test-evidence.md` (entry format)                                              |
| TODO.md, Todo flow                                                                 | `AGENTS.md`, "TODO.md and FLAKES.md"; `.agents/todo.md`                                                                      |
| Deflake runs                                                                       | `deflake/AGENTS.md`                                                                                                          |
| Run all tests and fix                                                              | `.agents/run-all-tests.md`                                                                                                   |
| plans/                                                                             | `plans/AGENTS.md`                                                                                                            |
| review_feedback_queue/, Trailing code review, Triage step, Execute triage outcomes | `review_feedback_queue/AGENTS.md` and the files it lists                                                                     |
| Harness-specific code                                                              | `AGENTS.md`, "Code rules"                                                                                                    |
| Harness marks in the sidebar                                                       | `AGENTS.md`, "Code rules"; `docs/harness-marks.md`                                                                           |
| Brand marks: the icon and the wordmark                                             | `.agents/docs-media.md`                                                                                                      |
| Docs website                                                                       | `website/AGENTS.md`                                                                                                          |
| Desktop/web UI bug triage                                                          | `docs/desktop-web-triage.md`                                                                                                 |
| README hero screenshot, README demo video, Docs refresh, Docs screenshots          | `.agents/docs-media.md`                                                                                                      |
| Agent screen fixtures                                                              | `docs/agent-screen-fixtures.md`                                                                                              |
| Driving real agent TUIs                                                            | `.agents/real-agent-tui.md`                                                                                                  |
| The live install is off-limits                                                     | `AGENTS.md`, "The live install is off-limits"                                                                                |
| Sharing the machine with other agents                                              | `AGENTS.md`, "Sharing the machine with other agents"; `.agents/machine.md`                                                   |
| Claude Code shell commands: no globs, no expansions                                | `AGENTS.md`, "Claude Code shell commands"; `.agents/machine.md`                                                              |
| Agent scratch space                                                                | `AGENTS.md`, "Agent scratch space"; `.agents/machine.md`                                                                     |
| lore/                                                                              | `lore/AGENTS.md`                                                                                                             |
