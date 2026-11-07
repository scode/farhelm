# Openssh floor percent path disables stable host pin

Reviewed commit: 88bf83e267b5066815448885c61ed4148b851930

## TLDR

On supported OpenSSH 7.6, a percent-bearing managed path can prevent the host key from being retained, so later
connections accept a new key again.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: highest.

Source location: `crates/farhelm-helm/src/ssh.rs:392,615,284-287,296,332 at 88bf83e267b5066815448885c61ed4148b851930`.

Source assessment (read with the later corrections below) (review record g046/f11, event 28):

Reviewed commit 88bf83e267b5066815448885c61ed4148b851930. TLDR: with a percent-containing helm state path on supported
OpenSSH 7.6, automatic Tensorlake SSH accepts new gateway keys repeatedly instead of retaining and enforcing its
first-use trust. Concrete configuration remains /state/50%/tensorlake with its real key/known_hosts, and no
/state/50%%/tensorlake sibling directory. The helper unconditionally doubles percent signs for IdentityFile and
UserKnownHostsFile; old OpenSSH expands the identity path but not the known-host path, so the valid registered key can
load while known-host trust reads/writes target a missing literal doubled-percent directory. This supplements the
complete f11 event and corrects the web-rendered upstream line numbers to raw files retained as
openssh-7.6-{ssh.c,readconf.c,sshconnect.c,hostfile.c} in this g046 directory. Raw source references: ssh.c:217-227 and
1423-1426 only tilde-expand known-host files; 1933-1951 percent-expands and loads identity files. readconf.c:1091-1111
stores the parsed known-host names without percent decoding. sshconnect.c:793-797 reads those filenames directly;
HOST_NEW:875-974 accepts accept-new, attempts the add, and only logs failed persistence at 968-970. Its final path at
1116-1157 confirms no further rejection for this missing-file case: when both host and IP are new, the changed-IP
condition is false, UpdateHostKeys is already disabled, and check_host_key returns success at 1157. hostfile.c:271-281
passes the filename unchanged to hostkeys_foreach:662-679, which fopen(path, r)s directly; add_host_to_hostfile:448-461
uses fopen(filename, a) without creating a parent or expanding tokens. Thus both an existing genuine pin in the real
state directory and a previously successful connection do not cause HOST_CHANGED when a different endpoint key arrives
on a later fresh process. Supported primary URLs are
https://raw.githubusercontent.com/openssh/openssh-portable/V_7_6_P1/ssh.c ,
https://raw.githubusercontent.com/openssh/openssh-portable/V_7_6_P1/readconf.c ,
https://raw.githubusercontent.com/openssh/openssh-portable/V_7_6_P1/sshconnect.c ,
https://raw.githubusercontent.com/openssh/openssh-portable/V_7_6_P1/hostfile.c . Direct raw retrieval resolved the web
source tool's unavailable hostfile.c and differing displayed line numbers; no dynamic/network experiment occurred.
Source artifacts SHA-256: ssh.c 9f497d3c03fd63b2674b5288520f28c5079aa1da462c8b036a8c297e6561f941; readconf.c
3889f9d565c1cbbd1afde1778b223102f27931c93e40491c2111195f58dd1a84; sshconnect.c
64b6d00631780ac61cfe670cb08e48938beb7835951f77298b815538138c9c91; hostfile.c
0b30b76874c9f2ce01a42016b61acf85af88ecbf366d89bcc9781c87d0743e7b. Suggested bucket highest for the established SSH
server-authentication bypass, without claiming private-key disclosure, successful fake-supervisor adoption, automatic
resume or observed attack. Confidence confirmed by exact supported-version source, not measured installed-client/cloud
runtime. No live gateway first-user-key-refusal premise is needed; a counterfeit endpoint can accept the client's
authentication, and the SSH layer itself no longer rejects its changed host key. Governing contracts: SPEC_impl.md's 7.6
floor and Tensorlake persisted known-host policy; SPEC.md Security's first-use gateway-key recording. Fix direction:
keyword/version-correct encoding or fail-closed refusal, with an exact-version fixture checking persisted trust when
runtime work is authorized. Distinct from f10: this failure needs only a percent character, not quote/backslash parsing;
changing strdelim escape handling alone leaves absent UserKnownHostsFile percent expansion. Builder encoder predates
this commit; the assigned host consumer is newly exposed here. No repair in the access option file or helper through
pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b (pinned diff checked). proposed_drop false. possible_cover unknown;
earlier encoder groups may overlap and parent owns consolidation. Caveats: a writable literal doubled-percent sibling
can store a different file, and a client that does expand UserKnownHostsFile percent tokens avoids this route. All
substantive leads resolved.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g046/f11`; supporting records
`g043/g046:f11`, `g047/interface-g046-f11`. The retained private log preserves earlier wording and its corrections. This
record does not claim runtime reproduction.
