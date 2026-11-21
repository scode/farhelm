# A legal inherited PATH can make managed supervisor startup fail

Reviewed commit: 23ae2b9850c539d6d90284dd48322a98af7d78de

## TLDR

On Linux, an inherited PATH the app can start with can still prevent its supervisor from starting, including after the
previous supervisor stops.

## Details

**high**. Confirmed by source and contract inspection.

Confirmed conditional source finding. Linux execve(2), “Limits on size of arguments and environment,” applies a
per-string limit to environment entries independent of total argument/environment size, with the bound named
MAX_ARG_STRLEN in linux/binfmts.h:10-16. A value that fits under the four-character original name can exceed that bound
under FARHELM_SESSION_START_PATH. The probe can still succeed when the shell shortens its tool PATH. Lib.rs:199-207
accepts that result and keeps the old bytes, and :1382-1393 adds them to the real supervisor command with no size guard.
During replacement the --version preflight inherits only the shortened app PATH (:1284-1291), then :639-670 stops the
old owned supervisor before spawning the command with the distinct environment. On first start, :632-638 propagates the
same spawn error. SPEC.md:4760-4764 and SPEC_impl.md:7493-7499 specify this transfer. SPEC_impl.md:7523-7534 places
foreseeable startup refusals before stopping a serving predecessor. Estimated high for inability to open the desktop and
lost supervisor availability under a valid but unusual input. There is no established loss of running terminal
processes, and the report does not infer behavior on macOS. Possible fix direction: carry the data through a bounded
handoff that does not consume a larger single environment entry, and validate it before stopping the predecessor.
Possible_cover: none identified. Later warning and relaunch changes keep the supervisor handoff and preflight ordering
through 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f. Source and OS contract only, no runtime reproduction.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `desktop-path/f18`.
