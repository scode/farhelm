# SSH help commands can run extra commands when pasted into fish

Reviewed commit: ec767b4912bc238ad20a165148adc6d0d061dd1a

## TLDR

A destination containing backslashes and quotes can turn the connection command offered by SSH help into additional
commands on your machine when pasted into fish.

## Details

Suggested bucket: `highest`, for unintended local command execution. Confidence: likely; fish parsing rules were
verified from its official documentation, but no execution reproduction was performed. Source: separate adversarial
trailing-review agent, welcome/SSH scope. Proposed drop: none.

`crates/farhelm-helm/src/ssh.rs:366–374` quotes every argument with the POSIX-only helper at
`crates/farhelm-proto/src/text.rs:78–95`. Destination validation at `crates/farhelm-helm/src/store.rs:1566–1567` rejects
empty values, a leading dash and NUL, but permits the backslash/quote combination. The Copy button at
`crates/farhelm-ui/src/ssh_help.rs:102–110` supplies those exact command bytes, while the instructions say to run them
in a terminal without naming a required shell.

For destination `a\'; id #`, POSIX quoting produces `'a\'\''; id #'`. Fish treats backslashes before quotes and
backslashes inside single quotes differently, leaving `id` outside the argument. Its
[language documentation](https://fishshell.com/docs/4.3/language.html) specifies those escapes. Verify using a harmless
command stub before choosing a fix. Refusing values that cannot be represented safely, or supplying an encoding safe for
the invoking shell with an explicit supported-shell contract, would avoid the mismatch.

Possible cover: `SPEC.md:2811–2812` excludes other login shells for agent launches and terminal tabs. It does not
clearly cover the user's separate terminal where this help asks them to paste a command. Destination and state-path text
come from authenticated operator input, local configuration or `--ensure-hosts`; no remote-supervisor route that changes
them was found. This limits the threat model and must remain alongside the claim. The removed SSH plan report already
mentioned the issue as a possible follow-up; that is not a specification acceptance or triage decision. No exact Planned
item, accepted bug, queue item, ledger decision or filter covers it.

Additional provenance: the bookkeeping reviewer surfaced the landing report at
`25ddfeb0a6240b2c7b55837f75bc8357b6fdeb11:plans/reports/ssh-2fa-help.report.md:124–130`; the approval reviewer
independently surfaced its deletion by `8d463e232f87f05becd7ed794fd31e76007ecf32`. Both reports are consolidated here;
approval intentionally removes completed plan reports and did not introduce the underlying command path.
