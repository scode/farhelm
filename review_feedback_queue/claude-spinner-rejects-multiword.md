# Claude's multi-word activity lines may read as idle

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

While Claude compacts its conversation (which can take tens of seconds mid-turn), Farhelm may show the session as idle,
because the working-line check only accepts one-word activity text.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F54 / COR-CLAUDE-SPINNER-WORDS`, tagged **possible**. Anchor and title:
`crates/farhelm-supervisor/src/agent_kind/screen_reader.rs:356` — the Claude spinner check rejects activity text
containing a space, such as "Compacting conversation…".

The supervisor recognizes Claude's working spinner by shape rather than by a list of glyphs (`is_claude_spinner_line`,
`screen_reader.rs:356-361`): one non-alphanumeric glyph, a space, some text ending in `…`, then `(` and a digit, as in
`✶ Imagining… (2s · thinking)`. It additionally requires the text before `… (` to contain no whitespace, i.e. a single
word, and a unit test near line 606 pins that by asserting `✶ Two words… (2s)` is not a spinner.

Some of Claude's in-turn activity lines are several words. The reviewer's example is automatic context compaction, which
happens inside a turn and can last tens of seconds, and whose line would look like `✻ Compacting conversation… (…`.
Under the one-word rule such a line is not a spinner, so if the reader has found the input box (see F52) it reports Idle
as a confident reading. The consequences are the same as F53: a busy session shows idle, and a spurious work start is
recorded when a one-word spinner comes back. The open premise is the exact shape of Claude's compaction line; none of
the captured fixtures in the repository contain it.

The suggested fix is to allow multi-word text while keeping the other anchors (glyph, space, text, `… (`, digit, and the
search window above the box), update the unit test accordingly, and capture a compaction screen as a fixture.
