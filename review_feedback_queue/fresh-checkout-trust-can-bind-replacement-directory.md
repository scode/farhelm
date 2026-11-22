# A fresh-checkout trust grant can follow a replacement directory

Reviewed commit: aedc40c90b32779c35dbe8bfcd1637e85e7aad12

## TLDR

Farhelm can grant saved Claude trust to a replacement directory after a configured checkout step changes the path that
was originally checked.

## Details

**highest.** Source inspection.

aedc40c90b32779c35dbe8bfcd1637e85e7aad12 SPEC.md:1915–1927; crates/farhelm-supervisor/src/launch.rs:1736–1779,
1889–1949, 1951–1986; crates/farhelm-supervisor/src/agent_kind/claude/trust.rs:81–104;
1a53d9d35889d7dde96e31320cc1681dd013737d SPEC.md:1744–1758, 1918–1930; SPEC_impl.md:3534–3549

The supervisor checks the identity of the managed checkout, runs the configured post-clone preparation, then resolves
the pathname again to select the Claude trust record. A successful configured step can replace the directory at that
path before the later lookup. The recorded grant is equivalent to accepting repository hooks and commands, and the
specification limits it to the checkout Farhelm created. Source inspection establishes the scope error in the stored
grant. A replacement by a lower-authority actor and subsequent unintended command execution remain unverified. Bind the
affirmative trust write to the directory identity used for the original authorization. See
`crates/farhelm-supervisor/src/launch.rs:1749–1779,1889–1986`, `agent_kind/claude/trust.rs:81–104`,
`SPEC.md:1730–1758,1918–1930` and `SPEC_impl.md:3534–3545`.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `contract-corrections/CC-01`.
