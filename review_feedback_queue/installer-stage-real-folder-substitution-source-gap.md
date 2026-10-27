# Private staging cleanup retains a real-folder substitution source gap

Reviewed commit: 0ae92a2a7957de08c23dc6db8a10af281175a5b7

## TLDR

Cleanup removes the current private staging directory without rechecking its acquired identity. An ordinary supported
replacement placing unrelated work there remains unestablished.

## Details

`crates/farhelm-supervisor/src/installation/install.rs:937–983,1158–1174` later removes the current staging name.
`installation/dirs.rs:30–43,69–79,234–247` refutes stable symlink traversal, but does not authenticate a replacement
real directory. The missing source/input premise is an ordinary producer of that replacement; no actual unrelated-work
deletion is established.

Suggested bucket: `other`; a proven outside-work deletion route could raise it. Confidence: incomplete source lead.
Possible cover: trailing-tip `SPEC.md:3320–3333` covers deliberate imitations. `SPEC_impl.md:4679–4683` accepts the
separate public-app move-aside interruption route, which supplies neither arbitrary private-folder substitution nor
blanket staging ownership. Provenance: `g009/f2`, independently retained remainder `a003/f1`. No reproduction ran.
