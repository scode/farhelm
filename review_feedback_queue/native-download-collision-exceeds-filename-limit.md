# Duplicate downloads fail for maximum-length filenames

Reviewed commit: 550e7e592a6489cfc475b71c678131de4d9f67e7

## TLDR

The desktop cannot download another copy of a file whose name already fills the destination filesystem’s filename limit.

## Details

Suggested bucket: `other`. Confidence: confirmed by code inspection for a destination filesystem with a 255-byte
filename limit; not runtime reproduced. Source: dedicated `gpt-6.1-sol` high adversarial review. Proposed drop: none.

crates/farhelm-helm/src/downloads.rs:139–146 appends the collision suffix without shortening the stem; :188–190 passes
those candidates to publication. On a filesystem allowing 255-byte components, a basename consisting of 251 ASCII
characters followed by '.pdf' is valid. If that name already exists in Downloads, the next candidate is 259 bytes.
StagedStream::publish_no_clobber (crates/farhelm-supervisor/src/files.rs:656–670) retries only AlreadyExists;
ENAMETOOLONG aborts the save. Every retry fails until the user renames or removes something. Browser downloads are
unaffected. Reserve space for the numbered suffix when constructing native-save candidates.

Possible cover: None found. SPEC_impl.md:1605–1609 specifies unique native-save names. FILTER.md’s rare self-correcting
failure filter does not cover a failure that repeats unchanged.

Current-main status: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703; downloads.rs is unchanged from the reviewed
commit. No fixing commit.

The full commit diff and relevant paths were read. No runtime reproduction was run.
