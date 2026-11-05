---
title: Tensorlake sandboxes
description: Create a cloud host from a recipe, run sessions there, and suspend or delete it when you are done.
sidebar:
  order: 6.5
---

:::caution[Experimental]

Tensorlake support is early and experimental. Expect rough edges, and expect it to change between releases. You are
responsible for making sure your sandboxes are not running, and billing you, when you do not want them to.

:::

A Tensorlake sandbox is a Linux machine in your Tensorlake project that Farhelm creates and uses as a host. You choose
its size and software in a saved recipe, then start [sessions](/docs/using/start-a-session/) on it like any other host.
You can suspend it with its processes and sessions intact, then resume where you left off.

Sandboxes bill while running, including while their agents are idle. Snapshot storage also costs money, including while
a sandbox is suspended; see [Tensorlake pricing](https://www.tensorlake.ai/pricing) for current terms. Building a
recipe's snapshot uses a temporary sandbox that bills too. Closing Farhelm does not delete your sandboxes.

## Connect your Tensorlake account

Open Settings from the gear at the foot of the [navigation rail](/docs/using/manage-hosts/), the column of icons down
the left edge of the sidebar, and choose **Tensorlake**.

1. Create an API key in the Tensorlake dashboard for the project you want to use. Paste it under **API key** and press
   **Save**, then **Test**. Test checks the saved key by listing the project's sandboxes; save a replacement before
   testing it. **Clear** removes the saved key.
2. Choose **Create SSH key**. Farhelm keeps its own SSH key for these hosts on the machine running your
   [helm](/docs/how-it-works/the-pieces/), the program that brings your hosts' sessions together.
3. Use **Copy** beside the registration command and run it in a terminal on any machine where Tensorlake's `tl` CLI is
   signed in to your account. It has the form `tl sbx ssh keys add --name farhelm-<tag> '<public key>'`; the copied
   command already contains Farhelm's public key. Do this once for this helm.

Farhelm stores the API key **unencrypted on the helm's machine**, in its state directory. It never shows the saved key
again. Farhelm cannot check SSH key registration with an API key; an unregistered key shows up as an error during the
first sandbox setup or snapshot build.

## Make a recipe

Choose **New recipe** in Settings → **Tensorlake**. A recipe keeps the choices for hosts you create from it; it is
separate from a [launch template](/docs/using/launch-templates/), which keeps choices for starting a session.

Give the recipe a name and choose **CPUs**, **Memory, GB** and **Disk, GB**. Tensorlake decides what your plan allows
when the sandbox is created. Names use lowercase letters, digits and hyphens, start with a letter, end with a letter or
digit, and have at most 32 characters. A saved recipe cannot be renamed; create another recipe for a different name.

Under **Software**, choose one of these:

- **Install packages** builds a snapshot from Tensorlake's Ubuntu image with the Ubuntu packages you enter, separated by
  spaces or newlines. Farhelm also installs Claude Code and Codex with their official installers.
- **Use an existing image or snapshot** takes an image name or snapshot ID from your Tensorlake project. You supply the
  agents yourself. The image must have systemd installed, enabled and running as the init system at startup, for example
  an image built from `tensorlake/ubuntu-systemd`. Farhelm uses passwordless sudo to prepare user services, installs its
  [supervisor](/docs/how-it-works/the-pieces/) to keep sessions running, and applies any agent endpoints in the recipe.
  An image containing an existing Farhelm supervisor installation is refused because it would copy another host's
  identity. You also cannot reuse a snapshot Farhelm manages for a recipe: a later rebuild would delete it.

Saving a recipe does not build a snapshot or create a host.

Saving a change from **Install packages** to **Use an existing image or snapshot** deletes the recipe's prepared
snapshot; existing hosts keep running. Switching back needs another billed snapshot build.

### Agent API endpoints

Under **Agents**, you can set a base URL and optional token for Claude Code, Codex, or both. A base URL can point at a
[tunnel](#tunnels-to-your-side), such as `http://127.0.0.1:8080`. Farhelm writes these settings when it sets up each
host, for both software choices. Changing the recipe does not reconfigure agents on hosts already set up, and updating
Farhelm on a host does not reapply them.

Tokens are stored unencrypted in the helm's state and never shown again. A blank token field keeps the saved token while
the server stays the same; changing the server clears it, as the editor explains. Use the clear switch to remove one
deliberately. Subscription logins and uploading agent credentials are not supported yet; use API authentication for
these hosts.

### Tunnels to your side

Each row under **Tunnels to your side** maps a sandbox port to a host and port reachable from the helm's machine. For
example, sandbox port `8080` → host `127.0.0.1`, port `8080` lets programs in the sandbox use `127.0.0.1:8080` to reach
a service on the helm's machine. **Anything in the sandbox, agents included, can use these tunnels.**

Tunnels run while Farhelm is connected to the host and stop when it disconnects, suspends or deletes the host. Changes
to recipe tunnels reach existing hosts at their next connection. Host details show each mapping and its status; a failed
tunnel makes the host's dot amber and shows the reason while Farhelm retries.

The helm's machine must reach Tensorlake's SSH gateway directly for tunnels. They do not use your SSH configuration or
**ProxyJump**. These are separate from
[opening a remote localhost link](/docs/using/work-in-a-session/#open-a-remote-localhost-link) in your browser, which
carries traffic in the other direction.

## Build or rebuild a snapshot

A recipe using **Install packages** has **Build snapshot** in its Settings row. You can build it there, or let adding
the first host build it. The row shows progress and then the build time and installed agents' versions. A failed build
shows its error and **Retry**.

Use **Rebuild snapshot** for fresh packages and agents, or after changing the recipe's packages. A rebuild replaces the
old snapshot and deletes it once the new one is ready. Hosts already created from the old snapshot keep running; their
software and size do not change. A failed rebuild keeps the previous snapshot available. Size changes apply to new
hosts, and package changes apply to new hosts after a rebuild. If you reduce the disk below the snapshot's size, rebuild
before adding a host.

The recipe's row warns if Farhelm could not delete an old build sandbox or snapshot; the next build or deleting the
recipe retries that cleanup. Deleting a recipe also deletes its managed snapshot, and is refused while a host uses the
recipe.

## Add a sandbox host

Open **Add host** and choose **Tensorlake sandbox**. Enter a **Name**, which is how Farhelm shows the host, and choose a
**Recipe**. **New recipe…** opens the same editor inside this dialog. Choose **Save** in the editor to save the recipe
and return to the host form.

Choose **Continue**, review the size, sandbox name and setup steps, then **Create sandbox**. The confirmation explains
that the sandbox bills while running. **Create Tensorlake sandboxes without asking** in Settings → **Tensorlake** skips
this question; it starts off. **Set up new hosts without asking** under Hosts does not skip it.

The host appears in **Hosts** as soon as setup starts, with progress for the snapshot build if needed, sandbox creation
and Farhelm setup. If setup fails, its details show why; **Re-run** continues setup on the same sandbox. Use
[Delete sandbox](#delete-a-sandbox) if you do not want to keep it.

New Tensorlake hosts have **Start YOLO sessions without asking** on. This skips Farhelm's
[YOLO confirmation](/docs/using/start-a-session/#confirm-a-yolo-launch); you still choose the session's permission mode
in the launcher. Turn the host setting off if you want that question. The other host permissions start off.

### Managed checkouts

[Managed checkouts](/docs/using/start-a-session/#start-from-a-managed-checkout) use Git credentials on the sandbox, just
as on any host. Public repositories need no credentials. For private repositories, set up Git credentials on that host
once; if it has GitHub's `gh` command installed, you can run `gh auth login` in a Farhelm terminal there. Farhelm does
not yet supply `GH_TOKEN` to Tensorlake sandboxes.

## Suspend and resume

Choose **Suspend** in the host's `⋯` menu in **Hosts**. Farhelm asks first if an agent is working or it cannot check
activity reliably. Suspending keeps processes, files and sessions intact. The host shows **Suspended**, and **Resume**
on its row or menu continues it. An open session also offers **Resume host** in its suspended banner.

While Tensorlake is suspending the host, Farhelm shows **Suspending…**; **Suspended** means Tensorlake has confirmed it.
Resuming shows **Resuming…**, then **Connecting…** until Farhelm can reach the host again.

On Tensorlake's Usage Credits plan, Tensorlake suspends a sandbox after 24 hours without a connection from Farhelm, for
example while the helm's machine is asleep. An idle agent while Farhelm stays connected does not start that countdown.
Farhelm does not suspend idle hosts itself or resume them without a user action. A tunnel by itself does not keep a
sandbox from reaching Tensorlake's idle limit.

Launching on a suspended host shows progress while it resumes and reconnects. Starting in an existing directory then
continues automatically. For a fresh GitHub checkout, resume refreshes the checkout preview: review it and click
**Launch** again.

## Delete a sandbox

**Delete sandbox** in the host's menu permanently deletes the sandbox, its files, sessions and snapshots taken from it.
It always asks for confirmation, even with **Remove hosts without asking** on. There is no **Uninstall** or forget-only
option for these hosts. Keep any work you need elsewhere before deleting. The recipe and its prepared snapshot remain
available for another host.

If deletion fails, the host stays listed with the reason; choose **Delete sandbox** again to finish. A saved API key is
needed to delete it. Agents cannot delete Tensorlake hosts.

### Sandboxes not on this helm

Settings → **Tensorlake** lists **Sandboxes not on this helm** when it finds Farhelm-created sandboxes with no host or
snapshot build on this helm. These can remain after interrupted setup, lost helm state, or creation by another helm; the
latter are marked **from another helm**. Sandboxes you created outside Farhelm do not appear.

Nothing is deleted automatically. Check the name and state before choosing **Delete**: confirmation deletes that
sandbox's files, sessions and snapshots too. Failed cleanup stays visible with **Retry cleanup**; **Refresh sandboxes**
checks the list again.

Next: [Start a session](/docs/using/start-a-session/), or [Manage hosts](/docs/using/manage-hosts/) for host settings
and updates.
