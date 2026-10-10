# Managed checkouts

Type `gh:owner/repo` in the launch composer's search field and select the repository to launch an agent in a new clone
on the selected host. Choose the agent independently: structured harnesses and raw commands both work. Ordinary folder
launches still work without any checkout configuration.

## Configure the destination

Create a directory on each target host where Farhelm should put new checkouts. Then run this on the machine running the
helm, against its existing state:

```sh
farhelm helm checkout-config set-root '~/work'
farhelm helm checkout-config show
```

Quote `~` so the command stores it literally. The target supervisor expands it using its own home directory; the helm
does not expand it or create the root. There is no default root. If the helm uses a custom state directory, pass
`--state-dir /path/to/state` to each configuration command.

The setting above is global. Add `--host <id>` to a command to configure a registered host's override. The root and hook
inherit independently. `clear-root --host <id>` removes that override and restores the global root; `clear-root` without
a host removes the global setting. `show --host <id>` shows the selected host's stored and effective values.

An optional post-clone shell command runs inside the new checkout before the agent. For example, if `jj` is installed on
the target:

```sh
farhelm helm checkout-config set-post-clone 'jj git init --colocate'
```

`clear-post-clone` removes the global hook. With `--host <id>`, it restores inheritance instead. To disable an inherited
hook for one host, set that host's post-clone command to the empty string (`''`). Configuring the hook authorizes that
shell command to run for future managed checkouts; cloning itself is built in and does not require a hook.

## Launch and reuse

Select a host, choose an agent, then select `gh:owner/repo`. Farhelm shows the exact destination before enabling Launch.
Previewing or cancelling creates nothing. With no name, `owner/bar` gets the lowest free `bar-N`; with the name `fix`,
it gets `bar-fix`. A name already starting with `bar-` keeps that prefix once. Renaming the session later does not move
its directory. An explicit name that is already occupied refuses to launch; Farhelm does not reuse it.

Clone progress, Git credential prompts and the configured hook appear in the session terminal. Git uses the target
user's credentials and ordinary configuration, including any HTTPS-to-SSH `insteadOf` rewrite. A clone or hook failure
stops before the agent and leaves the partial checkout available for inspection. Restart after successful preparation
reuses the checkout without repeating clone or hook. Interrupted or uncertain preparation refuses automatic repetition.

Suggestions come from this host installation's recent launches and immediate clones under its configured root, without
querying GitHub. You can still enter a valid pair when discovery is incomplete. Repository input accepts `owner/repo`,
not a URL or `@branch` suffix. Reusing a saved repository setup creates another managed checkout. Clone of a session in
a managed checkout starts another checkout of that repository, even from a subdirectory. Replace reuses the source
session's actual directory; explicitly select a repository to request a managed checkout instead.

Selecting a folder result or editing the ordinary folder field leaves managed-checkout mode while preserving the agent
choice. Unlabelled folder search still works; `folder:` restricts it to folders.

If a launch reply is lost, leave the request unchanged and click Launch again to recover its outcome. Farhelm keeps the
original request even if configuration changes meanwhile. When the server proves that request cannot allocate a
checkout, the composer refreshes its preview; launching that new destination requires another explicit click. A
connection error or ordinary conflict alone does not authorize a second checkout.

## Repository caches

Repeated managed checkouts of the same repository are faster because each host keeps a cache of its Git objects. Before
each checkout, Git fetches new branches, tags and objects into that cache in the session terminal, then makes an
independent clone with the ordinary GitHub origin. Git uses the host's own credentials for both commands; a private
repository without a credential helper can ask for credentials twice. A cache error stops the clone stage and names the
cache path, rather than falling back to downloading the whole repository again.

The cache is `<supervisor-state-dir>/repo-cache/<owner>/<name>.git`, outside the working-copy root. It takes roughly the
disk space of one extra repository copy per host. When the supervisor starts, it removes caches unused for more than 30
days. You can remove a cache directory by hand at any time; existing checkouts do not depend on it, and the next fresh
checkout downloads it again. Avoid removing it while a checkout is being prepared, since that preparation can fail.
Leave the sibling `.lock` files in place so simultaneous checkouts continue to take turns on the same lock.

## What deletion does

A checkout stays in place while any retained session uses it or one of its subdirectories. Stopped, exited, and errored
sessions all count. Deleting the final session moves the entire checkout into `farhelm-archived-working-copies` under
its original root, with a timestamped name. This includes untracked files and Git metadata; it frees no disk space. The
trash button beside New opens these archives grouped by host, newest first, with their repository, archive time, disk
usage and on-disk directory. Copy anything you want to recover out of that directory before cleanup. Size totals are
partial when a directory cannot be measured; an unreachable host cannot be checked or emptied.

Delete all on a host, or across reachable hosts, asks once before removing the selected archived checkouts permanently.
Uncommitted or unpushed work in them is lost. Farhelm verifies its recorded ownership again before deleting: unrelated
archive neighbours and replaced directories stay untouched, and an archive containing a retained session's working
directory is refused. Missing or replaced entries can be forgotten from the trash with a visible explanation. Cleanup
failures are shown per checkout, including a failure after some contents have already been removed. There is no restore
action, and nothing empties the trash automatically.

An ordinary session that borrows a managed checkout can be its final reference, so deleting that borrower can cause the
move. Same-directory replacement keeps a reference throughout. Moving a replacement elsewhere releases the old checkout
only when no other session retains it. Directories that Farhelm never allocated remain unmanaged and are not moved.

A failed archive move never blocks Delete. The session is removed, Farhelm stops managing the checkout, and a visible
notice names the directory left behind and the reason. Farhelm does not overwrite a foreign directory, copy across
filesystems, or recursively remove checkout contents as a fallback.

A rare interruption can occur after Farhelm creates the directory but before it records the directory's identity.
Recovery leaves an error session because the pathname alone cannot prove ownership. Deleting that session removes its
unresolved metadata and logs the preserved path, but leaves the unknown directory untouched for manual inspection.
