# The test recorder's and deflake's folder checks claim a protection their sticky-folder exception removes

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

The test-run recorder and the deflake sweep keep their files under a fixed shared folder in `/tmp`. Their documentation
says they refuse that folder when another account created it first. They do not: they accept it whenever it is marked
sticky, and the account that owns a sticky folder can still move anything inside it. Development tooling is accepted as
running on a machine with no hostile other accounts, so this is not a security problem to fix. It is documentation that
promises something the code does not do.

## Details

Found by the 2026-10-22 prioritized persistent review, groups `220-maint-scripts-8of9` (finding f1),
`221-maint-scripts-9of9` (finding f1) and `235-test-scripts-1of17` (finding f1), merged here.

Scope: the reviewers reported that another local account could use this to overwrite the maintainer's files, read test
output, forge test results, or choose what the deflake sweep executes. An independent check found that security
consequence covered by SPEC_impl.md "Development tooling assumes a single-user machine" (`SPEC_impl.md:4923-4927`): "The
test harnesses, the capture and recording scripts, deflake and the docs preview run on a development machine with no
hostile other local accounts, and need not defend against one". This item covers only the remainder: the code's own
documentation contradicts what the code does.

What the documentation says:

- `scripts/record-test-run.py:296-300` (`safe_parent`'s docstring) says a sticky directory is safe because "only an
  entry's owner may rename or remove it", and names the threat: "another account could create `/tmp/farhelm-tmp` first
  and then swap this user's evidence root for a directory of its own."
- `docs/test-run-evidence.md:120-121`: the directory holding the root "must belong to that user or to root, or be sticky
  like `/tmp`; otherwise another account that created the fixed-name directory first could swap or read the evidence."
- `deflake/bin/deflake:96-102` (`prepare_state_root`'s docstring): "a tree another account created first (or a
  `farhelm-tmp` it owns) would let that account choose what the sweep executes."

What the code does:

- `scripts/record-test-run.py:304` accepts the parent when `st_uid in (euid, 0)` or the sticky bit is set, whoever owns
  it.
- `deflake/bin/deflake:110-111` does the same.
- In a sticky directory, the directory's owner may still rename or remove any entry (the rename(2) and unlink(2)
  permission rules allow the file's owner, the directory's owner, or a privileged process). `/tmp` is safe only because
  root owns it. So an account that runs `mkdir -m 1777 /tmp/farhelm-tmp` before the maintainer does passes both checks,
  which is exactly the case the documentation says is refused.

How to verify: read the lines above. Or, as a second account, run `mkdir -m 1777 /tmp/farhelm-tmp`, then run the
recorder or `deflake start` as the maintainer and see the root accepted.

Fix sketch, either of:

- Correct the docstrings and `docs/test-run-evidence.md` so they no longer claim protection against another account
  owning `/tmp/farhelm-tmp`, and cite the single-user assumption in SPEC_impl.md.
- Make the code match the text: require the parent to be owned by this user or by root (a root-owned sticky directory
  like `/tmp` still passes), in both `safe_parent` and `prepare_state_root`.

Suggested bucket: other, per the independent check. It is not highest: under the accepted single-user assumption, the
wrong statement cannot lead a reader into a security mistake. The reviewers had suggested highest for the security
consequence, which is now covered.

Confidence: confirmed by code reading. The behavior of sticky directories is standard POSIX and Linux behavior; nothing
was executed.

Possible cover: SPEC_impl.md "Development tooling assumes a single-user machine" covers the security consequence, not
the documentation mismatch. `TRIAGE_OUTCOMES.md` "install-lock-owner-unchecked.md" settled a similar case for the
installer's install directory, a different directory, which the user chose.
