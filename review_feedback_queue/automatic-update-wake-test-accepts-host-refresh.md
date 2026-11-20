# The update wake test can pass on an unrelated refresh

Reviewed commit: aa0c139dd77691606c57740702210491840f63f4

## TLDR

The test can miss that turning on automatic updates failed to wake the helm, because a delayed first host refresh can
wake it instead.

## Details

The fixture at `crates/farhelm-helm/src/provisioning.rs:1859–1905` waits for Connected and one scan while updates are
off. The host can publish Connected before its first collections read finishes and send a fleet wake when the read
settles. Establish that the initial refresh is complete before testing the preference-only wake, or observe that wake
directly. The source confirms the alternative producer. Its scheduling was not exercised.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `automatic-updates/l7`.
