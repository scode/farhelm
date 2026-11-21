# The session-view spec still calls the shell’s main tab Agent

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

The session-view spec still tells Bash and Zsh users to look for an Agent tab instead of Shell.

## Details

**other**. Confirmed by source and contract inspection.

The spec still tells Bash and Zsh users to look for Agent when returning to their main terminal, while current Farhelm
shows Shell. Confirmed documentation-only contradiction after 05ae73f23eb2cbb7ab284ca42473d66a771207a9, retained at
8bcca67df1afe6eb0f184ef73b9952ed7ebb329f. SPEC.md:48–53 now defines a main terminal for an agent, shell or command. The
unchanged Session view passage at SPEC.md:2259–2262 says `It shows the agent terminal if you chose Agent` and
`Agent is available while the list loads`. It does not qualify Agent as the label for agent sessions. The later code at
terminal/set.rs:692–712 expressly draws `Shell` with `Shell: show the session's shell` when the open row says Bash or
Zsh, with a source-level test at terminal/set/tests.rs:353–378. The defect in this record is the normative label and
described default, distinct from shell-presentation/f2 and shell-contract/f8's missing-row fallback and
shell-presentation/f13's selected-tab reset. Estimated bucket other. No destructive or runtime route. Possible_cover:
none.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-contract/f11`.
