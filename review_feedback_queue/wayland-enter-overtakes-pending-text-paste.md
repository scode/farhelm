# Enter can reach a shell before a preceding paste

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

On Wayland, Enter can run the command already on the line while Farhelm is still fetching pasted text meant to finish
it. The paste can then arrive at the next prompt.

## Details

Bucket: **highest**. GUI migration final whole-tree review, R2 Sol paste order (H).

Source references: `crates/gui/farhelm-gui/src/terminal/attached.rs:1377–1394, 1618–1656`,
`vendor/gpui-pre-linux/src/linux/wayland/clipboard.rs:292–337`.

The platform clipboard reads from an external producer on a worker. The terminal defers the paste behind that read and
earlier pastes, while Enter goes straight into the attachment's send queue. Its FIFO preserves enqueue order, which is
already wrong. A command can run before the option that was supposed to make it harmless.

Sol rated it source-confirmed and critical. Opus verification H confirmed the mechanism and rated it noncritical because
the producer must answer slowly enough for Enter to overtake it and the command must depend on the pasted text. The
asynchronous clipboard change regresses the old browser's synchronous clipboardData route. Highest reflects the
established input-order route to executing a different command.

Queue following key input behind a bounded pending paste for the same attachment. Verify order with a controlled delayed
clipboard response. SPEC.md lines 3114–3119 accepts slow system clipboard behavior. Whether that covers Farhelm
reordering its own queued terminal input is unclear.
