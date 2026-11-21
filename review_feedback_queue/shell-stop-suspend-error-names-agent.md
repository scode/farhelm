# Stop and suspend errors still tell shell users that an agent stopped

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

A partial Stop and suspend failure can say an agent stopped after stopping Bash or Zsh.

## Details

**other**. Confirmed by source and contract inspection.

Confirmed source. Session_list/stop.rs:129–171 and session_list/environment.rs:554–569 name the shell before submission.
Helm sessions/sandbox.rs:574–592 then emits “The agent stopped, but cleanup was incomplete. The environment was not
suspended” and “The agent stopped, but the environment was not suspended” independent of launch type. The desktop's
environment.rs:667–675 only suppresses the expected carries-on timeout, so ordinary non-timeout refusal text remains on
the session, as its :53–60 documents. SPEC.md:1978–1981 requires a visible failure or partial result and :1997–2003
distinguishes the shell as the process Stop ends; §Sandbox sessions :1629–1633 requires the Stop and suspend outcome to
remain intelligible. Estimated bucket other: it misidentifies the program in a partial-failure diagnostic, while the
message still reports that the environment did not suspend. Possible_cover: none. The backend's phrasing predates shell
support. The feature made it wrong for a supported launch, and 05ae73f23eb2cbb7ab284ca42473d66a771207a9 only corrects
Delete. No later fix through pinned tip 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-presentation/f6`.
