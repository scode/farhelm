/**
 * Exercise stable work ordering through real terminal output and two clients.
 *
 * Each owned shell reads a private mode file. Changing that file starts or
 * finishes output without focusing a browser terminal, so a rename editor can
 * remain genuinely focused while the supervisor observes the next work burst.
 * No listing or notification is fabricated: both clients consume the real helm.
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
  }[];
}

/** Ignore unrelated fixture rows without letting a client invent its own ordering. */
async function renderedOrder(page: Page, ids: string[]): Promise<string[]> {
  return page.locator(".session-row").evaluateAll((rows, owned) =>
    rows.map((row) => row.getAttribute("data-session-id")!).filter((id) => owned.includes(id)), ids);
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
 * Observe both overlapping work and a later genuine restart of output. The
 * latter must move a row while its exact rename textarea, draft and selection
 * remain intact; finishing output must not move either client backwards.
 */
test("real work bursts stay ordered across output, completion, reload and rename", async ({
  page, browser, request, timeline,
}) => {
  test.setTimeout(150_000);
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
    const keys = overlapping.map((row) => [row.id, row.last_work_started_at]);
    for (const client of [page, other]) {
      await expect.poll(() => renderedOrder(client, ids)).toEqual([b.id, a.id]);
    }

    const before = Number(await readFile(a.progress, "utf8"));
    await expect.poll(async () => Number(await readFile(a.progress, "utf8")), {
      timeout: 10_000,
      message: "the older producer must actually emit more output during the overlapping burst",
    }).toBeGreaterThan(before + 30);
    expect((await ownedRows(request, ids)).map((row) => [row.id, row.last_work_started_at])).toEqual(keys);
    await writeFile(a.mode, "idle");
    await expect.poll(async () => (await ownedRows(request, ids)).find((row) => row.id === a.id)?.status.state, {
      timeout: 30_000,
    }).toBe("idle");
    expect((await ownedRows(request, ids)).map((row) => [row.id, row.last_work_started_at])).toEqual(keys);

    const source = page.locator(`[data-session-id="${a.id}"]`);
    await openRowMenu(source);
    await source.locator(".session-row-rename").click();
    const field = page.locator(".rename-dialog .rename-input");
    await field.fill("draft-name");
    await expect(field).toBeFocused();
    await field.evaluate((node) => {
      (window as any).__workOrderRename = node;
      (node as HTMLTextAreaElement).setSelectionRange(0, 5);
    });
    await writeFile(a.mode, "run");
    await expect.poll(async () => (await ownedRows(request, ids)).find((row) => row.id === a.id)?.last_work_started_at, {
      timeout: 20_000,
    }).toBeGreaterThan(overlapping[0].last_work_started_at);
    for (const client of [page, other]) {
      await expect.poll(() => renderedOrder(client, ids)).toEqual([a.id, b.id]);
    }
    expect(await field.evaluate((node) => node === (window as any).__workOrderRename)).toBe(true);
    await expect(field).toBeFocused();
    expect(await field.evaluate((node) => [
      (node as HTMLTextAreaElement).selectionStart, (node as HTMLTextAreaElement).selectionEnd,
    ])).toEqual([0, 5]);
    await page.keyboard.insertText("saved");
    await expect(field).toHaveValue("saved-name");
    await field.press("Enter");
    await expect(page.locator(".rename-dialog")).toHaveCount(0);
    await expect(source.locator(".session-row-menu-panel")).toHaveCount(0);
    await expect.poll(async () => (await ownedRows(request, ids)).find((row) => row.id === a.id)?.title).toBe("saved-name");

    const finalKeys = (await ownedRows(request, ids)).map((row) => [row.id, row.last_work_started_at]);
    await Promise.all([writeFile(a.mode, "idle"), writeFile(b.mode, "idle")]);
    await expect.poll(async () => (await ownedRows(request, ids)).map((row) => row.status.state), {
      timeout: 30_000,
    }).toEqual(["idle", "idle"]);
    expect((await ownedRows(request, ids)).map((row) => [row.id, row.last_work_started_at])).toEqual(finalKeys);
    await other.reload();
    await expect.poll(() => renderedOrder(other, ids)).toEqual([a.id, b.id]);
    await expect.poll(() => renderedOrder(page, ids)).toEqual([a.id, b.id]);
  } finally {
    await otherContext.close();
    for (const id of ids) await cleanupSession(request, id);
    await rm(directory, { recursive: true, force: true });
  }
});
