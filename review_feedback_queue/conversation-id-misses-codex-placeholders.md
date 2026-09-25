# The conversation-id check misses Codex directory placeholders

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

None today beyond a failed resume for a malformed id.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F26 / SEC-CODEX-PLACEHOLDER-ID`, tagged **possible**. Anchors and title: `agent_kind/mod.rs:2306`,
`agent_kind/mod.rs:2740-2766` — The conversation-id check misses the Codex directory placeholders

When a session is restarted with "resume", Farhelm builds the agent command from a resume template, such as
`claude … --resume {conversation}` or `codex … resume {conversation}`. The captured conversation id replaces
`{conversation}` (`filled_resume_argv`). That id comes from vendor record files or agent reports, which any local
process could write. Later, `spawn_agent` (`service/core.rs:12214`) runs a second substitution pass, `fill_cwd`, over
the finished command. `fill_cwd` replaces whole elements equal to `{cwd}` with the working directory. It also replaces
the two Codex placeholders `{codex:trusted-cwd}` and `{codex:untrusted-cwd}` with a Codex project-trust setting,
`projects={"<cwd>"={trust_level="trusted"}}` (or `"untrusted"`).

`is_plausible_conversation_id` (`agent_kind/mod.rs:2306`) is the shape check every captured id must pass. Its docstring
says it refuses ids spelled `{cwd}` or `{conversation}` so that the second pass cannot reinterpret what the first pass
wrote, since otherwise "a record file … could steer what the resume argv carries". The check does not refuse the two
Codex placeholders. Both are printable ASCII with no quotes and no leading dash, so both pass. A conversation id spelled
exactly `{codex:trusted-cwd}` would therefore reach the resume command as a whole element and be rewritten into a trust
override. I traced the id paths for every agent kind (the Codex and Grok locator ids and the generic id path all go
through this check), and each would let such an id through.

The reviewer's claim that there is no impact today holds. The built-in templates put `{conversation}` right after
`--resume` or `resume`, so the rewritten string lands where the agent expects a session id, and the only effect is a
failed resume. This is a hardening fix: the refusal list and the placeholder list have drifted apart, and a future
template (or a user-written custom template) that places `{conversation}` after `-c` would let file contents choose a
Codex trust setting. The suggested change is to refuse every placeholder that `fill_cwd`/`has_cwd_placeholder`
recognizes, ideally from one shared list, or simply to refuse `{` and `}` in conversation ids, which no vendor id
contains. Nothing breaks today, so this is optional.
