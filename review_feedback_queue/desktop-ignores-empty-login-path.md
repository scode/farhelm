# Farhelm ignores an explicitly empty login PATH

Reviewed commit: 23ae2b9850c539d6d90284dd48322a98af7d78de

## TLDR

An explicitly empty login PATH triggers fallback and makes tools available that the shell excluded.

## Details

**other**. Confirmed by source and contract inspection.

Confirmed by source. Login_path.rs:18 passes the shell value to the writer, and :38-53 rejects a present empty value
after removing the delimiter. The test at :152-166 explicitly expects fallback for exported empty PATH. The session
environment merge independently preserves an empty starting PATH at host_launch.rs:1233-1246. SPEC.md:4760-4764 calls
for the PATH set by the shell, with fallback on lookup failure. SPEC_impl.md:7486-7499 provides no empty-value
exception. Distinguish a present empty value from an unset variable in the result framing if only the latter should keep
fallback. Synthesis with path-notices/f1 and /f3 at 1d4fd8a723e5a195f61e625ecf62e8c3a347eb29: NoPath returns None and
yields a failed lookup observation, so the later generic warning correctly describes the fallback, although the ignored
empty value caused it. A notice save failure is the separate path-notices/f4 condition. This report concerns actual tool
selection, separate from the fixed notice copy the spec accepts. Estimated bucket: other. Possible_cover: none for empty
value handling. Login_path.rs remains unchanged through 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f. No runtime
observation. Discovery updates the test citation to include the assertion. Consequence and disposition are unchanged.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `desktop-path/f3`.
