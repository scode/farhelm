# The Codex "working" screen check never matches real screens

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

When Farhelm cannot see Codex's spinner in the terminal title, a busy Codex session shows as idle, because the backup
check that reads the "Working … esc to interrupt" line never matches the screens the supported Codex version actually
draws.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F14 / COR-CODEX-WORKING`, tagged **definite**. Anchor and title:
`crates/farhelm-supervisor/src/agent_kind/mod.rs:1765` — the Codex "Working … esc to interrupt" screen check never
matches real Codex 0.159.0 screens.

The supervisor has two ways to tell that Codex is busy. The main one is the tmux pane title, where Codex draws a braille
spinner while it works. The backup is a screen-text check for Codex's `• Working (9s • esc to interrupt)` status line
sitting just above the input box. The backup matters when the title shows no spinner. For example, when the supervisor
fails to read the title, it substitutes an empty title and carries on
(`crates/farhelm-supervisor/src/service/ticker.rs:1502-1512`).

The backup first locates the input box with `codex_composer_bounds`
(`crates/farhelm-supervisor/src/agent_kind/mod.rs:1765-1771`). That function requires the second-to-last line of the
screen to be blank padding (`lines.len() - 2`). On the supported Codex version, 0.159.0, that is never true. The
captured screens under `tests/fixtures/screens/codex/0.159.0/working-*.txt` end with two footer rows: a model/context
line (`GPT-6-Astra high · … · Context 2% used …`) and then `← for agents · ? for shortcuts`. The second-to-last line is
the model/context footer, not padding, so the function gives up and the "working" check returns false on every real
screen. The unit tests pass only because they use made-up screens with a single footer row. With no title spinner, a
busy Codex screen then falls through to the input-box check and reads Idle.

So the backstop meant to keep a busy Codex session from showing as idle does nothing on the version Farhelm supports.
The suggested fix has three parts:

- Locate the input box by its `›` prompt row followed only by blank or indented rows down to the bottom, not by a fixed
  line offset.
- Allow blank rows between the status line and the prompt. The real fixtures have two blank rows there, and today's code
  allows only one.
- Test against the real `working-*.txt` fixtures with an empty title.
