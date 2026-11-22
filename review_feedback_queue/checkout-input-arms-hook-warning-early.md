# Checkout preparation input can trigger the conversation-hook warning before the agent starts

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

Enter during managed checkout preparation can make Farhelm warn about a missing hook report before the agent starts.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d
crates/farhelm-supervisor/src/service/core.rs:11678-11711,11901-11927,14643-14650,14679-14684;
crates/farhelm-supervisor/src/launch.rs:1182-1192,1889-1947,2195-2206;
crates/farhelm-supervisor/src/service/connection.rs:475-487,515-538;
crates/farhelm-supervisor/src/service/capture.rs:169-199,355-388; SPEC.md:3129-3153; SPEC_impl.md:3474-3478;
crates/farhelm-supervisor/src/service/handlers.rs:1564-1579,1667-1706; crates/terminal-host/SPEC.md:86-95,117-122

On a fresh managed checkout with Grok or Antigravity hook setup enabled, answering checkout preparation in the terminal
can produce a warning that the agent never reported, even though the agent has not started or received a prompt.
Reviewed commit 1a53d9d35889d7dde96e31320cc1681dd013737d. Successful setup arms the hook flag before terminal start. The
create confirms and publishes the child terminal while its shim can still be running the clone or user-configured
post-clone command on that same terminal. Those operations are synchronous in the shim and preparation can take longer
than 65 seconds. Input accepted on TerminalId::Agent starts the timer unless the selected agent's screen reader
recognized Waiting. There is no preparation or agent-start check on that path, and the timer checks only elapsed time,
the hook flag and missing capture. SPEC.md:3129-3153 and capture.rs:169-188 describe the clock as evidence the agent
received a line it could report on. This also applies if preparation finishes during the interval but the user has yet
to prompt the agent. Tie the diagnostic clock to the agent phase without blocking normal terminal input. The reviewer
suggested other for misleading guidance and a prematurely spent warning, confirmed by source conditional on a qualifying
Enter during preparation and no accepted report before expiry. No incidence asserted. ; Possible coverage: generic timer
antecedent predates this patch, but persistent setup newly arms it on these launches. The ordinary agent attach resolves
the published terminal by host liveness, with no preparation-ready gate. That makes the preparation input route
independent of display heuristics except for the existing Waiting exclusion.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-supervisor/HS-17`.
