# Installer cleanup retains a mounted-descendant admission source gap

Reviewed commit: 0ae92a2a7957de08c23dc6db8a10af281175a5b7

## TLDR

Recursive installer cleanup could reach external mounted contents if such a tree passes all preceding moves. No
supported incidental Mac topology establishing that admission was found.

## Details

`crates/farhelm-supervisor/src/installation/dirs.rs:234–247` has no mount boundary. The movement and deletion paths are
at `installation/install.rs:603–650,960–983,1171–1174`. The missing premise is a supported mounted-descendant layout
surviving every preceding rename or swap and reaching unrelated backing files. Ordinary-directory tests do not establish
those platform semantics.

Suggested bucket: `other`; established deletion of external work could raise it. Confidence: incomplete platform/input
lead. Possible cover: recognized-bundle contents ownership in trailing-tip `SPEC.md:450–454` and deliberate setup at
`3320–3333` are partial or ambiguous for externally backed files. The public-app interruption exception does not settle
mounted contents. Provenance: `g010/f3`, `a003/f3`. No mount or deletion experiment ran.
