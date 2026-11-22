# Landing removed the ambiguous-create refusal-marker assertion

Reviewed commit: 4fc89689b56d288bbd414a0b503ea576df883aa1

## TLDR

The regression still checks that the session and its retry key are retained, but it stopped checking whether the client
is wrongly told to discard that key.

## Details

**other.** Source inspection. Main reference `crates/farhelm/tests/e2e/host_creates.rs:1220`.

The exact parent of the reviewed commit checked the error chain at host_creates.rs:1218-1224. The Harness imports
farhelm_helm::SupervisorClient (harness.rs:25,1038-1043). Its create_session_with_key delegates to
create_session_with_extras, which sends CreateSession through request and preserves its errors
(crates/farhelm-helm/src/client.rs:3160-3183,3191-3232). request adds SettledCreateUnaccepted context whenever the reply
bit is true (:3023-3038). crates/farhelm-helm/src/lib.rs:1876-1877 gives it the exact deleted text; :1714-1719 also
makes that typed context significant at the HTTP boundary. A valid database reservation does not guarantee correct reply
metadata. SPEC_impl.md:3026-3041 explicitly excludes retained launches from that proof. Assert absent typed proof at the
client boundary, or assert create_unaccepted is false on the actual reply. Bucket: other for a demonstrated
test-coverage regression. No incorrect product reply is established. Confidence: confirmed by source. Possible coverage:
ordinary_create_refusal_proof_requires_the_matching_failed_reservation covers the predicate with a retained row, and a
wire test checks the false bit for a mismatched key (core.rs:28084-28110,28183-28217), neither exercises this
ambiguous-host branch.

Additional source record at `4fc89689b56d288bbd414a0b503ea576df883aa1`, `crates/farhelm/tests/e2e/host_creates.rs:1220`.

Reviewed commit: 4fc89689b56d288bbd414a0b503ea576df883aa1. Withdrawal reason: the landing commit's explanation does not
match the call path. The e2e Harness uses farhelm_helm::SupervisorClient (harness.rs:25,1038-1043), whose request
handler adds the exact removed SettledCreateUnaccepted context when a CreateSession reply carries the flag
(crates/farhelm-helm/src/client.rs:3023-3038 and lib.rs:1876-1877). See l4 for the retained finding and its contract.
The assertion was deleted, but its ability to catch this reply regression is source-supported.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `lifecycle_e2e/l4`, `lifecycle_e2e/c1`.
