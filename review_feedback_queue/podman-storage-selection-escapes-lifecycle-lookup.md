# Podman storage selection can escape lifecycle lookup

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A container created under a login shell's Podman configuration can survive while Stop reports absence from a different
store.

## Details

Suggested bucket: **high**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The creator inherits login-shell-selected CONTAINERS_STORAGE_CONF, while supervisor lifecycle lookup/cleanup can use a
different namespace. Both stores must contain the requested image for creation to proceed. The resulting false
absence/Stop success is source-established conditionally; scoped cleanup can separately terminate helper processes and
narrows live-process claims, but does not make the storage lookup correct. Deliberate-environment acceptance does not
clearly settle false lifecycle success. This is high for material control failure; a distinct security or established
user-work-loss route was not shown. No alternate store was configured or tested.

Source locations:
`crates/farhelm-supervisor/src/container.rs:62-90,132-145,174-194,405-483; crates/farhelm-supervisor/src/service/core.rs:13767-13799,15328-15329`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Discovery pass 1 policy correction, same f2 at d6645f771bee8ed03a66486ef8755834937d759d. Preserve the established
persistent-login-export producer and split across supervisor preflight/control versus login-shell creator/agent exec,
plus tab refusal and Stop false completion; parent reports a005 independently confirms that source route. Correct
possible_cover from none to SPEC.md 3676-3689: deliberate shell environment is user intent when the consequence follows
from what that setup means. This can plausibly cover selecting another Podman store and the resulting admission/refusal
differences. Coverage is not plainly complete for a Stop reported successful against a different store: the same
paragraph expressly preserves protections stated for specific cases (3687-3689), and container lifecycle specifically
promises actual container-state confirmation and retained ownership on unconfirmed cleanup (3640-3644; SPEC_impl.md
5789-5794). Whether the false completion is an accepted consequence of the deliberate environment or violates that
specific promise is a policy ambiguity, not a source uncertainty. Retain it for parent synthesis; do not silently
exclude it and do not claim no possible cover. Suggested high for the established route, confidence confirmed
source/documentation semantics, not runtime reproduced; proposed_drop none pending policy judgment; possible_cover
SPEC.md 3676-3689 against 3640-3644. No security/user-work-loss or highest consequence route established. Existing
detailed f2 events retain the phase map and qualified Delete consequence. This turn now proceeds to discovery beyond
f1-f3.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g115/f2`, `g113/f4`, `g116/f6`, `a005/f6`,
`a032/g113/f4`, `a035/f3`, `a036/g113-f4`. These are original reviewer identities for this concern, not additional bugs.
Full revisions, withdrawals and independent verdicts remain in the private review evidence. No fix or triage decision
was executed.
