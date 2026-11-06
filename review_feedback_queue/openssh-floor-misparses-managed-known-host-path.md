# Openssh floor misparses managed known host path

Reviewed commit: 88bf83e267b5066815448885c61ed4148b851930

## TLDR

The supported older OpenSSH parser can misread managed known-hosts paths containing quoting characters, preventing
normal sandbox access.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location:
`crates/farhelm-helm/src/ssh.rs:284-287,296,332,392,612-615 at 88bf83e267b5066815448885c61ed4148b851930`.

Source assessment (read with the later corrections below) (review record g046/f10, event 24):

Reviewed commit: 88bf83e267b5066815448885c61ed4148b851930. TLDR: Tensorlake hosts fail to connect on the declared
supported OpenSSH 7.6 client when the helm state-directory path contains a double quote or a literal backslash; the
supplied path is parsed as a different file or rejected. Mechanism: ssh.rs:612-615 doubles literal backslashes and
encodes quotes as backslash-quote; ssh_config_path_option:392 wraps that string in quotes. This commit adds those
Tensorlake IdentityFile/UserKnownHostsFile values to shared and unshared host argv at 284-287,296,332 via
tensorlake/ssh.rs:95-105. Supported upstream V_7_6_P1/readconf.c:994-1005 parses IdentityFile with strdelim, and
1038-1057 does likewise for known-host file entries. V_7_6_P1/misc.c:167-201 removes enclosing double quotes by
searching for the next quote with strpbrk; it never consumes backslash escapes. Thus a backslash in an actual path
remains doubled in the parsed value. An escaped interior quote closes the token early; remaining path text triggers
readconf.c:1598-1601's fatal garbage-at-end check. The identity loader at V_7_6_P1/ssh.c:1837-1855 only applies tilde
and percent expansion before key_load_public, not a second backslash decoding, so the doubled backslash selects a
nonexistent/different identity path. Exact primary URLs:
https://raw.githubusercontent.com/openssh/openssh-portable/V_7_6_P1/readconf.c ;
https://raw.githubusercontent.com/openssh/openssh-portable/V_7_6_P1/misc.c ;
https://raw.githubusercontent.com/openssh/openssh-portable/V_7_6_P1/ssh.c . SPEC_impl.md Helm ↔ supervisor transport
declares an OpenSSH 7.6 floor; Tensorlake snapshot builds/hosts promises correctly encoded local key/known-host paths,
and this automatic-authentication route is not covered by SPEC.md's narrow copied-command configuration trust exception.
To isolate the newly affected route from the pre-existing ControlPath encoder, choose a real state path long enough to
disable sharing and containing a backslash/quote: ordinary UserConfig connections already fall back to no sharing, but
the new Tensorlake host argv still includes the misencoded identity path and cannot use its registered key. Existing
shared ControlPath has a related older-client limitation; do not count that unchanged route as introduced here. The
builder used the path encoder before this commit; the assigned patch adds the host consumer. Suggested fix: encode paths
according to supported client parsing, or explicitly detect/refuse an incompatible client/path with a clear supported
remedy; changing the declared version floor is a product decision. Add static/isolated exact-version parser coverage
when tests are authorized. Bucket other for unusual-path portability/correctness failure, no established security or
user-work-loss route for this tokenizer defect. Confidence confirmed by exact supported-version primary source,
including downstream path load and fatal trailing-token handling; no runtime/cloud measurement was performed or needed
for the stated parser mismatch. Modern V_9_6_P1 uses argv_split and does consume these escapes; this is not a
contradiction of f7's narrow modern-parser line-break/comment-character refutation. No repair through pinned tip
8a8c9579e04299410652f0e4681975a7198dfc7b: the later ssh.rs change only negotiates the supervisor idle option.
proposed_drop false. possible_cover unknown; parent owns coverage. Separate f11 investigates percent expansion in old
known-host file handling and is not settled by this tokenizer finding.

Later qualification (review record g046/f10, event 27):

Reviewed commit 88bf83e267b5066815448885c61ed4148b851930. TLDR remains: automatic Tensorlake host connections fail on
supported OpenSSH 7.6 with a quote/backslash in the helm state path. This supplements the complete f10 finding above and
corrects upstream line references: web-rendered source line numbering differed from the directly retrieved raw files.
The raw V_7_6_P1 files are retained privately in this g046 directory as openssh-7.6-readconf.c, openssh-7.6-misc.c and
openssh-7.6-ssh.c; their URLs remain
https://raw.githubusercontent.com/openssh/openssh-portable/V_7_6_P1/<original-name>. Correct raw references:
readconf.c:1044-1056 parses IdentityFile through strdelim; 1091-1111 parses known-host path entries; 1686-1690 rejects
trailing garbage. misc.c:176-213 strips quotes without decoding backslash escapes and closes at the next quote.
ssh.c:1933-1951 performs tilde/percent expansion then loads the public identity, without decoding doubled backslashes.
The assigned encoder at ssh.rs:392,612-615 therefore selects a different path for a backslash and rejects a
quote-containing IdentityFile. The longer-than-socket-limit example isolates the new Tensorlake host identity option
from pre-existing ordinary ControlPath behavior: ordinary access can run unshared, but Tensorlake access still supplies
these misencoded key paths. The new argv-only tests at ssh.rs:1258-1262,1285-1343 assert encoded strings and do not
validate the declared oldest supported parser. Bucket other; confidence confirmed static exact-version compatibility
defect, no runtime measurement. SPEC_impl.md's 7.6 floor and automatic Tensorlake access contract govern; the
copied-command trust exception does not cover this route. Fix direction and temporal qualification from the earlier f10
event remain: support/version-gate the encoding, and do not attribute the already-existing builder/control-path
consumers as new. No later repair through pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b. proposed_drop false.
possible_cover unknown; parent consolidates. f11's absent known-host percent expansion is a distinct later-stage
mechanism, not a duplicate quote-tokenizer failure. f7 remains a narrow modern-tokenizer refutation, not a
universal-version statement.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g046/f10`; supporting records
`g043/g046:f10`, `g047/interface-g046-f10`. The retained private log preserves earlier wording and its corrections. This
record does not claim runtime reproduction.
