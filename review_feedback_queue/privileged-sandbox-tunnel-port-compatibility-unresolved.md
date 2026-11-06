# Privileged sandbox tunnel port compatibility unresolved

Reviewed commit: 68790603037bb51ccd8655e4a948c7ffb747760a

## TLDR

Recipe tunnels accept privileged sandbox listener ports that the gateway may refuse. Actual gateway enforcement remains
unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location: `crates/farhelm-proto/src/tensorlake/recipes.rs:533-543 at 68790603037bb51ccd8655e4a948c7ffb747760a`.

Source assessment (read with the later corrections below) (review record g063/d3, event 19):

Reviewed commit: 68790603037bb51ccd8655e4a948c7ffb747760a. TLDR: A recipe accepts low sandbox tunnel ports such as 80
and 443, but those forwards may fail when opened through the sandbox's SSH connection, requiring a different port or a
clearer refusal. Unverified after investigation: the actual Tensorlake gateway's privilege policy is not established, so
this is not a claim that those ports demonstrably fail. Mechanism and checks: check_recipe in the assigned proto file
accepts all nonzero sandbox ports except 22; when the consumer arrives at c340166d754d3edb3de7fd538c6408948d3536c6,
tensorlake/tunnels.rs:237-246 emits ssh -R 127.0.0.1:<sandbox_port>:<host>:<port>. tensorlake/ssh.rs:127-131 uses
<sandbox>@<sandbox>.sandbox.tensorlake.ai, not a root login; its module docs say the connection terminates at
Tensorlake's gateway. The primary OpenSSH manual, https://man.openbsd.org/ssh.1#R, states privileged remote ports
require root login. SPEC_impl.md, Tensorlake recipe tunnels and unowned sandboxes, lines 5999-6009 at the consumer
revision, confirms that -R path and failure/retry behavior but does not describe a low-port exception or binding
capability. Searched pinned Tensorlake/provisioning source and SPEC documents for ip_unprivileged_port_start, privileged
port, and CAP_NET_BIND_SERVICE: no matching policy. Official Tensorlake site describes full port forwarding, but that
broad statement does not settle port privilege handling. Official docs index https://docs.tensorlake.ai/llms.txt
identifies pty-sessions.md as the SSH documentation; attempts to retrieve that page and a guessed /sandboxes/ssh page
were inaccessible through the read-only source tool. Searches did not locate public gateway enforcement source. Missing
premise: Tensorlake's gateway applies the usual non-root remote-forward restriction rather than implementing a
privileged/custom listener path that accepts these ports. Available Farhelm code cannot settle external gateway policy,
and generic OpenSSH documentation is not proof of that implementation. An accessible exact gateway policy/source,
retained documented low-port result, or separately authorized narrow gateway check would settle it. Suggested bucket:
other, for a possible accepted-setting failure/retry only; no security or user-work-loss route. Confidence: likely
conditional on that explicit missing enforcement premise; do not treat as verified failure. Suggested fix only if
premise holds: validate the supported sandbox listener range or implement an explicitly supported low-port path; do not
reduce the target service port range merely because the sandbox listener is privileged. Temporal qualification:
validation introduced here; actual -R consumer starts at c340166d754d3edb3de7fd538c6408948d3536c6. The validation is
unchanged at pinned tip; current consumer was inspected earlier this turn and still emits the same host/port mapping.
proposed_drop: false pending parent disposition. possible_cover: unknown, no queue/plan/filter searches. Novel
unverified-after-investigation discovery finding for discovery pass 1; d3 lead is resolved to this qualified finding. No
runtime, process, package or network experiment performed.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g063/d3`; supporting records
`g046/f8`. The retained private log preserves earlier wording and its corrections. This record does not claim runtime
reproduction.
