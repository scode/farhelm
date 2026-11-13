---
title: Approve what agents do
description: How Farhelm asks before agents change sessions, templates or hosts, or open links.
sidebar:
  order: 9
---

An agent running in a Farhelm session can do more than work in its own terminal. With the `farhelm` command, which it
uses when you ask it to (with `$farhelm` in your message) but can also run on its own, it can look at your hosts and
sessions, start new sessions on any host, in an existing directory or a new
[managed checkout](/docs/using/start-a-session/#start-from-a-managed-checkout), copy a session to the same host or
another, rename, stop, restart, or delete sessions, change your [launch templates](/docs/using/launch-templates/), and
[add, edit or remove hosts](/docs/using/approve-agent-requests/#host-changes). It can also ask to open a web link in
your browser ([Open links from a remote session](#open-links-from-a-remote-session)). Farhelm asks you before any of
those changes happens by default, whoever typed the command; a command you type yourself in a session's terminal is
asked about the same way. You can [stop asking for a host](/docs/using/approve-agent-requests/#stop-asking-for-a-host).
Looking never asks. An agent in a [container session](/docs/agents/containerized-sessions/) or a
[sandbox session](/docs/using/tensorlake-sandboxes/#give-each-session-its-own-sandbox) can act only within its own
container or sandbox ([Agents in sandboxes and containers](#agents-in-sandboxes-and-containers)). Agents on other hosts
cannot delete those sessions, and they start new sandboxes and containers only behind a card that always asks
([New sandboxes and containers](#new-sandboxes-and-containers)).

An agent starting another session can supply its first message with `--prompt` or `--prompt-file` (`-` reads stdin). The
approval card shows the full message. It is sent once when the new agent starts; restarting, copying or replacing that
session does not send it again. A launch template can supply a message on every launch using it.

## The card

When an agent asks to change something, a compact card appears near the top of the main area, below an open session's
tabs. It leaves the sidebar and the terminal's prompt clear. It says what the agent wants to do and shows the details
that matter:

- which session asked, and from which host;
- the session it would act on, for a rename, stop, restart, or delete, the command a restart would run, and whether a
  delete would stop the agent or terminal tabs;
- for a copy of a session, the session being copied;
- for a new session, the host, the directory, the agent and its choices, the full command it would run and the command a
  later restart would run, and whether it would run in [YOLO mode](/docs/using/start-a-session/#confirm-a-yolo-launch)
  (the agent bypasses its safety checks). For a new
  [managed checkout](/docs/using/start-a-session/#start-from-a-managed-checkout), the card also names the repository it
  would clone, such as `GitHub: acme/web`, above the exact directory the clone would go into;
- for a Codex, Muse or Pi launch or restart, whether it trusts the directory as a
  [managed checkout](/docs/using/start-a-session/#trust-in-managed-checkouts) under **Agent trust** in Settings, which
  lets the repository's own hooks, settings and commands run without asking;
- for a template change, the whole template as it would be saved, including any command line. If the agent changes its
  **⚡** mark or number key, the card shows the change and which other template loses the mark or key;
- for a link, the site your browser would contact, above the full URL. Check the site: a URL can be written to look like
  a different site than the one it opens.

Most of what a card shows was written by the agent: session titles, directories, commands. Read those as the agent's
words, not Farhelm's; each one sits under its own label.

Cards normally offer three buttons:

- **Allow** lets this one request go ahead.
- **Always allow from** the host lets this request go ahead and turns on that host's **Run farhelm commands without
  asking** setting for a session or template change, so agents there stop asking (see below). A link card turns on only
  that link's permission instead. Other cards from that host that are already waiting stay up until you answer them.
- **Deny** refuses it, and the agent is told you declined.

For a session request that stays on the requesting host (including the source when copying a session), the card also
offers **Always allow on &lt;host&gt; only**, with that host's name. It turns on **Manage sessions on this host without
asking** and allows this request. **Always allow from &lt;host&gt;** beside it grants the broader permission to act
across your hosts and change templates. Other cards already waiting still need an answer.

An agent-supplied [wrapper prefix](/docs/agents/agent-wrappers/) on a host that asks before YOLO is a special case: its
launch card offers **Allow** and **Deny** only, even if you turned on either command permission for that host. The
prefix is the agent's own command text, which Farhelm cannot check; allowing one request does not approve later
prefixes. An agent that creates or edits a template so that it carries a wrapper prefix gets the same **Allow** and
**Deny** card on every host, because launches from that template later treat the prefix as yours.

The oldest waiting request opens first. Other requests appear as headers below it; click one to read and answer that
request instead. The card shows how many requests are waiting, and the request you chose stays open until it is answered
or expires. Then the oldest remaining request opens. A long request keeps its answer buttons in view: its details scroll
inside the card, and long commands and prompts scroll in their own boxes, with nothing cut off.

The rest of Farhelm stays usable while requests wait, and several can wait at once, up to four from one host. While four
are waiting, that host's agents are turned away until you answer one. Cards stay clickable even over an open dialog.
When the waiting requests change or you switch to another request, the answer buttons pause for a moment, so a
double-click cannot answer a request you have not read.

## Agents in sandboxes and containers

An agent in a [container session](/docs/agents/containerized-sessions/) or a
[sandbox session](/docs/using/tensorlake-sandboxes/#give-each-session-its-own-sandbox) can ask Farhelm to act on its own
container or sandbox and nothing else. It can start more sessions there with `farhelm spawn` or `farhelm agent create`,
and rename, stop, restart or delete the sessions there. Its lists of hosts and sessions show only that container or
sandbox. Deleting another session there removes just that session: the container or sandbox stays, since the agent's own
session is still in it.

It can also ask to [open links](#open-links-from-a-remote-session), under the container's or sandbox's own link
permissions, and start a session from a launch template that sets no host and no recipe. Farhelm refuses, with "Agents
in sandbox and container sessions can only act within their own sandbox or container.", anything aimed at another host
or at a new sandbox or container, copying a session, listing or changing launch templates, and host changes. Its
requests ask you the same way as anywhere else, using the container's or sandbox's own
[settings](#stop-asking-for-a-host), which start out asking.

## New sandboxes and containers

An agent on an ordinary host can ask for a session in a new [sandbox](/docs/using/tensorlake-sandboxes/) or
[container](/docs/agents/containerized-sessions/), built from one of your recipes:

- `farhelm agent create --recipe dev-box --agent claude` asks for a new Tensorlake sandbox from the `dev-box` recipe.
  Add `--container-host build-box` for a container on that host instead.
- `farhelm agent create --template <name>` does the same when the template's **Runs on** is a recipe.
- `farhelm agent clone --source-session <id>` of a sandbox or container session asks for a new one from the same recipe,
  with the same agent and repository, like **Clone** in the session menu.

These always show a card, even on a host whose settings let other agent commands through without asking, and the card
offers only Allow and Deny. It names the recipe and where the new sandbox or container will be, and for Tensorlake says
that it bills while it runs. Allow is your confirmation: Farhelm then builds it the same way the session launcher does,
and its progress shows in the sidebar. Deny creates nothing. The new session starts in the sandbox's or container's home
directory, or in a fresh checkout there with `--gh`.

## Host changes

An agent can ask to add an SSH host, edit a host's appearance or remove an SSH host. Each request needs your approval,
every time. Neither **Manage sessions on this host without asking** nor **Run farhelm commands without asking** covers
host changes, and these cards offer no **Always allow** choice.

Agents cannot add [Tensorlake sandbox hosts](/docs/using/tensorlake-sandboxes/) or delete Tensorlake hosts. Add those
from **Add host**, and use **Delete sandbox** in the host's menu yourself.

- `farhelm agent host add build-box --alias build` asks to add an ssh destination, with an optional display alias.
- `farhelm agent host edit build --icon flask --color teal` asks to change a host's appearance. It can also set an alias
  with `--alias` or clear it with `--no-alias`. Edits cover only the alias, icon and color; an agent cannot change a
  host's destination or permissions. The card shows each changed field's old and new value.
- `farhelm agent host remove build` asks Farhelm to [forget that host](/docs/using/manage-hosts/#remove-a-host). An
  agent cannot remove the host its own session runs on, **Local (this machine)**, or a Tensorlake host.

For edit and remove, use the host's name from `farhelm agent hosts`. Both offer **Allow** and **Deny**.

An add request waits as a card in every Farhelm window, and never opens anything over what you are doing. Choose **Open
add dialog** on the card to open the ordinary **Add host** dialog, with the destination and alias filled in and the
asking session and host shown. You can change either field before choosing **Add** to approve, or **Cancel** to decline.
Approval starts the ordinary [add host flow](/docs/get-started/add-a-remote-host/) with the values you chose. If Farhelm
needs to be installed on the host, it still asks you to confirm setup, even with **Set up new hosts without asking**
turned on. The settings that skip setup or removal questions apply to actions you start in the UI; they do not approve
an agent's request.

The agent gets `accepted` when you approve and Farhelm starts adding the host. That does not mean Farhelm is installed
or that the host is ready: adding can still fail, need setup, or be cancelled at the setup question. The agent must run
`farhelm agent hosts` and check the host's state before relying on it.

## When nobody answers

The agent waits up to nine minutes for your answer. If nobody answers by then, the card disappears and the agent is told
so; nothing was changed, and the agent can ask again. If no Farhelm window is open when the agent asks, it is told
straight away to ask you to open Farhelm. On your Mac, quitting Farhelm or closing its window counts as no window.

A card stays up even if the agent gives up waiting for it before the nine minutes are over, and allowing it then still
does what it says. Deny cards you no longer want.

If the session that asked is deleted while its card waits, the request is refused.

An agent can delete another session, including its terminal tabs and stored state. It needs `--stop-if-running` when
anything is still running, and the card says what will stop. It cannot delete its own session; it can still stop its own
agent, which also closes the session's terminal tabs, and leave the session listed.

## Stop asking for a host

In [host settings](/docs/using/manage-hosts/#host-settings), **Manage sessions on this host without asking** lets agents
in that host's sessions create, spawn, copy, rename, stop, restart and delete sessions on the same host without a card.
A copy's source session must be on that host too. This includes creating a session with an initial message. Requests for
other hosts, template changes, [host changes](/docs/using/approve-agent-requests/#host-changes) and links stay outside
this permission. Sending messages to agents in existing sessions is unavailable.

It starts off on every host, including Tensorlake sandboxes. Turn it on in host settings or choose **Always allow on
&lt;host&gt; only** on a qualifying card. The
[helm, the program that shows all your hosts' sessions and serves the UI](/docs/how-it-works/the-pieces/), must still be
reachable to handle these requests.

**Run farhelm commands without asking** is broader: agents in that host's sessions can act on sessions on any of your
hosts and change or delete templates without asking. It starts off too. Turn it on only for a host whose agents you
would let do that unsupervised. Changing one switch does not change the other; you can change either while the other is
on. Both still ask about the [wrapper prefixes](/docs/using/approve-agent-requests/#the-card) described above and
[host changes](/docs/using/approve-agent-requests/#host-changes), and neither covers links. The
[YOLO launch rules](/docs/using/approve-agent-requests/#yolo-sessions-and-custom-commands) still apply.

If you **Adopt** a host that was reinstalled or replaced at the same address, both settings go back to off, like the
YOLO setting and both link settings: you have not decided about the new installation yet.

## Open links from a remote session

When an agent or a command in a remote session tries to open a web link through `BROWSER`, Farhelm asks to open it on
your machine. The card names the session and host and shows the site it would open and the full URL. Only HTTP and HTTPS
links are accepted. **Allow** opens it in your default browser from the Mac app, or in a new tab from Farhelm's web UI.
**Deny** opens nothing. Commands can also ask directly with `farhelm agent open-url -- https://example.com`, from any
session, including ones on your Mac. In a [sandbox or container](#agents-in-sandboxes-and-containers), its own link
permissions apply.

The host settings have two separate permissions, both off initially. **Open links without asking** covers public
addresses. **Open local-network links without asking** covers addresses on your own machine or private network; those
cards explain why they ask separately. **Always allow from** turns on only the permission that link needs. Neither
command permission above covers links. Farhelm decides which permission a link needs from the address as written,
without looking up where a name points, so a public-looking name can still lead to your private network.

With the matching permission on, the Mac app opens the link immediately. Farhelm's web UI still shows an **Open** button
because your browser requires a click; **Dismiss** refuses the request. Turning on the local-network permission lets
agents on that remote host make your browser load addresses on your machine or network without another approval.

A running agent gets forwarding at its next launch; new terminal tabs get it immediately. Sessions on your Mac open
links in your browser directly, as they always have. Your own `BROWSER` setting wins, even when empty. If Farhelm is
installed under a path with spaces or other special characters on the host, its sessions get no forwarding. A command
that waits for Farhelm learns whether you allowed or refused; a caller that starts it in the background can report
success before you answer. Codex's browser login does that. Claude Code 2.1.296 on Linux does not read `BROWSER` the way
most programs do, so its own opener does not forward; use the explicit Farhelm command instead.

In the Mac app, a localhost link from a remote session also offers **Tunnel and open** first. That opens the service on
the remote host; **Open** keeps the ordinary behavior of reaching the address on your Mac. The first tunnel needs your
acceptance of a security warning, and you can close tunnels from their list beside the session's terminal tabs or turn
them off under **Terminal** in settings.
[Work in a session](/docs/using/work-in-a-session/#open-a-remote-localhost-link) explains the access you grant. A host's
permission to open links without asking still opens the raw address; it does not create a tunnel automatically.

A login redirecting to `localhost` on the remote host is not forwarded automatically. Opening a link does not itself
connect that remote login callback to your machine.

## YOLO sessions and custom commands

On a host that asks you to [confirm YOLO launches](/docs/using/start-a-session/#confirm-a-yolo-launch), an agent cannot
start a YOLO session at all, with or without your approval. That includes a launch with
[custom permission flags](/docs/agents/#permission-modes), which always counts as YOLO. It cannot start a session that
runs a [custom command](/docs/agents/custom-commands/) there either, because Farhelm never reads a command to check
whether it runs in YOLO mode, and an agent's word on that is not enough. The same goes for copying a session that an
older version of Farhelm started, whose command line may not be the one Farhelm would put together today. To let agents
do any of these on a host, turn on its **Start YOLO sessions without asking** setting. Neither command permission waives
this requirement for creating, spawning or copying a session.

An agent can use a prefix already saved in a launch template; you approved the template when it was written. If it
supplies `--wrapper` itself, copies a session with a prefix, or inherits one from its own session, Farhelm asks you to
approve that prefix before starting a non-YOLO agent launch on a host that asks before YOLO. This does not let the agent
start a YOLO launch there.

[Restarting](/docs/using/stop-restart-resume/) a session that already exists is not affected: a restart resumes the
session with the settings it has saved, including a YOLO session. It gets the ordinary approval card unless one of the
two command permissions covers it.

## Next

[Manage hosts](/docs/using/manage-hosts/) covers the rest of a host's settings, and
[Launch templates](/docs/using/launch-templates/) what a template holds.
