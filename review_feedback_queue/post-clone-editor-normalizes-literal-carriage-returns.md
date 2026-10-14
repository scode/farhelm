# Editing the post-clone command silently rewrites literal carriage returns

Reviewed commit: f6da791d88336193fba346079fe03de5c84a5ae9

## TLDR

Editing and saving an existing command can change its executable contents beyond the user's edit, affecting future
checkouts.

## Details

Suggested bucket: `highest`. Confidence: confirmed by static tracing, pinned dependency source, and the HTML contract;
no runtime reproduction. Source: dedicated `gpt-6.1-sol` high adversarial review. Proposed drop: none.

Classification caveat: highest — conservative classification because saved executable user configuration loses literal
data; no credential exposure or destructive execution was established.

At crates/farhelm-ui/src/checkout_post_clone.rs:49–58, the textarea's input handler replaces the entire draft with
event.value(). HTML textarea values normalize CR and CRLF to LF; both pinned Dioxus 0.7.10 event implementations read
that normalized value. The original command is loaded verbatim at lines 30–35, but any edit replaces it with normalized
text, which lines 83–96 save. Trigger: a command previously saved through the CLI/API contains a literal carriage
return, including inside shell quotes. For example, a stored command represented as "printf 'one\rtwo' > setup.txt"
changes to "printf 'one\ntwo' > setup.txt" after an unrelated edit and Save, changing generated file contents. The
backend accepts CR at checkout_config.rs:569–579 and stores the command verbatim at 298–305. The supervisor executes
those bytes through shell -c at launch.rs:1456–1459. This contradicts SPEC.md:680–682 and SPEC_impl.md:491–495. Opening
alone preserves storage; the problem requires editing and saving. Preserve such characters losslessly or visibly refuse
editing text the control cannot preserve. Browser normalization is specified by the
[HTML Standard](https://html.spec.whatwg.org/multipage/form-elements.html#dom-textarea-value).

Possible cover: None found. TRIAGE_OUTCOMES.md headings
template-command-editors-hide-meaningful-characters-silently-saved-command-editor.md and
template-command-editor-displays-untrusted-command-bytes.md concern template commands, not this configuration field. The
rare-glitch filter excludes durable changes.

Current-main status: Persists at pinned main 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703. checkout_post_clone.rs is
identical to the reviewed version; configuration validation is unchanged. No fixing commit found.

The full commit diff and relevant failure paths were read. No runtime reproduction was run.

## Additional affected scope

Reviewed commit: f6da791d88336193fba346079fe03de5c84a5ae9

## TLDR

Editing a saved post-clone command can change its shell meaning, including turning commented-out commands into
executable commands.

## Details

Suggested bucket: `highest`. Confidence: confirmed by code inspection and the textarea specification; no browser or
destructive-command reproduction performed. Source: dedicated `gpt-6-astra` high adversarial review. Proposed drop:
none.

crates/farhelm-ui/src/checkout_post_clone.rs:49–58 loads executable text into a textarea and replaces the entire draft
with event.value() on any edit. A textarea's API value normalizes CR and CRLF to LF, per the
[HTML standard](https://html.spec.whatwg.org/multipage/form-elements.html#the-textarea-element). The CLI/API accept
literal CR: crates/farhelm-helm/src/checkout_config.rs:569–579 refuses only NUL and excessive length. Thus an unrelated
edit followed by Save changes previously stored CR bytes. In bash, text after a comment marker and literal CR remains
commented until LF; normalization can make that text a separate executable line. Quoted arguments and filenames can also
change. The command subsequently reaches shell -c verbatim at crates/farhelm-supervisor/src/launch.rs:1433–1459. Opening
Settings alone does not persist the change; editing and saving triggers it. Preserve these bytes through an explicit
representation or refuse unsafe editing visibly before browser normalization loses them.

Possible cover: SPEC.md:678–689 explicitly promises preservation of other spacing and newlines. TRIAGE_OUTCOMES.md
headings template-command-editors-hide-meaningful-characters-silently-saved-command-editor.md and its resume-command
counterpart cover template commands, not this post-clone setting. The original queue's
template-editor-conceals-peer-model-and-folder.md covers different fields and provenance. No matching coverage found.

Status at frozen endpoint: Persists at pinned main 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703. checkout_post_clone.rs is
unchanged; production validation in checkout_config.rs is unchanged. No fixing commit.

Review coverage: full commit diff read: true. No commit-diff hunks remain unread. The 13,089-line triage ledger was
searched rather than read end-to-end; unrelated queue finding bodies and unrelated specification sections were not read.
Supporting dependency code and checkout retry machinery were traced selectively, not exhaustively. Aggregate tool-output
truncation affected an initial broad TODO read and a sessions excerpt; the complete Planned bucket and relevant sessions
excerpt were reopened before use. New working-copy queue additions and other reviewers' reports were not read.

Runtime evidence: No runtime tests or reproductions run, as requested. Applied the supplied test-authoring checklist to
all added tests. Findings are static-review conclusions with the stated caveats. No checkout or scratch files created,
modified or deleted; no fetch, rebase or range extension.
