// The scratch helper's placement contract, pinned. Every converted spec
// accepts whatever path stackScratchDir() returns, so nothing else in the
// suite would fail if the helper quietly reverted to os.tmpdir() — and
// placement inside the stack's state dir is the entire orphan-cleanup
// guarantee (see helpers/scratch.ts). The prefix half matters too:
// filters.spec.ts derives a search needle from its scratch dir's
// basename, so a helper that discarded the requested prefix would
// degrade that test without failing it.

import { expect, test } from "./helpers/evidence";
import fs from "node:fs";
import path from "node:path";
import { stackScratchDir } from "./helpers/scratch";

test("scratch dirs land in the stack state dir and keep their prefix", () => {
  const raw = fs.readFileSync(path.resolve(__dirname, "../.stack-info.json"), "utf8");
  const state = (JSON.parse(raw) as { state: string }).state;
  const first = stackScratchDir("fh-contract-");
  const second = stackScratchDir("fh-contract-");
  try {
    for (const dir of [first, second]) {
      expect(path.dirname(dir)).toBe(state);
      expect(path.basename(dir).startsWith("fh-contract-")).toBe(true);
      expect(fs.statSync(dir).isDirectory()).toBe(true);
    }
    expect(first).not.toBe(second);
  } finally {
    for (const dir of [first, second]) {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  }
});
