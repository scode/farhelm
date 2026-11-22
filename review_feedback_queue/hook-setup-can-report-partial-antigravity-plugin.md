# Antigravity hook setup can report success for half a plugin

Reviewed commit: 47140bd63486498b7af024b05fa8ade7f8ea849e

## TLDR

Removing the Antigravity plugin directory during an accepted launch can leave it partly installed while Farhelm reports
setup succeeded.

## Details

**other.** Investigated uncertainty.

47140bd63486498b7af024b05fa8ade7f8ea849e crates/farhelm-supervisor/src/agent_kind/antigravity.rs:17-30,34-50;
crates/farhelm-supervisor/src/agent_kind/persistent_hooks.rs:39-56,68-85;
crates/farhelm-supervisor/src/files.rs:203-208,400-432; 1a53d9d35889d7dde96e31320cc1681dd013737d
SPEC.md:3979-3986,5344-5372; SPEC_impl.md:4503-4516

Setup publishes the manifest and callback separately and creates the parent directory for each file. The documented
removal procedure can overlap a launch that has already acquired setup consent. If removal occurs between those writes,
the second write can recreate the directory with only its callback, after which the supervisor records Installed. This
is a false status even without a filesystem error on either individual write. A later launch may repair it. The distinct
possibility of a redirected replacement of unrelated user files remains unverified. Validate the prepared pair together
before committing setup success. See `crates/farhelm-supervisor/src/agent_kind/antigravity.rs:17–50`,
`persistent_hooks.rs:39–56,68–86`, and `website/src/content/docs/docs/agents/antigravity.md:35–37` at the pinned tip.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-persistence/hp-path-boundary`.
