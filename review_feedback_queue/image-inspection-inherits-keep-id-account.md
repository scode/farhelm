# Image inspection inherits a synthetic keep-id account

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

An ambient Podman keep-id setting can make account inspection reject an image whose ordinary keeper account is valid.

## Details

Suggested bucket: **high**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The disposable inspector inherits keep-id account synthesis, while the real keeper passes --passwd=false. Its
passwd/home observations can therefore describe an injected account instead of the selected image account. Matching
Podman source confirms the mismatch; normal launch availability can fail under that configuration.
Intentional-environment policy coverage remains ambiguous. This is a source-confirmed conditional launch failure, not a
host-file ownership-loss claim or a runtime test result.

Source locations: `crates/farhelm-supervisor/src/container/account.rs:344-361,195-227`. Historical revision qualifiers
in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Discovery-pass finding, reviewed d6645f771bee8ed03a66486ef8755834937d759d; introduced
f73589c5201a0408f2f1555c18a4efca6f6057be. TLDR: setting the ordinary Podman keep-id default can make Farhelm
consistently refuse a container image that should receive a new host-matching account. Preconditions: host account
name/UID absent from the image, host home absent in the image, image WORKDIR empty or /, and ambient
PODMAN_USERNS=keep-id or containers.conf userns=keep-id. No installed image metadata is asserted. Farhelm
control_command132-145 preserves these defaults; choose_account344-361 supplies --user0:0 but neither --passwd=false nor
a namespace override. Podman v4.9.3 specgenutil/specgen.go224-238 consumes those defaults; generate/namespaces.go217-234
requests host-account passwd injection independently of the explicit user;
container_internal_common.go2443-2449,2465-2480,2517-2554 adds the absent host name/UID with HOME equal to the image
working directory, whose empty default is / (generate/container_create.go508-515). Farhelm's parser now sees an account
not in the original image. Rule1 at195-209 is bypassed, and fallback224-227 calls validate, which rejects the synthetic
/ home at85-86. This is a repeatable refusal, not a transient error fixed by retry. The final keeper uses --passwd=false
(container.rs348), so inspection and keeper do not have the same account facts. The existing no-mount/workdir/keep-id
unit test (container.rs876-901) records argv and supplies synthetic facts; absence of an explicit --userns flag does not
prove the runtime namespace default, and it never checks automatic passwd suppression. The seam-based real-Podman
account tests do not exercise ambient injection for an absent real host account. Suggested bucket high; confidence
confirmed conditional source behavior, no runtime occurrence/frequency claimed. No security or existing-user-work loss
established. Coverage searches: both specs, TODO Planned, unlanded queue, BUGS, all feedback files/index and
TRIAGE_OUTCOMES searched for Podman userns/keep-id/passwd injection; no matching accepted behavior, planned fix or
queued duplicate. SPEC.md3591-3613 specifically promises creation for absent host name/UID, and SPEC_impl.md5796-5810
requires original-image facts and describes the injection hazard. SPEC.md3659 preserves containers.conf. possible_cover:
broad deliberate-environment intent at3676-3689, but no clear acceptance that a normal namespace default may contaminate
the account-selection facts and defeat the explicit matching-account contract. Proposed drop none. A fix should suppress
account injection during inspection, rather than relying on no explicit keep-id flag. The non-/ WORKDIR variant
selecting a keeper-absent synthetic user remains unnecessary to the verified refusal and is not separately claimed.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g125/f4`, `a035/f2`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
