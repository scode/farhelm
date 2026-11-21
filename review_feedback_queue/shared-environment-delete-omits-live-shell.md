# Connected environment Delete still omits the selected shell and tabs from the stop warning

Reviewed commit: 05ae73f23eb2cbb7ab284ca42473d66a771207a9

## TLDR

Deleting a shell in a shared environment can stop its live processes even though the confirmation only mentions stopping
processes if the whole environment is deleted.

## Details

**highest**. Confirmed by source and contract inspection.

Deleting a Bash or Zsh session through a reachable environment can end its shell and terminal tabs even when the
confirmation speaks of stopping processes only under the last-session condition. Source-confirmed and estimated highest
for unspoken destructive consequence. The single question at delete.rs:174–186 and marked question at bulk.rs:280–285
use environment_delete::Asked's consequence. Environment_ui.rs:578–588 says “Delete this session and its checkout?” and
mentions “every process” only if the environment is destroyed. If a running shell has a sibling or its displayed
liveness changes, environment_delete.rs:204–217 supplies no ordinary liveness guard, helm sessions.rs:3976–4005 passes
the connected lane to sessions/sandbox.rs:244–279, and supervisor service/handlers.rs:1433–1469 plus
service/teardown.rs:531–561 end the selected session's processes even when the environment stays.
SPEC.md:4563–4565,4575–4589 provides a supported way for multiple shell/agent sessions to share an environment;
:2103–2105,2128–2141 states the live-consequence requirement. Possible_cover: the specific environment text at
SPEC.md:1575–1579 says “The confirmation states both outcomes: the session and its checkout are deleted, and if it is
the last session in the sandbox, the sandbox, every process and file in it and its snapshots are deleted too”;
:5123–5128 and :2323–2329 describe the same outcomes. They do not expressly waive the live-process warning. The
whole-environment Delete separately warns of every process and is excluded. Shell-permissions/f5 agrees this is distinct
from the correctly warned Stop and suspend. Shell-runtime/f4 and shell-permissions/f8 describe the shared process-reaper
history-loss defect, not an authorization fix or an extra requirement to enumerate history. The ordinary wording
follow-up f1 did not edit this question or its backend, and no later change is present through pinned tip
8bcca67df1afe6eb0f184ef73b9952ed7ebb329f.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-presentation/f4`.
