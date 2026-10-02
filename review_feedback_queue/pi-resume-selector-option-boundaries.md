# Pi restart rewriting can lose unrelated option values

Reviewed commit: 1ec60cc06baff3442bcdd73760a9ebdc8b08c49c

## TLDR

Some Pi sessions can be offered a resume command that no longer passes the verified session file through Pi's session
option, so the restarted agent may start a fresh conversation instead.

## Details

Found by pre-pr-review-swarm run `20261001-1343-1ec60cc-5337` as `F2 / COR-PI-OPTIONS`, tagged **definite**.

`crates/farhelm-supervisor/src/agent_kind/mod.rs:1376` removes Pi conversation selectors before appending the verified
`--session` file. Its option-consumption table omits valued options such as `--name`, `-n`, `--api-key`, `-t`, and
`-xt`. The exact boundary failure is the original argv `[pi, --name, --resume]`: here `--resume` is the value of
`--name`, not a selector. The stripper deletes it, then appends `[--session, file]`, leaving
`[pi, --name, --session, file]`; Pi consumes `--session` as the missing name value and never receives a session
selector.

Share the complete Pi option-consumption grammar between launch parsing and selector stripping. Values must remain
opaque even when they look like selector flags. Add regressions for selector-looking values, including the exact
`--name --resume` shape.
