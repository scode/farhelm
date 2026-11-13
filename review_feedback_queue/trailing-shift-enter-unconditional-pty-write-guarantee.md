# Qualify the grid Shift+Enter single-write guarantee

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: The new implementation explanation promises an unconditional single PTY write, although the terminal-host contract explicitly allows a short-write suffix to be written later. Future maintainers receive incompatible guarantees.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g010/f1`.

### g010/f1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 4 (finding, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e. Synthesis revision: retain the documentation-only finding; distinguish a single encoder message, an offer to libc::write, successful physical writes, and program read boundaries. TLDR: The new implementation explanation promises an unconditional single PTY write, although the terminal-host contract explicitly allows a short-write suffix to be written later. Future maintainers receive incompatible guarantees. Details: SPEC_impl.md:1722-1727 newly says the encoder sends ESC CR as one input, which the terminal host writes to the pty in one write, and states "The single write is the promise". One input is established by grid_input.rs:298-313; that does not establish one successful physical write. terminal-host/src/host.rs:573-580 explicitly describes offering the whole input initially, possible macOS short writes and no room-first wait. Its implementation at 591-617 writes segment.data[segment.written..], advances written by the returned count and retains a short-write suffix for a later call. The exact lower contract at crates/terminal-host/SPEC.md:399-403 is: "An `Input` of at most `ATOMIC_INPUT` bytes is offered to the pty whole, in one `write` call, so the host itself never
splits a key that encodes to several bytes (Shift+Enter's ESC CR). The pty may still take only part of it when its input
queue is nearly full (macOS's pty takes what fits and returns a short write), and the rest then follows in later writes.
How the program's reads divide the bytes is up to the program and its terminal settings; the host promises nothing about
it." SPEC.md:2869-2876 already contains the stronger product-level single-write/read-boundary motivation; the newly added root implementation wording repeats that stronger guarantee without reconciling it with the actual terminal-host contract. It is therefore an incorrect unconditional implementation explanation even though it is not a newly introduced end-to-end product requirement. Qualify the new text as offered whole in the initial write call, preserve the short-write and program-read caveats, and reconcile the existing product statement rather than treating a one-message encoder as proof of delivery atomicity. Original suspected consequence preserved: ESC and CR might reach a boundary-sensitive line editor separately, changing a newline chord into Escape-plus-submit. The lower-level contract explicitly permits the relevant PTY/read behavior; no runtime split, unintended submit, destructive command or loss-of-work sequence was observed or established. This record does not request a terminal-host code change and does not lower any previously established security/data-loss route (none was established). Suggested bucket: other, documentation contract accuracy. Could-be-highest: no established route; a destructive submit would require a concrete shipped input sequence and effect not shown by these source checks. Confidence: confirmed documentation contradiction and partial-write source path; user-visible split outcome is not verified. proposed_drop: false. possible_cover: none for the inaccurate new explanation; the terminal-host Input contract accepts the actual implementation. No runtime execution or reproduction performed.

Returned proposal (claims-004, event 4): **retain-verified**, suggested bucket **other**.

The new explanation overstates what a single encoded input establishes: the host initially offers all bytes, but a short write can require a later suffix write. The lower contract explicitly permits this.

Original stronger consequence: ESC and CR might reach a boundary-sensitive line editor separately, changing a newline chord into Escape-plus-submit. No runtime split, unintended submit, destructive command or loss-of-work sequence was observed or established. The ordinary initial offer and single-message encoder behavior are correct.

Unresolved premise or limitation: A concrete supported terminal/line-editor input sequence and effect showing a destructive submit or loss of work is missing. Short-write possibility alone does not establish that stronger consequence.

Proposed basis: Retain the other-priority documentation mismatch and reconcile the root wording with the lower contract. Because the history narrows a potential unintended-submit route to documentation, have the parent independently check that consequence narrowing; this consultation does not accept a drop of a security/work-loss route.

Existing parent disposition (event 4): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: retain-verified-other. Full004newf560SPEC_implunconditionalonePTYwriteexplanationcontradictsactualhostshortwritesuffixloopandterminal-hostSPECinitialwholeoffer/shortwriteallowed/no-read-boundaryguarantee. RootSPECstrongerwordingalreadybase, butnewimplementationclaiminrange. Retainpersistentdocumentationcontractmismatch, ordinaryencoderoneframe/initialofferisnotphysicalwriteatomicity. Proposedotherbecauseactualsupportedlineeditorsequence/split→unintendedsubmit→destructiveworklossnotestablished; originalhypothesishighestremainsinregister forfreshpriorityjudgment, ambiguousloweringkeepshighest. No runtime/falsephysicalreadclaim. SeparateRustblockedreaderandAltGraphrootsnotduplicates.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g010 record](records/review-through-00de0b6c/groups/g010.json). Record ID: `f1`.

