# Agent records

Where agents keep their logs and notes outside the repository, and everything about the `agent-records` repository that
holds them: its layout, the host files, the records tool the unattended flows use, plain git for everyone else, tokens,
and the backup. Read it before setting up or changing records on a host, minting or replacing a token, reading records
for status or history, or keeping an attended flow's log (`AGENTS.md`, "Flows").

NOTE: This is not where the planning flows' rules live. What executors and the landing monitor write, and when they
push, is in `plans/.agents/outside-files.md` and the files it points to. This file is the reference behind them.

## What it is

Plan executors, the landing monitor, and four attended flows (triage, releases, deflake runs, run all tests) keep small
text records: working logs, notes, and a landing round's text files. They live in one git repository, `agent-records`,
hosted in Cloudflare Artifacts (namespace `records`), so that an agent on any host can continue what an agent on another
host started, and so the history stays searchable in one place. Plan state is not in it: claims, Blocked questions,
Decisions and reports live on GitHub main and nowhere else.

Everything sits under a `farhelm/` root directory, so another project could share the repository later:

```
README.md                         the layout, for anyone reading with plain git
farhelm/
  plans/<slug>/log.md             a plan's working log, continued by each claim holder in turn
  plans/<slug>/notes/             what a later claimer needs that is not log text
  executors/<slot>/log.md         one executor slot's log (retired-<checkout>/ holds logs from before slots existed)
  landing/log.md                  the landing monitor's log
  rounds/<n>/                     one landing round's plans.txt, base.txt, prs.txt, tested_commits.txt and notes/
  flows/triage/<checkout>/log.md          a checkout's triage working log
  flows/release/<checkout>/log.md         a checkout's release working log, and vX.Y.Z-changelog.md beside it
  flows/deflake/<checkout>/log.md         a checkout's deflake working log
  flows/run-all-tests/<checkout>/log.md   a checkout's run-all-tests working log
  flows/unsorted/                 files the one-time import could not place
```

Bulky or host-bound things never go in: test runs and their evidence, deflake state, landing diffs, screenshots,
transcripts, watchdog status files, build output. Losing those with a host is accepted.

## Host files

Everything a host needs is in `~/.fh-records/` (mode 700; files mode 600). The tokens and the config are never printed,
logged or committed.

- `config`: the records tool's settings, in TOML. `remote` is the repository's git URL exactly as the Artifacts API
  returned it, `token_file` the repo token to use. An `[artifacts]` section (`account_id`, `namespace`, `repo`,
  `api_token_file`) is needed only where `backup` runs. Then one `[[slot]]` per executor or monitor checkout, with a
  `name` and the checkout's absolute `checkout` path: `landing` for the monitor, `exec-<N>` for executors. Slots whose
  checkout does not exist yet are fine, so a host's config can list more executor slots than it runs; the tool makes a
  slot's clone only when that slot first uses it. A slot name must be unique across every host that shares the
  repository: commits carry it as their author, and the tool treats a commit with its own slot's name as its own.
- `agent-records_token`: a repo token for `agent-records` with `write` scope, the whole string the API returned
  (including its `?expires=<unix seconds>` suffix). Repo tokens live at most a year; replace it before it expires
  ("Tokens" below).
- `cloudflare-api_token`: the Cloudflare API token, only on the maintainer's own host. It can create and delete repos
  and mint repo tokens, so it never goes to another host.
- `clones/<slot>/`: the records tool's clone for each slot. On the host running a slot, its files are the live copy of
  that slot's records, newer than anything pushed.
- `clones/attended/<checkout>/`: a plain git clone for one checkout's attended flows ("Attended flows' logs" below).

`~/.fh-records/` is a location a rule names, like goal files and working logs (`.agents/machine.md`, "Agent scratch
space"); its clones are small.

## The records tool (unattended flows only)

`cargo xtask records <command>` exists for executors and the landing monitor, and nothing else uses it: its purpose is
to keep their instructions stable, naming records rather than a repository, a URL, a token or a git command, so the
store and the push mechanics can change behind it without rewriting the flows. It is built from the checkout it runs in
(the `xtask/` crate, its own cargo workspace, built into `target/xtask/`), so each checkout runs its own version. Run it
from the checkout's root directory: cargo resolves the alias's paths from the working directory, so it fails from a
subdirectory. One consequence: a plan stack that changes `xtask/` runs its own version of the tool, including in the
landing monitor's checkout while that stack is being landed.

- Record names: `plan/<slug>`, `executor` (the slot's own log), `landing`, `round/<n>`. Executor slots may push `plan/*`
  and `executor`; the landing slot may push `landing` and `round/*`.
- `path <name>` brings the slot's clone up to date and prints the record's directory. Files with local edits are left
  alone, except where the remote changed the same file: one writer per record means such an edit can only be a stale
  leftover from a claim this slot no longer holds (a released plan, or one it blocked or gave back that another slot
  then took), so the remote's version wins, and the local one is copied to the clone's `.git/records-set-aside/<time>/`
  with a line on stderr saying so. A remote change that only this slot's own commits made (a push the remote accepted
  while the tool saw it fail) is not someone else's: a local file that matches it is taken as pushed, and one the agent
  has written to since stays as it is.
- `push <name>... -m <message>` brings the clone up to date the same way, except that a remote change to a record it is
  pushing, overlapping a local edit, refuses the push: that would be two writers on one record. It then commits exactly
  the named records' directories on top of the remote's head and pushes without force, retrying with backoff when
  another slot pushed first or the repository is busy with another push. Artifacts takes one push to a repository at a
  time: six slots pushing at the same moment all got through in about twelve seconds when measured. Edits that arrive
  while it runs stay local for the next push.
- `backup --dest <dir>` is described under "Backup".

The tool sends the repo token to git as an `Authorization: Bearer` header through the child process's environment, so it
never appears on a command line or in a clone's config. Its module docs (`xtask/src/`) have the full contract.

## Reading records

Reading needs no tool. The slot clones on a host are the live view of what its agents are doing right now: read their
files, but never run git in them, since even a read-only `git status` can take the index lock while the tool is pushing.
For history, use plain git on a clone of `agent-records` of your own (or your checkout's attended clone) after a fetch;
that gives each record as of its last push. Artifacts has no search over file contents, so use `git log`, `git grep` and
friends on a clone. A status overview needs only GitHub main (plan states, claim ids and claim history) and these
records: the executor logs say which slot took which claim id, the plan logs what each holder did, and the push times
when each agent last did something visible.

## Attended flows' logs

Triage, releases, deflake runs and run all tests keep their working logs in the records, in their checkout's attended
clone, `~/.fh-records/clones/attended/<checkout>/`, where `<checkout>` is the checkout directory's name. Each flow's own
file names its path in the clone. They use plain git, not the records tool. Only that checkout's agent writes there, one
agent per checkout as `AGENTS.md` requires, so a clone has one writer.

The first time a checkout needs its clone, create it with these commands, filling in the placeholders with literal
values: `<remote>` is the `remote` line of `~/.fh-records/config`, `<home>` the home directory, `<repo-checkout>` this
checkout's absolute path, and `<checkout>` its directory name. The credential helper answers git with the token file's
contents the way Cloudflare's Basic-auth rule requires (the part before `?expires=`).

```
git clone -c credential.helper= -c 'credential.helper=<repo-checkout>/scripts/records-git-credential.py <home>/.fh-records/agent-records_token' <remote> <home>/.fh-records/clones/attended/<checkout>
```

Then, from the flow's start: write the log at its path in the clone as you go, and push it

- right before anything the flow changes on GitHub (a PR pushed, opened, marked ready or merged, a release tag, a queue
  verb);
- once when the flow ends;
- whenever the maintainer says "sync", "sync state", "sync please" or anything like it.

To push, run these one at a time: `git -C <clone> add farhelm/flows`, then
`git -C <clone> commit -m '<flow> <checkout>: <what>'` ("nothing to commit" is fine: go on), then
`git -C <clone> pull --rebase --autostash`, then `git -C <clone> push`. Never force. Each checkout writes only its own
directories, so the rebase cannot conflict; if it ever does, stop and tell the maintainer. A failed push (an expired
token, say) is reported to the maintainer and does not stop the flow: the log is still on disk, and the next push
carries it.

When the clone cannot be made (no `~/.fh-records/config` or token on this host yet, or the clone fails), tell the
maintainer, keep the log meanwhile where these flows kept it before records existed, beside the checkout as
`farhelm-<flow>-log-<checkout>.md` (a stable path a compacted session finds again, unlike a scratch directory), and move
it to its place in the clone once the clone exists; this does not stop the flow either.

## Tokens

The Cloudflare API token needs exactly one permission: Account → Artifacts → Edit, on the one account. That covers
creating the namespace and repositories and minting and revoking repo tokens. Read adds nothing. This account-token
template link opens the dashboard's create-token form already filled in (confirmed working on 2026-11-26):

```
https://dash.cloudflare.com/?to=/:account/api-tokens&permissionGroupKeys=%5B%7B%22key%22%3A%22artifacts%22%2C%22type%22%3A%22edit%22%7D%5D&name=agent-records-setup
```

The permission dropdowns can look empty on first load; "← Edit token" and back fixes it. Put the token in
`~/.fh-records/cloudflare-api_token`, mode 600.

Repo tokens come only from the REST API, authenticated with the API token as `Authorization: Bearer`:
`POST https://api.cloudflare.com/client/v4/accounts/<account_id>/artifacts/namespaces/records/tokens` with JSON
`{"repo": "agent-records", "scope": "write", "ttl": 31536000}` (`scope` is `read` or `write`; `ttl` 60 seconds to a
year, in seconds). The token is `result.plaintext`; keep the whole string. `GET .../repos/agent-records/tokens` lists a
repo's tokens and `DELETE .../namespaces/records/tokens/<id>` revokes one. The account id comes from
`GET https://api.cloudflare.com/client/v4/accounts`. Anything that handles a token is a script file that reads the API
token from its file and writes the result to its file without printing either (`AGENTS.md`, "Claude Code shell
commands"). A host's write token is replaced the same way: mint a new one, write it over the old file, then revoke the
old one.

## Backup

Artifacts has only `read` and `write` repo tokens, and nothing documented rules out a force push with a write token, so
history is kept safe by a backup that never follows a rewrite. `cargo xtask records backup --dest <dir>`, run from any
checkout on a host with the config's `[artifacts]` section and the API token file, mints itself a short-lived read token
and fetches into the bare repository at `<dir>` (creating it the first time) without force. If the remote's history was
ever rewritten, the fetch is refused and the backup keeps the history from before it. It exits non-zero and sends a
notification (`plans/AGENTS.md`, "Notifications") when anything fails. Scheduling it, nightly or so, is the
maintainer's; it assumes nothing about which host runs it.

Whether a write token can actually rewrite history was checked once, on 2026-11-26, on a throwaway repository: it can. A
`write` repo token force-pushed a non-fast-forward update to `main` and deleted a branch without complaint. So the
backup is load-bearing, not just insurance, and the same run confirmed it: after the forced update, `backup` refused the
fetch and kept the history from before it.
