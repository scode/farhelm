# Legacy inode approval retains an aliasing source gap

Reviewed commit: 0ae92a2a7957de08c23dc6db8a10af281175a5b7

## TLDR

A different directory with the same inode number could inherit legacy-app replacement approval. A supported incidental
Mac history reaching harmful replacement remains unestablished.

## Details

`scripts/install.sh:1090–1095` forwards only the inode; `crates/farhelm-supervisor/src/installation/install.rs:340`
compares that scalar. The later full-identity check at `861–893` identifies the subsequently inspected object. Tests at
`installation/install/tests.rs:516–542` retain the original folder and require the newcomer to have a different inode,
so they do not settle reuse or aliasing. A cross-volume collision alone does not establish that the following rename
succeeds.

Suggested bucket: `other`. Confidence: confirmed scalar comparison with missing supported platform/input and
harmful-route premises. Possible cover: trailing-tip `SPEC_impl.md:4675–4683` specifies inode approval and a separate
interrupted-rebuild exception; neither explicitly accepts this preinspection aliasing consequence. Provenance:
`g010/f2`, `g014/f3`, `a003/f2`. No work loss or filesystem behavior was reproduced.
