# Inline sign-in tests conflate the question and attempt identifiers

Reviewed commit: fc361c4ba6a00ebe69be51fc81fe07ae034a1959

## TLDR

The Add Host tests can pass if Send or Cancel uses the identifier for the whole sign-in instead of the individual
question.

## Details

`crates/gui/farhelm-gui/src/hosts/add/tests.rs:347`, `:407` and `:485–517` give the attempt and its question the same
fixture value. Use distinct values and assert the question action addresses the displayed question while completion uses
the attempt identifier. The source confirms that the test cannot distinguish them. The shipped form binds its own
question ID.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `maintenance-ui/f2`, `maintenance-contract/f3`,
`sign-in-contract/f5`.
