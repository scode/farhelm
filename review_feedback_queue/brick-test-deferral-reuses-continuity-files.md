# Remote update deferral can publish stale receipt names

Reviewed commit: be6cb86e1e0922b661272c17ebb108a73752ddf8

## TLDR

The release test can turn a failed or unavailable remote update into a harness failure when it carries forward receipt
filenames from a different checkpoint.

## Details

The new prerequisite at `releasing/mac-vm-test/guest_operations.py:827–830` can defer before recording a new continuity
sample. The deferral still saves attempt metadata at `:337–345` using the continuity pair in pass-wide state. Receipt
validation resolves names in the current attempt’s directory at `:439–501`, so the older pair can change the intended
failure verdict. Keep exported receipt names in attempt-local state and cover a deferral after a real earlier pair. The
bookkeeping path follows from source. Runtime incidence is unmeasured.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `release-brick-test/l3`.
