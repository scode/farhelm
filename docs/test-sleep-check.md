# Checking delays in tests

`scripts/check-test-sleeps.py` finds delay calls in Rust and browser test source and requires a reason for each one. Use
the inventory to inspect intent before changing a test: readiness polling belongs in a named helper, while an
observation window or deliberate timing stimulus may need to remain. An annotation records a reason; it does not prove
that reason is sound. Apply [.agents/test-authoring.md](../.agents/test-authoring.md) when reviewing the result.

Install the small pinned parser packages in an isolated environment outside the checkout, then invoke that interpreter:

```sh
python3 -m venv /path/to/test-sleep-env
/path/to/test-sleep-env/bin/python -m pip install -r scripts/test-sleep-requirements.txt
/path/to/test-sleep-env/bin/python scripts/check-test-sleeps.py
```

The command neither builds nor executes test code. `--inventory` prints JSON for all recognized calls, including their
annotations. Both modes exit 1 while any recognized delay lacks a rationale, 0 when none do, and 2 when source discovery
or parsing is incomplete. Cancellation retains its signal status. `--root` selects another checkout with the same source
layout. The existing sleeps are being migrated; this command is not yet wired into CI.

Place `// sleep-ok: <why>` on its own line immediately before the call or after the completed call on its ending line.
Explain the observable window, scheduling stimulus, deadline or helper responsibility that requires the delay. Empty
annotations, strings containing annotation text, block comments and doc comments do not count. A rationale on a shared
source line can cover multiple delay calls on that line; the reviewer must check that it explains each one.

The checker reads Rust under `crates/` and TypeScript under `e2e/tests/`. Integration-test directories and the
`farhelm-teststate` fixture crate are test infrastructure throughout. Other Rust files are checked in test-attributed or
test-only `cfg` declarations, including their external module files. An alternative production condition such as
`cfg(any(test, target_os = "macos"))` does not establish test-only scope. Missing or ambiguous test-module sources fail
the check. Conditional module paths (`cfg_attr` containing `path`) fail explicitly because the checker does not evaluate
attribute-dependent source selection. Generated dependency/build directories (`node_modules`, `target`, `.git`, `.jj`)
are excluded; other symlinks fail discovery rather than silently hiding a source subtree.

Recognition uses [Tree-sitter syntax trees](https://github.com/tree-sitter/py-tree-sitter) and the
[Rust module path rules](https://doc.rust-lang.org/reference/items/modules.html). It recognizes Rust `sleep` and
`sleep_until`, including qualified calls and macro token trees, and browser `setTimeout`, `setInterval` and
`waitForTimeout`. Playwright `test.setTimeout` and `testInfo.setTimeout` configure deadlines and are excluded. Explicit
import renames are recognized regardless of declaration order. Transparent callee parentheses and TypeScript type
assertions preserve recognition. Strings, comments and regex text remain inert; expressions inside template literals
remain code. The checker does not expand macros or perform type resolution, arbitrary function-value alias analysis, or
interprocedural timing analysis. A same-named custom function is still a candidate; moving a delay behind a newly named
wrapper requires review of that wrapper's responsibility.

Source reads are bounded to 2 MiB per file, 64 MiB total, 10,000 files and 100,000 directory entries. Reports are
bounded to 10,000 calls and an 8 MiB cumulative budget for annotation/report fields; formatted JSON is capped at 16 MiB.
Annotations are indexed once by source row instead of searched afresh for every call. The 60-second deadline is checked
during syntax traversal and result construction as well as between filesystem operations; it cannot interrupt a
kernel-blocked filesystem call. Parser errors are failures, not an empty inventory. The pinned Rust grammar is tested
against ordinary `raw` variable references as well as test syntax; changing parser versions requires rechecking the
source inventory.

Focused checker validation uses the same environment:

```sh
/path/to/test-sleep-env/bin/python -B scripts/test-test-sleep-syntax.py
/path/to/test-sleep-env/bin/python -B scripts/test-check-test-sleeps.py
```
