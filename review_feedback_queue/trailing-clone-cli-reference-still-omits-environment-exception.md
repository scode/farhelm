# Clone CLI reference still omits the environment exception

Reviewed commit: 44137d0659d28b9cadf90a8ffe96a65930035aad

## TLDR

The CLI reference tells agents to supply a host for a Clone that the environment contract requires to omit it.

## Details

**other.** Source inspection. Main reference `SPEC_impl.md:7243`.

#4529 corrects the blanket host requirement at the supervisor doorway (handlers.rs:7214–7219) and puts destination
validation in the helm. The CLI reference at SPEC_impl.md:7217–7219,7238–7249 still repeats the original blanket claim.
SPEC.md:4749–4761 explicitly excludes --host and --cwd for environment sources. Shipped CLI main.rs:414–426 and 737–751
keeps those fields optional, and the helm resolves the source, branches to environment Clone first, then requires --host
on the ordinary path (agent_requests.rs:3861–3890). Existing main.rs:2955–2962 asserts the hostless envelope. Add the
exception to the CLI syntax or qualify its ordinary-host line. Bucket: other, documentation. Confidence: confirmed by
pinned source and an existing assertion, no runtime observation. Possible coverage: l9 for the related supervisor
contract, distinct test issue.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery_contract/f11`.
