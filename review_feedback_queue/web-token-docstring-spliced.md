# web_token docstring opens with a spliced fragment claiming it inserts

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

None at runtime; misleads maintainers of the token bootstrap path.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0328-2b597e9-9c17` (audit of Area 2, helm HTTP/WS authentication boundary) as
`F14 / COR-WEBTOKEN-DOC`, tagged **definite**. Anchors and title: `store.rs:2876-2887` — `HelmStore::web_token`'s
docstring opens with a spliced fragment claiming it inserts a token

This is a documentation-only finding, with no runtime effect.

The doc comment on `HelmStore::web_token` begins with a paragraph that belongs to another function: "Return the
recoverable web token, inserting `candidate` if this helm has never minted one before. … A caller must never assume its
candidate won merely because the row". That paragraph stops mid-sentence. The function's real description follows
directly: "Read the recoverable web token without creating one." The leading fragment describes `web_token_or_insert`,
the next function in the file, which does insert.

The first line of the docstring therefore says the opposite of the function's contract. That contract matters:
`token
show`'s read-only path (`show_existing_token`) depends on `web_token` never creating a token. A maintainer who
reads only the first line will get it wrong.

Suggested fix: delete the orphaned leading paragraph. If the complete sentence was meant to be on `web_token_or_insert`,
restore it there. That function's current docstring covers the same ground without the broken sentence.
