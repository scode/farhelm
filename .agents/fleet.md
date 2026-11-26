# Running the agent fleet

The maintainer's map of the unattended agents and what they run on: plan executors, the landing monitor, their slots and
checkouts, the agent records they keep, the credentials, and the backup. Read it for the fleet flow (`AGENTS.md`,
"Flows"), and before acting on any request about looking after those agents: their status, a records backup, a token, a
slot, or a host.

NOTE: The fleet flow, a message that is "fleet flow" or starts with "fleet flow," (as with "todo flow," and the planning
flows), is a request to get ready, not to do anything by itself. Read this file and every file under "Reading list" in
full. Then, if the message asked for something after the comma, do that; otherwise say in a few lines that you are ready
and what you can help with, and wait for the next request. Loading the context runs nothing that changes state, starts
no flow and no session, and reports no status unless asked: the point is an agent that knows the system before the
maintainer's next request, whatever it is.

## Reading list

- this file
- `.agents/agent-records.md`: the records repository, the host files, the records tool, tokens and the backup
- `plans/AGENTS.md`: the opening sections ("Unattended means no questions", "Layout", "Notifications") and "Flows and
  reading lists", for what each planning role does and which files it reads
- `plans/.agents/outside-files.md`: what executors and the monitor keep in their records, and when they push
- `plans/.agents/release-help.md`: releasing, holding and unholding plans

A later request that starts a flow (planning, reviewing plans, releasing a claim) reads that flow's own reading list as
usual; this list does not replace it.

## The pieces

- **GitHub main** holds the plan queue (`plans/queue/INDEX.md`, the plan files, reports) and is the only place plan
  state lives. Every state change goes through `scripts/plans-queue.py`.
- **Slots.** Each executor and the landing monitor runs in a slot: a checkout of this repository listed by name in the
  host's `~/.fh-records/config` (`[[slot]]` entries, `exec-<N>` for executors and `landing` for the monitor). Read that
  file for the slots and their checkout paths, without printing its `[artifacts]` values into anything public. One agent
  runs per slot. The config may list more executor slots than have checkouts, so another executor needs only a checkout
  at a listed path.
- **Executors** are interactive agent sessions, one per executor slot, started by the maintainer in that checkout with
  "drain the plans and keep monitoring" (or one of the single-plan requests in `plans/AGENTS.md`).
- **The landing monitor** is one interactive session in the landing slot's checkout, started with "monitor for complete
  plans". It lands what executors deliver and answers most blocks.
- **Agent records** (`agent-records` in Cloudflare Artifacts, namespace `records`) hold every slot's logs, plan notes
  and landing rounds, plus the attended flows' working logs. On the host running a slot, that slot's clone under
  `~/.fh-records/clones/<slot>/` is the live copy.
- **The maintainer's host** holds the Cloudflare API token (`~/.fh-records/cloudflare-api_token`), which can create and
  delete repositories and mint repo tokens, and so never goes to another host. The maintainer's own attended sessions
  (planning, reviews, triage, releases) run here too.
- **Notifications** go to the maintainer's ntfy topic (`plans/AGENTS.md`, "Notifications"); never read or print its
  file.
- **Other hosts.** Today every slot runs on the maintainer's host. Running executors or the monitor on VMs is planned
  (TODO.md, "Run plan executors and the landing monitor on other hosts") and not built: each VM would need its own
  config with one slot, a repo token, the ntfy topic file, a GitHub login and a checkout.

Keeping the executors and the monitor running is the maintainer's job, done out of band. Nothing unattended judges
whether another agent is alive, and taking a claim away from an agent is the maintainer's manual step, after that agent
is gone.

## Common requests

- **Status** ("how are the executors doing", "plan status"): use the `farhelm-plan-status` skill.
- **A records backup** ("run a records backup into `<dir>`"): from the root of a checkout of this repository, run
  `cargo xtask records backup --dest <dir>` with `<dir>` absolute. It needs the config's `[artifacts]` section and the
  API token file, makes `<dir>` a bare repository the first time, prints nothing on success, and exits non-zero (with a
  notification) on failure. A refused non-fast-forward fetch means the remote's history was rewritten: the backup kept
  the old history, so report it to the maintainer and never "fix" it by forcing.
- **What an agent did** ("show me exec-2's log", "what happened in plan X"): read the files in the slot clones as plain
  files, without running git there, or clone `agent-records` into your scratch directory as `.agents/agent-records.md`,
  "Attended flows' logs", shows, and use `git log` and `git grep` on that clone.
- **Another executor**: make a checkout of this repository (colocated jj) at the path an unused `exec-<N>` slot lists,
  if there is none, and tell the maintainer it is ready; the maintainer starts the session. Its first records call
  builds the tool and makes the slot's clone.
- **A stuck or dead agent**: stopping or restarting a session is the maintainer's; do not kill agents' processes. To
  free its claim, follow "release plan X" (`plans/.agents/release-help.md`), which happens only once the holder is
  stopped.
- **A records push that fails or refuses**: read the tool's message. A `records: set aside` line names where a slot's
  stale edits were saved. A refusal because another slot changed a record being pushed means two writers, which only the
  maintainer can sort out. Never force-push or edit the remote by hand.
- **Tokens** ("what do I need for a token", "renew the token", "a token for a VM"): `.agents/agent-records.md`,
  "Tokens", has the dashboard link, the one permission, and the API calls. Anything that handles a token is a script
  file that never prints it.
- **A new host**: not built yet (see the other-hosts item under "The pieces"). Say what it would take and plan it with
  the maintainer rather than improvising.

Fence: loading the fleet flow's context changes nothing. Never print, log or commit a token or the ntfy topic. The
executors' and the monitor's sessions and checkouts are theirs: read them, do not change them unless the maintainer
asks. The live install stays off-limits (`AGENTS.md`, "The live install is off-limits").
