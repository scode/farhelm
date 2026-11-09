# A stopped old host can pass Update without a source-version check

Reviewed commit: e5142ab67c3a509dd4f4a89cb0aefb8b85b7f0fd

## TLDR

A stopped host running Farhelm below the minimum update version can still be updated without checking its installed
version. Whether the minimum-version rule is intended to cover stopped supervisors remains unresolved.

## Details

Suggested bucket: **other**. Confidence: **source admission confirmed; policy applicability unverified after
investigation**. No failed upgrade, data loss, credential exposure or lost process is established. A hypothetical
destructive migration is not evidence for a higher bucket.

Concrete source route: a registered, provisioned SSH Linux host has the real parseable v0.22.0 binary at its absolute
registered path, working SSH/manager/payloads, writable paths and no conflicting run or identity freeze. Its supervisor
socket does not answer. Published v0.22.0 `crates/farhelm/src/main.rs:1625-1649` exits 75 from `internal stdio` in that
case. `crates/farhelm-helm/src/provisioning/backend.rs:1564-1576` invokes that installed binary without asking its
version; `2609-2613` maps the empty status-75 result to `Absent`.

The new checks cover answering compatible/skewed peers, but planning's `Absent` arm at `provisioning/service.rs:1398`
proceeds to inspection/retention (`1400-1424`). Retained execution's `Absent` arm at `2338` keeps the plan and admits
execution at `2348`. Inspection checks platform and ownership, not installed Farhelm version (`backend.rs:2638-2730`).
Payload execution reaches `install_uploaded_source`, whose digest/mode check and atomic replacement have no version
acquisition (`backend.rs:1383-1436`, `2804-2833`). The UI permits Update for an unreachable supervisor. Failed SSH or
unrelated ownership/identity guards can stop an attempt; they are excluded by the stated trigger. The shared floor is
v0.23.0 (`crates/farhelm-proto/src/version.rs:13`).

The unresolved question is contractual. `SPEC.md`, "Upgrade compatibility and client scale", lines 4956-4959 broadly
requires refusing below-floor supervisors at planning and retained execution. `SPEC_impl.md`, "Provisioning", lines
7593-7601 describes checking the **probed supervisor version** on connected/skewed paths, without expressly requiring
installed-binary probing when the supervisor is stopped. A readable unparseable/development stamp is explicitly exempt;
an unobserved parseable installed release is a different case. The Mac installer's conservative on-disk record policy
does not establish a Linux probing requirement.

Investigation traced the old binary, probe classification, both admission boundaries, UI eligibility, concrete plan
coordinates and final replacement, and read the governing specifications. Source settles admission, but cannot settle
whether policy intentionally gates only responding supervisors. An authoritative decision or specification covering
stopped installed sources would settle that premise. If the broader refusal is intended, acquire/gate installed versions
on the absent-supervisor path while preserving genuine absence and explicit version exemptions.

Possible coverage: no exact existing queue, ledger, accepted rule, known bug or unlanded plan settles this case; Planned
was empty. Do not treat the unknown-value exemption as permission to omit observation. The route is a policy gap with
ordinary disconnected-host input, so no filter clearly covers it.

Provenance: run `ee465fa5-0b29-4ce7-b1fe-8fabdb679cfe`, `remote-floor/f1` and `floor-contract/f7`, corroborated by
consumer scope checks. Earlier overconfident policy wording was corrected in the append-only history. No runtime or
network experiment ran. The reviewed provisioning files were checked unchanged through pinned tip
`1cf6e79af4548fa9158ee0f12dff6348cccdbc99`.
