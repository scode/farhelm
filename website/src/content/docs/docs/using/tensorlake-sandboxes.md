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

You can also [give each session its own sandbox](#give-each-session-its-own-sandbox), created when you launch and
deleted together with the last session in it.

Sandboxes bill while running, including while their agents are idle. Snapshot storage also costs money, including while
a sandbox is suspended; see [Tensorlake pricing](https://www.tensorlake.ai/pricing) for current terms. Building a
recipe's snapshot, and preparing it for new hosts, each use a temporary sandbox that bills too. Closing Farhelm does not
delete your sandboxes.

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

You can also save, test or clear the same API key in Settings → **Credentials**, where it is labelled **Tensorlake API
key**. Both places show whether a key is saved; neither shows its value again.

## Save named credentials

Settings → **Credentials** has lists for GitHub tokens, agent subscription logins and API endpoint tokens. Each has a
name you choose, fixed once saved. **Edit** replaces its value while keeping that name. A blank replacement keeps the
saved value; closing Settings discards text you have not saved.

**Not encrypted on disk:** Farhelm stores these values in plain text on the machine running the helm. Anyone who can
read its state directory can read them. Farhelm never shows a saved value again.

For Claude Code, save the token printed by `claude setup-token`. For Codex, run `codex login` on a machine with a
browser, sign in with ChatGPT, then paste or choose `~/.codex/auth.json`. Recipes select these logins, named API
endpoint tokens and GitHub tokens as described below.

Use one ChatGPT login per host. Copies share a rotating refresh token: when one copy renews, other copies cannot renew
and need a fresh sign-in. [Codex's authentication guidance](https://learn.chatgpt.com/docs/auth/ci-cd-auth) explains
this ownership. To share inference access, prefer a proxy or load balancer through an API endpoint. You are responsible
for keeping that use within your provider's terms of service.

## Make a recipe

Choose **New recipe** in Settings → **Recipes**. Recipes are shared by Tensorlake sandboxes and
[Podman container sessions](/docs/agents/containerized-sessions/). A recipe keeps their base, packages, credentials and
tunnels. A [launch template](/docs/using/launch-templates/) keeps choices for starting a session.

Settings → **Tensorlake** keeps the account, SSH key, creation confirmation and sandbox leftovers. **Recipes** holds the
shared recipe catalog.

Give the recipe a name. **CPUs**, **Memory, GB** and **Disk, GB** set the size of Tensorlake sandboxes. Podman
containers use the host account's Podman configuration for resource limits. Tensorlake decides what your plan allows
when the sandbox is created. Names use lowercase letters, digits and hyphens, start with a letter, end with a letter or
digit, and have at most 32 characters. A saved recipe cannot be renamed; create another recipe for a different name.

Under **Software**, choose the base Farhelm builds on:

- **Managed Ubuntu** uses Tensorlake's Ubuntu image with systemd for sandboxes and a pinned Ubuntu image for Podman.
- **Your image** takes an image name from your Tensorlake project and works only for Tensorlake.
- **Your snapshot** takes a snapshot ID from that project and works only for Tensorlake.
- **Container image** takes an image reference and **Container login user** for Podman only.

A recipe's supported destinations and build status appear in the list. An unsupported recipe remains visible in the
launcher with the reason it cannot be used there.

A custom base must run systemd as its init system, allow sudo without a password, be Ubuntu or Debian with apt, and
include bash, curl and git. The build checks these first and names the first missing requirement. A base containing an
existing Farhelm supervisor installation is refused because it would copy another host's identity: when Farhelm prepares
the recipe's snapshot for new hosts, with the reason on the recipe's row, or at host setup. You cannot select a snapshot
Farhelm manages for a recipe: a later rebuild would delete it.

For any base, enter optional **Ubuntu packages**, separated by spaces or newlines. Farhelm builds a Tensorlake snapshot
or a Podman image with those packages, Claude Code and Codex from their official installers. Each environment gets
Farhelm's [supervisor, the program that keeps its sessions running](/docs/how-it-works/the-pieces/).

**Cancel** and **Save** stay at the bottom of the editor while its fields scroll. Saving a recipe does not build a
snapshot or image, or create a host.

Changing the base keeps the previous built snapshot until a successful rebuild replaces it. Hosts already created from
it keep their software.

Recipes saved under the old **Use an existing image or snapshot** choice become custom bases with no built snapshot.
Build one before adding another host, or let adding the host build it. Existing hosts keep working, and **Re-run** setup
on those hosts does not start a snapshot build. Old package recipes keep their built snapshots.

### Agent API endpoints

Under **Agents**, choose **API endpoint** for Claude Code, Codex, or both, then set a base URL and choose a named
endpoint token. Create the token in Settings → **Credentials** first; **None** in the token picker uses the endpoint
without a token. A base URL can point at a [tunnel](#tunnels-to-your-side), such as `http://127.0.0.1:8080`. Farhelm
writes these settings when it sets up each host, for every base, and before it connects. Recipe changes reach existing
hosts at their next connection or setup. Updating Farhelm on a host does not independently reapply them.

Replacing a token in Credentials keeps its name, so you do not need to edit the recipes that use it. Changing a base URL
keeps the selected token name. A credential cannot be deleted while a recipe uses it; choose **None** or another name in
those recipes first. Replacing a token sends its new value to hosts already using it; disconnected hosts receive it when
they next connect. Existing agents may also pick up the replacement.

If a host cannot receive a replacement, the save still succeeds and says **Saved; not delivered to** that host, with a
reason. The host stays usable, with an amber mark and the reason in its details. Farhelm tries again at its next
connection or when you replace the credential again.

:::caution[Endpoint tokens after upgrading]

Tokens previously saved inside recipes are removed. Re-enter them in Settings → **Credentials**, then select their names
in the recipes. Reload Farhelm before editing a recipe form that was open during the upgrade.

:::

For Claude Code, choose **Subscription** and select the setup-token you saved in Credentials. Farhelm writes it into
Claude's usual settings and removes its API endpoint settings. Claude may show its own first-run setup once on a new
host, which you go through there. Choosing **None** removes the token and endpoint settings Farhelm manages, keeping
your other Claude settings.

For a ChatGPT login, choose Codex **Subscription** and select the login you saved in Credentials. Farhelm writes it to
the host's usual `~/.codex/auth.json` and leaves Codex's refreshed file alone until you replace that saved login. Recipe
selection changes take effect at the next connection or setup; replacing the saved login sends it to hosts already using
it, with the same delivery warnings as endpoint tokens.

Codex **None** and **Subscription** clear Farhelm's API-provider selection and bearer token, keeping your other
settings. Switching away from Subscription leaves `auth.json` on the host. Sign out in Codex on that host to remove it;
choosing None in the recipe does not sign you out.

### Tunnels to your side

Each row under **Tunnels to your side** maps a port inside a sandbox or container to a host and port reachable from the
helm's machine. For example, port `8080` inside → host `127.0.0.1`, port `8080` lets programs there use `127.0.0.1:8080`
to reach a service on the helm's machine. **Anything in that environment, agents included, can use these tunnels.**

Tunnels run while Farhelm is connected to the host and stop when it disconnects, suspends or deletes the host. Changes
to recipe tunnels reach existing hosts at their next connection. Host details show each mapping and its status; a failed
tunnel makes the host's dot amber and shows the reason while Farhelm retries.

Tunnels use the host's existing connection, including your SSH configuration and **ProxyJump**. They close when the helm
quits or crashes, and within about two minutes when its machine goes silent. These are separate from
[opening a remote localhost link](/docs/using/work-in-a-session/#open-a-remote-localhost-link) in your browser, which
carries traffic in the other direction.

## Build or rebuild a snapshot

In **Settings → Recipes**, each recipe's row shows its Tensorlake snapshot, with **Build snapshot** while it has none
and **Rebuild snapshot** once it has one, and a status line with **Rebuild** for each container host. Build the first
snapshot there, or let adding the first ordinary sandbox host build it. A session-owned sandbox needs that snapshot
before launch. Status shows build progress, age and failure reasons; a failed build offers **Retry**.

New hosts and sandbox sessions start from a second, prepared snapshot with Farhelm already installed. The first one to
need it prepares it, which takes about a minute; the recipe's row says whether it is ready.

**Rebuild snapshot** also picks up fresh packages and agents, or changes to the recipe's base or packages. A Tensorlake
rebuild replaces the old snapshot and deletes it once the new one is ready. Hosts already created from the old snapshot
keep running; their software and size do not change. A failed rebuild keeps the previous snapshot available. Size
changes apply to new hosts, and base or package changes apply to new hosts after a rebuild. If you reduce the disk below
the snapshot's size, rebuild before adding a host.

The recipe's row warns if Farhelm could not delete an old build sandbox or snapshot; the next build or deleting the
recipe retries that cleanup. Deleting a recipe cancels any running build and deletes the snapshots and container images
Farhelm built for it; your own base image or snapshot is kept. It is refused while a host uses the recipe, and container
images still in use must be released first.

## Give each session its own sandbox

In the session launcher's **Runs on** switch, choose **New sandbox**, then a **Recipe**. The recipe needs a built
Tensorlake snapshot first. You can also save this destination in a [launch template](/docs/using/launch-templates/).
Farhelm asks before creating the billed sandbox unless **Create Tensorlake sandboxes without asking** is on.

Choose a [managed checkout](/docs/using/start-a-session/#start-from-a-managed-checkout) or start in the sandbox's home
directory. **Clone** creates another sandbox from the same recipe; it does not copy the original's files. **Restart**
and **Replace** keep the existing sandbox and its files. Recent setups retain the recipe and home-or-repository choice
after Delete and create a fresh sandbox when applied. Opening **+** (New session) while one is selected starts from its
recipe too, ready to create a fresh sandbox.

**Suspend** freezes the entire sandbox with its processes intact; **Resume** continues them. **Stop** ends the agent and
its terminal tabs while the sandbox keeps running and billing. **Stop and suspend** does both. A session sandbox uses a
30-minute idle timeout after its connection from Farhelm ends. (On Tensorlake's Usage Credits plan, an ordinary host
sandbox instead suspends after 24 hours without a connection from Farhelm.)

These sandboxes appear in **Hosts** and the sidebar's update and problem notices, without permanent navigation rail
icons. A sandbox left by interrupted setup or failed cleanup stays listed there. Agents in sandbox sessions can ask
Farhelm to act only within their own sandbox
([Agents in sandboxes and containers](/docs/using/approve-agent-requests/#agents-in-sandboxes-and-containers)). An agent
there that deletes another session removes only that session, never the sandbox.

### Delete a session in its own sandbox

**Delete** always asks first, because the sandbox may go with the session. Its confirmation says both outcomes: the
session and its checkout are deleted, and if it is the last session in the sandbox, the sandbox, every process and file
in it and its snapshots are deleted too. Unless another session still uses it, the checkout is deleted for good; it does
not go to the [checkout trash](/docs/using/session-list/#managed-checkout-trash). If other sessions are still there, for
example ones an agent started or one a failed Replace left behind, the sandbox stays and Farhelm tells you how many
("The sandbox stays: 2 other sessions are in it"). A failed Replace itself preserves the sandbox; its error identifies
the machine to recover work from.

Delete checks with the Farhelm running inside the sandbox, the only place that knows every session in it. When that
check cannot happen, because the sandbox is suspended or unreachable or its Farhelm needs an update, or it gets no
answer, Delete offers **Delete session and sandbox** instead. Its confirmation warns that every session, process, file,
checkout, unpushed change and snapshot in the sandbox will be removed, including sessions Farhelm does not list. To
delete just the one session, [resume](#suspend-and-resume) or update the sandbox first.

While Farhelm deletes a sandbox, **Hosts** and the sidebar show it as **Deleting…**, and its sessions' actions are
unavailable. If that fails after its last session was deleted, it shows **Deleting failed** with the reason, and
**Retry** deletes it again.

Deleting several [marked sessions](/docs/using/stop-restart-resume/#stop-or-delete) works the same way, one session at a
time. The confirmation says which marked sessions are in a sandbox Farhelm cannot check with, and those take their whole
sandbox with them. **Delete sandbox** in **Hosts** always deletes the whole sandbox
([Delete a sandbox](#delete-a-sandbox)).

## Add a sandbox host

Open **Add host** and choose **Tensorlake sandbox**. Enter a **Name**, which is how Farhelm shows the host, and choose a
**Recipe**. **New recipe…** opens the same editor inside this dialog, with **Cancel** and **Save** in its footer. Choose
**Save** to save the recipe and return to the host form. The selected recipe's summary shows its size, base, packages
and tunnels. Farhelm generates the sandbox's name; **Name** is the name you see on the host and its sessions.

Choose **Continue**, review the size and numbered setup steps, then **Create sandbox**. **Back** lets you change the
name or recipe before creating anything. The confirmation groups billing, the pricing link and the 24-hour idle limit
together. **Create Tensorlake sandboxes without asking** in Settings → **Tensorlake** skips this question; it starts
off. **Set up new hosts without asking** under Hosts does not skip it.

The host appears in **Hosts** as soon as setup starts, with progress for the snapshot build and **Prepare snapshot**
when needed, sandbox creation, and connecting to Farhelm, which starts when the sandbox boots. If setup fails, its
details show why; **Re-run** continues setup on the same sandbox. Use [Delete sandbox](#delete-a-sandbox) if you do not
want to keep it.

New Tensorlake hosts have **Start YOLO sessions without asking** on. This skips Farhelm's
[YOLO confirmation](/docs/using/start-a-session/#confirm-a-yolo-launch); you still choose the session's permission mode
in the launcher. Turn the host setting off if you want that question. The other host permissions start off.

### Managed checkouts

[Managed checkouts](/docs/using/start-a-session/#start-from-a-managed-checkout) of private GitHub repositories can use a
named token from Settings → **Credentials**. Select it under **GitHub** in the host's recipe. New agents, terminal tabs
and restarted agents receive it as `GH_TOKEN`, including the Git clone that prepares a fresh checkout.

Farhelm-built snapshots include GitHub's `gh` command and Farhelm sets up its Git helper on the host. Hosts created from
old reused images, or older snapshots without `gh`, need it installed for private GitHub checkouts over HTTPS. If it is
missing, the host stays usable and its details say private clones need gh. Older hosts also need **Update** before their
Farhelm can deliver the token.

Replacing the token in Credentials sends it to connected hosts; other hosts receive it when they next connect. Recipe
selection changes wait for the next connection or setup. Existing programs keep their environment. Selecting **None**
removes Farhelm's token file, leaving any Git authentication you configured yourself. Public repositories need no token.

## Suspend and resume

Choose **Suspend** in the host's `⋯` menu in **Hosts**. Farhelm asks first if an agent is working or it cannot check
activity reliably. Suspending keeps processes, files and sessions intact. The host shows **Suspended**, and **Resume**
on its row or menu continues it. An open session also offers **Resume host** in its suspended banner.

While Tensorlake is suspending the host, Farhelm shows **Suspending…**; **Suspended** means Tensorlake has confirmed it.
Resuming shows **Resuming…**, then **Connecting…** until Farhelm can reach the host again.

On Tensorlake's Usage Credits plan, Tensorlake suspends a sandbox after 24 hours without a connection from Farhelm, for
example while the helm's machine is asleep. An idle agent while Farhelm stays connected does not start that countdown.
Farhelm does not suspend idle hosts itself or resume them without a user action. After Farhelm disconnects, configured
tunnels leave no separate connection keeping the sandbox awake.

Launching on a suspended host shows progress while it resumes and reconnects, then starts the session automatically. For
a fresh GitHub checkout, the directory is picked when the host is ready; a supplied session name determines its
directory name. If that name is taken, the launch is refused so you can choose another.

## Delete a sandbox

**Delete sandbox** in the host's menu permanently deletes the sandbox, its files, sessions and snapshots taken from it.
It always asks for confirmation, even with **Remove hosts without asking** on. There is no **Uninstall** or forget-only
option for these hosts. Keep any work you need elsewhere before deleting. The recipe and its prepared snapshot remain
available for another host.

If deletion fails, the host stays listed with the reason; choose **Delete sandbox** again to finish. A saved API key is
needed to delete it. Agents cannot delete Tensorlake hosts.

### Sandboxes not on this helm

Settings → **Tensorlake** lists **Sandboxes not on this helm** alongside the account settings. The recipe catalog is
under **Recipes**. These Farhelm-created sandboxes have no host, snapshot build or preparation on this helm. They can
remain after interrupted setup, lost helm state, or creation by another helm; the latter are marked **from another
helm**. Sandboxes you created outside Farhelm do not appear.

Nothing is deleted automatically. Check the name and state before choosing **Delete**: confirmation deletes that
sandbox's files, sessions and snapshots too. Failed cleanup stays visible with **Retry cleanup**. **Refresh** beside the
heading checks the list again, shows **Checking…** while it runs, and says **None found** with the time checked when the
list is empty. The section stays visible with a saved API key, so an empty result still confirms the check.

Next: [Start a session](/docs/using/start-a-session/), or [Manage hosts](/docs/using/manage-hosts/) for host settings
and updates.
