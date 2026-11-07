# Cancelled ssh capture can retain local readers

Reviewed commit: 744ea4122d033ef616454fe592ecb2173593736c

## TLDR

Cancelling an SSH capture may leave local helpers or pipe readers alive beyond the caller. Their actual surviving
lifetime remains unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location: `crates/farhelm-helm/src/tensorlake/shell.rs:175 at 744ea4122d033ef616454fe592ecb2173593736c`.

Source assessment (read with the later corrections below) (review record g052/l1, event 13):

Reviewed commit: 744ea4122d033ef616454fe592ecb2173593736c; behavior rechecked at inclusive pinned tip
8a8c9579e04299410652f0e4681975a7198dfc7b. TLDR: deleting a recipe while its snapshot builder is trying to reach a jump
host can finish cancelling the build while a local SSH helper and its stderr reader remain alive until that helper's
independent connection attempt ends. This is existing l1 refined by synthesis, not new discovery. Application mechanism
at original hash: shell.rs:152-175 creates SSH with kill_on_drop(true), isolates its process group, spawns through
SystemLauncher, then awaits capture_child. build.rs:713-724 drops that future on cancellation; :1085-1100 uses this
wrapper for scripts; clean_for_delete at :1257-1279 signals cancellation and awaits the build task. capture_child in
provisioning/backend.rs:1686-1730 spawns two readers and owns the child, but only calls group teardown on explicit wait
errors, stream errors or timeout. Dropping it detaches reader JoinHandles and drops the direct child; there is no
cancellation supervisor or group drop guard. backend.rs:1967-1979 sends group-wide SIGKILL only in terminate_child;
merely assigning process_group(0) at :479-485 does not invoke it. At tip the corresponding code is
build.rs:804-814,1177-1190,1347-1369; shell.rs:152-175; backend.rs:495-521 (SystemLauncher calls command.spawn, group
isolation only),1748-1788 and 2015-2046. Awaiting the build therefore does not await local group/reader teardown. Later
remote builder reconciliation cannot kill a local jump helper.

Dependency ownership recheck: Cargo.lock pins Tokio 1.53.2 at both hashes. Read that exact cached primary source under
tokio-1.53.2/src: process/mod.rs:1106-1128 drops ChildDropGuard by calling only its inner kill;
process/unix/mod.rs:169-172 delegates to std_child().kill, not a process-group operation. runtime/task/join.rs:11-36
documents that dropping JoinHandle detaches the task; its Drop at :357-364 does not abort it. Tokio's best-effort
direct-child reaping is not descendant termination. The earlier generic Tokio 1.48 documentation was not needed as a
substitute once locked source was available.

Supported helper producer now identified from OpenSSH V_9_6_P1 primary source, which is an example supported
implementation, not a claim about every installed version. ssh.c:1262-1284 turns ProxyJump into a separate ssh -W
helper. sshconnect.c:193-234 forks/execs that helper, rewires stdin/stdout and retains captured stderr for
non-ControlPersist connections; :257-265 only sends SIGHUP when its explicit ssh_kill_proxy_command cleanup runs. A
SIGKILL of the parent cannot execute that cleanup. The helper performs its own connection setup before opening the
stdio-forwarding channel: ssh.c:1551-1555 calls ssh_connect before :1696 calls ssh_session2; :1862-1880 installs stdio
forwarding later. sshconnect.c:476-478 calls timeout_connect for its jump-server TCP attempt. misc.c:359-394 either
blocks directly in connect with no application timeout or waits on only the connection socket; it does not watch closed
proxy stdin/stdout in that phase. Thus an in-progress jump-host connection is a concrete source-supported interval where
killing the outer SSH does not terminate the helper or close its inherited stderr. Its independent TCP/connect bound,
not Farhelm's cancelled command bound, ends that interval. Primary URLs:
https://raw.githubusercontent.com/openssh/openssh-portable/V_9_6_P1/ssh.c ;
https://raw.githubusercontent.com/openssh/openssh-portable/V_9_6_P1/sshconnect.c ;
https://raw.githubusercontent.com/openssh/openssh-portable/V_9_6_P1/misc.c . ProxyJump remains supported by the
project's retained user-config policy, tensorlake/ssh.rs:21-26; the OpenBSD ssh_config manual likewise describes it as a
separate SSH connection to the jump host.

Important narrowing: this does not prove every proxy survives, both readers remain, or helpers persist forever. The
ordinary nc producer was specifically set aside in c2 because it observes closed stdout. Proxy children replace stdout
with their private pipe to SSH; the demonstrated inherited Farhelm pipe is stderr, so the indefinite two-reader claim
from the initial hypothesis is withdrawn. With an already connected cooperative helper, normal EOF handling can end it
promptly. With a pending connection, the helper plus stderr drain can outlive completed recipe cancellation; actual
elapsed duration and deployment reproduction are unverified after investigation. No runtime/process/network experiment
was run. Static source settles the producer/ownership mechanism; an isolated pending-connect proxy fixture would verify
duration and fd lifetime dynamically, but that is outside this charter. No live gateway premise is needed for the
narrowed pending-jump-connect case.

Suggested bucket: other for local process/reader resource retention, with no established security or user-work-loss
consequence. Confidence: confirmed static cancellation-cleanup bypass and supported pending-connect producer; runtime
duration unverified as stated. suggested fix: keep the capture supervisor owned until explicit cancellation triggers
isolated-group kill, direct-child reap and bounded reader termination, then resolve the build task; a
dependency-injected helper fixture can validate lifecycle later. proposed_drop: false. possible_cover: shared
provisioning capture review may describe the same primitive; no queue/coverage searches performed. SPEC_impl.md
Tensorlake snapshot builds promises cooperative cancellation of script waits; no specification exception permits
dropping local ownership. The group isolation is useful for explicit timeout paths but does not repair this cancellation
path.

Later qualification (review record g052/l1, event 21):

Reviewed 744ea4122d033ef616454fe592ecb2173593736c. Corrects line references in earlier l1 events; no NEW discovery and
no change in mechanism, bucket or uncertainty. Numbered pinned source shows command configuration at shell.rs:152-160,
spawn :161-167, capture_child await :168, classification :175-182; previous references to the capture await at :175 were
imprecise. TLDR remains retained local SSH ProxyJump helper and stderr reader after deleting/cancelling a connecting
build. build.rs:713-724 drops this future, capture_child backend.rs:1686-1730 has only explicit branch cleanup, and
group-wide kill at :1967-1979 is bypassed by drop. Exact locked Tokio and OpenSSH pending-connect producer evidence is
preserved in the preceding l1 synthesis event: ProxyJump helper in its independent connection setup cannot yet observe
closed proxy stdio and retains captured stderr until its connect resolves. Bucket other; static bypass/producer
confirmed, actual runtime duration remains unverified after investigation, with no process experiments. No claim of both
readers, perpetual persistence, security impact or user-work loss. proposed_drop false; possible_cover shared
provisioning capture. Fix remains explicit cancellation-owned group termination, reap and bounded reader teardown before
the build task completes. Earlier detailed primary-source investigation remains operative; only project shell.rs line
coordinates are corrected here.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g052/l1`; supporting records
`g043/g052:l1`, `g046/f3`, `g051/i1`. The retained private log preserves earlier wording and its corrections. This
record does not claim runtime reproduction.
