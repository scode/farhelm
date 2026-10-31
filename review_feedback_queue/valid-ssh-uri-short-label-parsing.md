# Valid SSH URI forms break short labels

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Valid SSH URIs with a trailing slash, service-name port or empty port can show an incorrectly shortened host label.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

Matching primary OpenSSH/libc/service parsing confirms these accepted input forms. The short-label helper does not strip
each URI form before IP/hostname shortening. These are variants of the URI-normalization defect, separately from
abbreviated IPv4 policy. Full destination/operation identity remains available; no wrong-target dispatch was
established.

Source locations: `crates/farhelm-proto/src/host_name.rs:64-76`. Historical revision qualifiers in those references are
retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed full commit d6645f771bee8ed03a66486ef8755834937d759d; introduced by 993cd95e8af93f50bbf5797e299409397ed5010b.
Exact confirmed inputs: ssh://192.0.2.7:ssh -> short label 192; ssh://build:ssh -> build:ssh; ssh://192.0.2.7: -> 192;
ssh://build: -> build:. Service-name route/provenance remains as established in discovery3: OpenSSH V_9_6_P1
misc.c:515-526 and 1048-1054 accepts getservbyname ports, standard Debian netbase 6.6 services:24 maps ssh to TCP22,
glibc 2.39 files-service.c:32-53 handles that mapping. Discovery4 adds the empty-port acceptance boundary under this
same ID: OpenSSH misc.c:1003 initializes port=-1; hpdelim2 at 1041-1044 separates a terminal colon from the hostname;
the nonempty-tmp block 1048-1062 is skipped; success at 1064-1079 returns host with port=-1/path=NULL.
readconf.c:3275-3298 accepts this, and ssh.c:1115-1126 leaves configured/default port unchanged. Farhelm's is_port at
host_name.rs:75-76 rejects empty or nondigit suffixes, so destination_host retains the colon/suffix. The leftover suffix
defeats IpAddr parsing and host_name.rs:46-48 truncates a canonical IPv4 spelling. Same overly narrow URI port
recognizer, not a separately counted finding; distinct from f1's abbreviated-address classification and a027's
terminal-slash case. Bucket other; confidence confirmed source behavior for the exact supported inputs. No missing
external parsing premise. Impact is persistent wrong display text only: full destination and registry-ID operational
routing are unchanged; no wrong-target/security/user-work-loss claim. proposed_drop null. possible_cover: none found for
service-name ports; SPEC.md:216-222 does not explicitly discuss empty ports but promises derivation from the SSH
hostname and whole IP addresses, so any empty-port scope interpretation is noted rather than treated as coverage.
Completed discovery4 searches of SPEC.md, SPEC_impl.md, TODO (Planned empty), all plans/queue, BUGS, INDEX and
TRIAGE_OUTCOMES found no exact acceptance or planned repair for the empty-port boundary. This completes the coverage
check left pending in the immediately preceding event. Wrong derived text persists across refresh/reconnect, so the
rare-self-correcting filter does not fully match. No runtime experiment or test pass is claimed. Original
events/provenance remain append-only; first malformed private-event submission was corrected immediately and did not
erase or append a finding event.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g143/f3`, `a027/f7`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
