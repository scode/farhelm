# BUGS

Known bugs we are aware of but have no intention of fixing, documented because they can still bite users. This is
deliberately not TODO.md: entries there describe work that is still wanted, while entries here describe behavior we have
decided to live with — usually because the defect lives below farhelm (tmux, the kernel, a library) or because the only
fix would cost more than the bug does. An entry moves out of this file only if that calculus changes.

## A host install or update can hang until the helm restarts if a local ssh helper keeps its output open

If the helm stops a stalled or failed transfer of Farhelm's files to a host while a local helper started by ssh still
holds ssh's output open, the install or update never finishes. The host stays mid-install or mid-update (its row keeps
showing the run in progress) until the helm restarts: renaming it or changing its destination waits, removing it is
refused as busy, and one of the helm's four install slots stays taken meanwhile. Stopping the transfer kills ssh's whole
process group, so only a helper that left it can do this, such as an ssh `ProxyCommand` that daemonizes.

Here's the TLDR of the mechanics. Each provisioning step runs a local process (`ssh` for a remote host) and reads its
output to the end. After that process exits, and after any step other than the transfer kills it, the helm stops waiting
for the output to end after ten more seconds and fails the step, so a process that inherited an output pipe and lingers
costs a delay or an error. The transfer's reads after a kill have no deadline, and neither does the task feeding the
files into ssh. Remote leftovers cannot cause this: they keep `ssh` itself from exiting, and that wait has a deadline.

How sure we are: this comes from reading the code; nobody has seen it happen. Farhelm's own provisioning scripts start
no background processes, and a check with OpenSSH 9.6p1 showed that ssh's connection sharing (a `ControlPersist` master
that stays alive after the command) does not hold the output open: the read finishes within about half a second. Older
OpenSSH releases were not checked.

Why we are not fixing it: there is no realistic trigger on current OpenSSH, and what is left needs both a stopped
transfer and a helper outside ssh's process group.
