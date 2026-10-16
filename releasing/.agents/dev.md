# Cutting a dev release

How a dev release differs from an RC. It is the main file of the dev release reading list in `releasing/AGENTS.md`; the
procedure itself is `releasing/.agents/rc.md`, which that reading list also names.

> NOTE: In an unattended run (plan executor, landing monitor, delegate), an instruction here to ask, confirm with, or
> propose to the maintainer follows `AGENTS.md`, "Unattended runs".

A dev release is an RC under another name: `X.Y.Z-dev.N`, tagged `vX.Y.Z-dev.N`, cut by exactly the procedure in
`releasing/.agents/rc.md`, "Cutting an RC release", with `dev` in place of `rc` everywhere — the bump commit is
`chore: release X.Y.Z-dev.N`, N increments per attempt and a tag name is never reused, the workflow runs from the tag
and marks the release a prerelease (any semver prerelease suffix does; get.farhelm.io's `latest` still names the last
stable), and `scripts/install.sh` accepts `FARHELM_VERSION=vX.Y.Z-dev.N` the same way it accepts an `-rc.N`. Settle the
same two choices first, base and version, and ask when the request does not state them (a follow-up attempt after fixing
forward already has both); the RC version default in `releasing/.agents/rc.md`, "Cutting an RC release", does not apply
to dev releases. The `-dev.N` and `-rc.N` counters are independent, so `0.3.0-dev.2` and `0.3.0-rc.1` can both exist.
The name is the whole difference: it tells whoever reads the tag list later that the build was a trial of work in
progress, not a claim that this is what will ship as `X.Y.Z`.
