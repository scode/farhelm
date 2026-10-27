# Terminal-file backup crash persistence remains unestablished

Reviewed commit: 0ae92a2a7957de08c23dc6db8a10af281175a5b7

## TLDR

Whether power loss can preserve a replacement Terminal link while losing the preceding foreign-file backup remains
unresolved. The source's live-file preservation order alone establishes neither loss nor crash safety.

## Details

`crates/farhelm-supervisor/src/installation/install.rs:1077` renames the backup, `1099` publishes the link, and
`1116–1118` flushes their directory. Missing is supported filesystem/journal behavior allowing publication to survive
without the earlier same-directory backup. The interruption hook runs after that flush (`1119`);
`installation/install/tests.rs:218–290` observes a live filesystem and does not establish a power-loss ordering.

Suggested bucket: `other` pending that source premise; established foreign-work loss could raise it. Confidence:
incomplete durability lead, not reproduced loss. Possible cover: trailing-tip `SPEC.md:443–448` requires preservation;
no exact crash-order acceptance was found. Provenance: `g011/f6`, `g015/f3`, `a004/f2`. This is separate from the
confirmed live public-name overwrite race. No power-loss or filesystem experiment ran.
