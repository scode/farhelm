# Open Settings retains old shared Claude trust values

Reviewed commit: 0ad806334217ba8dde4e7f7dc6ecc7480d27cb9a

## TLDR

An already-open Settings dialog can keep showing the wrong automatic-trust choice or hint preference after another
client changes it.

## Details

Suggested bucket: `other`. Confidence: confirmed from resource dependencies; browser reproduction was not run. No
unauthorized trust edit or resulting work loss was established. Source: separate adversarial trailing-review agent,
Claude trust scope. Proposed drop: none.

`crates/farhelm-ui/src/claude_trust.rs:47–50` creates the Settings resource without a reactive input or invalidation
subscription. Its controls at `:62–65,116–119` keep the initial fetched values until a local interaction restarts that
resource. In contrast, the terminal band refreshes through accepted session updates at `:137–143`. A shared change can
therefore reach the band while an already-open dialog retains its old controls indefinitely.

The feature contract at `SPEC.md:831–836` says other clients observe saved changes through normal invalidation; the
implementation contract at `SPEC_impl.md:1860–1865` publishes those hints. Subscribe Settings to the applicable refresh
boundary while preserving pending local intent. A browser regression should leave Settings open while another client
saves each shared value, then check the existing controls without reopening the dialog.

Possible cover: `SPEC.md`'s "One GUI at a time" accepts races between several GUIs on a best-effort basis. The reviewer
found no exact acceptance for this dialog never refreshing a completed shared change; the feature's specific
invalidation promise points the other way. Retain that specification question for triage rather than assuming the
general qualifier settles it. No exact Planned item, BUGS entry, queue item, or ledger decision was found.

The complete introducing patch and relevant component context were read. No changed scope went unread; no browser test
was run.
