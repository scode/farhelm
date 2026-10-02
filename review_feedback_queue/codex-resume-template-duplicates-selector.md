# Codex resume launches can duplicate the resume selector

Reviewed commit: 1ec60cc06baff3442bcdd73760a9ebdc8b08c49c

## TLDR

Restarting a Codex session that originally started with `codex resume <id>` can fail before the replacement agent
starts, even when Farhelm has a valid conversation to resume.

## Details

Found by pre-pr-review-swarm run `20261001-1343-1ec60cc-5337` as `F1 / COR-CODEX-RESUME`, tagged **definite**. The
finding is surfaced for a specification decision because `SPEC.md` currently says to reuse the original argv and append
`resume <conversation-id>`.

At `crates/farhelm-supervisor/src/agent_kind/mod.rs:1557`, Codex's default restart template copies the original argv and
appends another `resume` subcommand and captured conversation ID. An original launch such as `codex resume old-id`
becomes `codex resume old-id resume captured-id`, which Codex rejects during argument parsing. A valid Resume offer can
therefore produce no replacement process. The captured conversation may also differ from the original selector, so
retaining the old selector would be wrong.

Decide whether supported existing resume forms should replace their selector, or whether Farhelm should refuse automatic
derivation for those forms and require an explicit template. Reconcile that choice with the literal argv-reuse rule in
`SPEC.md`; add a regression for an original resume launch whose captured conversation differs from the original
selector.
