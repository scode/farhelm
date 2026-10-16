# Instruction checker permits local files that suppress repository rules

Reviewed commit: 96ea626d9a502b26a722c900863c49b9a888ff7e

## TLDR

A developer can add personal Claude instructions, pass the repository's instruction check, and then start Claude without
Farhelm's repository rules being loaded by default.

## Details

Suggested bucket: `other`; maintainer tooling, without an established product consequence. Confidence: confirmed static
omission against the documented loader default; no Claude-loader or checker-fixture reproduction. Source: separate
adversarial trailing reviewer, with the official documentation independently checked during recording. Proposed drop:
none.

`scripts/check-agents-md.py:199–203` rejects `AGENTS.override.md` and filenames exactly equal to `CLAUDE.md`, but
permits `CLAUDE.local.md`. After `275b62afe240e772e02ba47d0aeeb1b39be9001a` removes the fallback symlinks, such a file
can prevent native `AGENTS.md` loading. Root AGENTS.md likewise names only `CLAUDE.md` in this prohibition.

[Claude's instruction-loading documentation](https://code.claude.com/docs/en/memory#when-claude-code-reads-agents-md)
states that a local instruction file in the working directory or an ancestor suppresses the default AGENTS loading.
Changing the user's instruction-loading setting can override that default; this finding does not claim that all Claude
configurations omit the rules. An ordinary untracked local file also matters when it is included in the checker's file
inventory; ignored files and ancestors need separate consideration.

Verify the checker with a valid synthetic root and an additional `CLAUDE.local.md`, then decide which local cases the
repository intends to detect. Do not create that file in a working checkout used by other agents. No exact Planned item,
BUGS entry, queue item, ledger acceptance or filter was found.
