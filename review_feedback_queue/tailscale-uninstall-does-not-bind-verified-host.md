# Tailscale uninstall can lose its verified host identity

Reviewed commit: 74fa1951bac92f5a78757100534498c458d86806

## TLDR

A confirmed uninstall can affect a different Tailscale host if the saved name changes after Farhelm verifies the
original host.

## Details

The uninstall contract in `SPEC_impl.md:8092` binds the operation to the host Farhelm verified.
`crates/farhelm-helm/src/provisioning/service.rs:3024` revalidates the plan, then `:2685` hands that plan to execution.
The SSH target in `crates/farhelm-helm/src/access.rs:115` carries the saved destination and access mode. Execution
prepares the connection again at `:265`, so the previously verified host identity is absent at the mutation boundary.
Carry the authenticated identity through execution and refuse if the command would act on a different host.

Bucket: highest. Source inspection establishes the uninstall path and its effect on the other host's running work.

Evidence retained under run `26a81958-b7bd-4dbd-a275-d09060967ae5`, finding `tailscale-setup/f1`.
