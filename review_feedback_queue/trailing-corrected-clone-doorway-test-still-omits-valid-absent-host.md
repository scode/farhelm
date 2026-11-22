# The corrected Clone doorway test still omits its valid absent-host case

Reviewed commit: 44137d0659d28b9cadf90a8ffe96a65930035aad

## TLDR

The test whose contract #4529 repairs would still pass if the supervisor reinstated a blanket host requirement for
Clone.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/handlers.rs:7253`.

handlers.rs:7214–7219 now says Clone with an explicit source may omit host at this boundary. The live relay validates
before forwarding (3287; Clone rules at 3757–3786). The test accepts no host for Create (7249–7251), pairs no host with
missing source in its Clone rejection (7253–7263), and uses an explicit host for its positive Clone at 7312–7327.
SPEC.md:4749–4761 requires environment Clone to omit host. CLI main.rs:2955–2962 checks parsing/projection and helm
agent_requests.rs:15537–15563 checks acceptance into an environment launch card via a direct helm handler call. Each
bypasses the supervisor doorway. Add an absent-host positive case to the changed test or name an existing test routing
it through that doorway. Bucket: other, local regression-test gap, no current failure. Confidence: pinned assertions and
call paths read, no runtime observation. Possible coverage: any full relay test that proves this input crosses the
supervisor, not established by the CLI and helm tests above.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery_contract/l9`.
