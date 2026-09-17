/**
 * Exercise active-first work ordering through real terminal output and two clients.
 *
 * Each owned shell reads a private mode file. Changing that file starts or
 * finishes output without focusing a browser terminal, so a rename editor can
 * remain genuinely focused while the supervisor observes the next status
 * transition. No listing or notification is fabricated: both clients consume
 * the real helm.
 *
 * The decisive shape is a MIXED fleet: B holds the newest burst key while A
 * is the one still running, so only the active-first grouping puts A above
 * B. Every phase asserts per-session keys alongside the order, because a
 * group move must never advance a key and an unchanged key array must never
 * be confused with an unchanged order.
 */
import { expect, newObservedContext, test } from "./helpers/evidence";
import type { APIRequestContext, Page } from "@playwright/test";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { cleanupSession, createSession, localHostId, openRowMenu, resetPreferences } from "./helpers/fleet";

/** Keep generated paths literal when the fixture command passes through a shell. */
function shellWord(value: string): string {
  return `'${value.replaceAll("'", "'\\''")}'`;
}

/** Read exactly the owned rows, preserving the helm's authoritative order. */
async function ownedRows(request: APIRequestContext, ids: string[]) {
  const response = await request.get("/api/sessions?sort=activity");
  expect(response.ok(), `listing failed: ${response.status()}`).toBe(true);
  const body = await response.json();
  return body.sessions.filter((row: { id: string }) => ids.includes(row.id)) as {
    id: string;
    title: string;
    status: { state: string };
    last_work_started_at: number;
    stale: boolean;
  }[];
}

/** Ignore unrelated fixture rows without letting a client invent its own ordering. */
async function renderedOrder(page: Page, ids: string[]): Promise<string[]> {
  return page.locator(".session-row").evaluateAll((rows, owned) =>
    rows.map((row) => row.getAttribute("data-session-id")!).filter((id) => owned.includes(id)), ids);
}

/**
 * Per-session burst keys, keyed by id rather than listed in order: a group
 * move reorders the rows while leaving every key alone, so comparing
 * positional pairs would mistake a reorder for a key change.
 */
function keysById(rows: { id: string; last_work_started_at: number }[]): Map<string, number> {
  return new Map(rows.map((row) => [row.id, row.last_work_started_at]));
}

/**
 * A real shell emits numbered output only in run mode. The progress file is
 * written after printf, providing a positive producer witness for the period
 * during which continuing output must leave the already allocated key alone.
 */
async function startProducer(request: APIRequestContext, directory: string, name: string, host: number) {
  const mode = join(directory, `${name}.mode`);
  const progress = join(directory, `${name}.progress`);
  await writeFile(mode, "idle");
  await writeFile(progress, "0");
  // sleep-ok: this private producer's polling interval drives controlled continuing output; API status and progress are the readiness oracles.
  const program = `n=0; printf 'producer ready\\n'; while :; do mode=$(cat ${shellWord(mode)}); if [ "$mode" = run ]; then n=$((n + 1)); printf 'work %s\\n' "$n"; printf '%s' "$n" > ${shellWord(progress)}; fi; sleep 0.1; done`;
  const session = await createSession(request, {
    title: `work-start-${name}`,
    cwd: directory,
    invocation: `sh -c ${shellWord(program)}`,
    host,
  });
  return { id: session.id, mode, progress };
}

/**
 * Observe active-first ordering across bursts, idle demotion, reload and rename.
 *
 * A starts first and B second, so B holds the newer burst key; both running
 * reads B,A. Stopping B while A continues must demote B below A on unchanged
 * keys — in the API and in both clients — while B's open rename editor keeps
 * its exact DOM node, draft, selection and focus. A genuine new burst for B
 * promotes it again, and idling both leaves work-start order within the one
 * inactive group.
 */
test("active sessions lead across bursts, idle demotion, reload and rename", async ({
  page, browser, request, timeline,
}) => {
  test.setTimeout(240_000);
  const directory = await mkdtemp(join(tmpdir(), "farhelm-work-order-"));
  const ids: string[] = [];
  const otherContext = await newObservedContext(browser, timeline);
  const other = await otherContext.newPage();
  try {
    await resetPreferences(request);
    const host = await localHostId(request);
    const a = await startProducer(request, directory, "a", host);
    ids.push(a.id);
    const b = await startProducer(request, directory, "b", host);
    ids.push(b.id);
    await expect.poll(async () => (await ownedRows(request, ids)).map((row) => row.status.state), {
      timeout: 30_000,
      message: "both producers must reach observed idle before their first burst",
    }).toEqual(["idle", "idle"]);
    const seed = new Map((await ownedRows(request, ids)).map((row) => [row.id, row.last_work_started_at]));
    await Promise.all([page.goto("/"), other.goto("/")]);
    for (const client of [page, other]) {
      await expect(client.locator(".sort-select")).toHaveValue("activity");
    }

    await writeFile(a.mode, "run");
    await expect.poll(async () => (await ownedRows(request, ids)).find((row) => row.id === a.id)?.last_work_started_at, {
      timeout: 20_000,
    }).toBeGreaterThan(seed.get(a.id)!);
    const first = (await ownedRows(request, ids)).find((row) => row.id === a.id)!;
    expect(first.status.state).toBe("running");

    await writeFile(b.mode, "run");
    await expect.poll(async () => (await ownedRows(request, ids)).find((row) => row.id === b.id)?.last_work_started_at, {
      timeout: 20_000,
    }).toBeGreaterThan(first.last_work_started_at);
    const overlapping = await ownedRows(request, ids);
    expect(overlapping.map((row) => row.status.state)).toEqual(["running", "running"]);
    expect(overlapping.map((row) => row.id)).toEqual([b.id, a.id]);
    const keys = keysById(overlapping);
    for (const client of [page, other]) {
      await expect.poll(() => renderedOrder(client, ids)).toEqual([b.id, a.id]);
    }

    const beforeA = Number(await readFile(a.progress, "utf8"));
    const beforeB = Number(await readFile(b.progress, "utf8"));
    await expect.poll(async () => Number(await readFile(a.progress, "utf8")), {
      timeout: 10_000,
      message: "the older producer must actually emit more output during the overlapping burst",
    }).toBeGreaterThan(beforeA + 30);
    await expect.poll(async () => Number(await readFile(b.progress, "utf8")), {
      timeout: 10_000,
      message: "the newer producer must actually emit more output during the overlapping burst",
    }).toBeGreaterThan(beforeB + 30);
    expect(keysById(await ownedRows(request, ids))).toEqual(keys);
    for (const client of [page, other]) {
      await expect.poll(() => renderedOrder(client, ids)).toEqual([b.id, a.id]);
    }

    // B's rename editor opens while B still leads; stopping B then demotes
    // the row UNDER the editor, which is the preservation case.
    const source = page.locator(`[data-session-id="${b.id}"]`);
    await openRowMenu(source);
    await source.locator(".session-row-rename").click();
    const field = page.locator(".rename-dialog .rename-input");
    await field.fill("draft-name");
    await expect(field).toBeFocused();
    await field.evaluate((node) => {
      (window as any).__workOrderRename = node;
      (node as HTMLTextAreaElement).setSelectionRange(0, 5);
    });

    await writeFile(b.mode, "idle");
    await expect.poll(async () => {
      const rows = await ownedRows(request, ids);
      return [rows.find((row) => row.id === a.id)?.status.state, rows.find((row) => row.id === b.id)?.status.state];
    }, {
      timeout: 30_000,
      message: "B must reach observed idle while A is still observed running",
    }).toEqual(["running", "idle"]);
    // A genuinely still working, not merely unsampled: its producer must
    // advance past the mixed-state observation.
    const mixedProgress = Number(await readFile(a.progress, "utf8"));
    await expect.poll(async () => Number(await readFile(a.progress, "utf8")), {
      timeout: 10_000,
      message: "A's producer must keep emitting while B sits idle",
    }).toBeGreaterThan(mixedProgress);
    const mixed = await ownedRows(request, ids);
    expect(mixed.map((row) => row.id)).toEqual([a.id, b.id]);
    expect(keysById(mixed)).toEqual(keys);
    expect(mixed.every((row) => row.stale === false), "both rows are connected reports").toBe(true);
    for (const client of [page, other]) {
      await expect.poll(() => renderedOrder(client, ids)).toEqual([a.id, b.id]);
    }

    // The demoted row's editor survives the reorder intact: same node, same
    // draft, same selection, still focused — and still functional.
    expect(await field.evaluate((node) => node === (window as any).__workOrderRename)).toBe(true);
    await expect(field).toBeFocused();
    await expect(field).toHaveValue("draft-name");
    expect(await field.evaluate((node) => [
      (node as HTMLTextAreaElement).selectionStart, (node as HTMLTextAreaElement).selectionEnd,
    ])).toEqual([0, 5]);
    await page.keyboard.insertText("saved");
    await expect(field).toHaveValue("saved-name");
    await field.press("Enter");
    await expect(page.locator(".rename-dialog")).toHaveCount(0);
    await expect(source.locator(".session-row-menu-panel")).toHaveCount(0);
    await expect.poll(async () => (await ownedRows(request, ids)).find((row) => row.id === b.id)?.title).toBe("saved-name");

    // A reload in the mixed state reconstructs the same grouped order from
    // the server alone — no client keeps a private rank.
    await other.reload();
    await expect.poll(() => renderedOrder(other, ids)).toEqual([a.id, b.id]);
    await expect.poll(() => renderedOrder(page, ids)).toEqual([a.id, b.id]);
    expect(keysById(await ownedRows(request, ids))).toEqual(keys);

    // A genuine new burst for B promotes it within the shared active group.
    await writeFile(b.mode, "run");
    await expect.poll(async () => (await ownedRows(request, ids)).find((row) => row.id === b.id)?.last_work_started_at, {
      timeout: 20_000,
    }).toBeGreaterThan(keys.get(b.id)!);
    await expect.poll(async () => (await ownedRows(request, ids)).map((row) => row.status.state), {
      timeout: 30_000,
    }).toEqual(["running", "running"]);
    const promoted = await ownedRows(request, ids);
    expect(promoted.map((row) => row.id)).toEqual([b.id, a.id]);
    for (const client of [page, other]) {
      await expect.poll(() => renderedOrder(client, ids)).toEqual([b.id, a.id]);
    }

    const finalKeys = keysById(await ownedRows(request, ids));
    await Promise.all([writeFile(a.mode, "idle"), writeFile(b.mode, "idle")]);
    await expect.poll(async () => (await ownedRows(request, ids)).map((row) => row.status.state), {
      timeout: 30_000,
    }).toEqual(["idle", "idle"]);
    expect(keysById(await ownedRows(request, ids))).toEqual(finalKeys);
    for (const client of [page, other]) {
      await expect.poll(() => renderedOrder(client, ids)).toEqual([b.id, a.id]);
    }
  } finally {
    await otherContext.close();
    for (const id of ids) await cleanupSession(request, id);
    await rm(directory, { recursive: true, force: true });
  }
});
