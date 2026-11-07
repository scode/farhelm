# Sandbox ssh agent key ordering unresolved

Reviewed commit: 744ea4122d033ef616454fe592ecb2173593736c

## TLDR

Configured SSH-agent keys can precede Farhelm's selected key and may prevent sandbox access. The current gateway's
authentication behavior remains unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: high.

Source location: `crates/farhelm-helm/src/ssh.rs:279 at 744ea4122d033ef616454fe592ecb2173593736c`.

Source assessment (read with the later corrections below) (review record g043/l1, event 12):

Reviewed commit: 744ea4122d033ef616454fe592ecb2173593736c. TLDR: Tensorlake snapshot builds can fail even after the user
registers the correct Farhelm SSH key when their ordinary SSH configuration names a key loaded in ssh-agent. The same
key policy later affects sandbox-host connections. Mechanism: assigned ssh.rs:279–287 preserves user configuration and
only adds caller options. tensorlake/ssh.rs:71–90 supplies IdentityFile for Farhelm and IdentitiesOnly=yes, but does not
disable IdentityAgent or remove configured identities. Its module docs at lines 4–8 say the gateway immediately refuses
an unregistered first key; lines 21–23 incorrectly assume configured keys follow Farhelm. Primary static source check:
OpenSSH 9.6p1 sshconnect2.c pubkey_prepare,
https://github.com/openssh/openssh-portable/blob/V_9_6_P1/sshconnect2.c#L1563-L1693, moves configured identities
matching agent keys into the preferred queue at 1641–1651 and appends the remaining file-only identities at 1692–1693.
IdentitiesOnly filters extra agent keys but does not stop that promotion. A Host * IdentityFile for an agent-loaded
personal key is therefore offered before the file-only Farhelm key, regardless of argv position. This corrects the
earlier l1 withdrawal: the source comment was not an adequate ordering refutation. Temporal: introduced for builders at
744ea4122d033ef616454fe592ecb2173593736c; 88bf83e267b5066815448885c61ed4148b851930 shares the options with hosts; pinned
tip 8a8c9579e04299410652f0e4681975a7198dfc7b tensorlake/ssh.rs:95–105 still has the same identity policy. Suggested
bucket: high, because affected users cannot build or connect, with no established data-loss/security route. Confidence:
likely; OpenSSH ordering is confirmed in readable upstream source, gateway first-key rejection is the repository
documented measured premise rather than a new live observation. proposed_drop: false. possible_cover: possibly assigned
Tensorlake SSH/client groups; queue/coverage not searched. Fix direction: make the Tensorlake authentication policy
actually exclude or deprioritize other configured/agent identities while preserving the supported proxy configuration;
do not rely on IdentitiesOnly or argv ordering alone. Verification when runtime work is authorized: use a local SSH
fixture with a configured agent-loaded unrelated key and a file-only Farhelm key, and assert which key is offered first;
no process or network experiment was run in this review. Unverified after investigation qualification: static repository
and upstream source establish the option policy, key ordering and the documented gateway rule. No readable gateway
server implementation is available in the assigned repository to independently verify that it still closes on the first
unregistered key, and this source-only charter forbids a live gateway experiment. A retained authorized gateway
observation, or gateway authentication implementation, would settle that last premise. This limitation affects
confidence, not the established high-priority consequence if the documented rule holds.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g043/l1`; supporting records
`g046/f1`, `g052/l3`, `g043/g046:f1`, `g047/c2`. The retained private log preserves earlier wording and its corrections.
This record does not claim runtime reproduction.
