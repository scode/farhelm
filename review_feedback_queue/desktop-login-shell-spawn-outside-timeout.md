# A slow shell launch can bypass the desktop's five-second fallback

Reviewed commit: 23ae2b9850c539d6d90284dd48322a98af7d78de

## TLDR

If the system stalls while starting the login shell, the desktop can miss its five-second fallback and stay closed.

## Details

**high**. Confirmed by source and contract inspection.

Confirmed application control flow: login_path.rs:32-36 computes the deadline and calls synchronous spawn before
checking it, :59-69 installs a setsid pre-exec callback, and lib.rs:199-225 cannot proceed to desktop bootstrap until
lookup returns. Local Rust 1.98.1 Unix std source, std/sys/process/unix/unix.rs:123-150 and :467-475, shows a pre-exec
callback selecting fork/exec and a synchronous wait on the close-on-exec error channel. The project pins 1.99.1, so this
is neighboring standard-library source evidence, and stalled shell loading on a deployed platform is a conditional
consequence. No OS stall was observed. Independently, login_path.rs:73-89 checks success before the deadline on each
iteration, so when the app wakes past the deadline it accepts any successful exit already available without knowing if
it happened on time. SPEC.md:4760-4764 promises fallback after five seconds. SPEC_impl.md:7486-7491 explicitly puts
passwd resolution before the timer, separately handled in f7. Synthesis with path-notices/f1, /f3 and /f10: until the
lookup returns the desktop has no new result to report. A late accepted result follows the success path. This precedes
notice saving and is separate from path-notices/f4. Estimated bucket: high for possible failure to open the app.
Possible_cover: none for the shell timer gap. Login_path.rs remains unchanged through
8bcca67df1afe6eb0f184ef73b9952ed7ebb329f. Source and contract only.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `desktop-path/f2`.
