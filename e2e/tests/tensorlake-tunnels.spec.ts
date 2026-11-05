/** Tunnel health is a host-view contract; orphan cleanup drives the shipped
 * routes against the stack's marked Tensorlake fake. No vendor resources or
 * credentials are used by this spec. */
import { expect, test } from "./helpers/evidence";
import { readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";
import { currentStackSlot } from "../stack-slots";
import { openHostsPanel } from "./helpers/fleet";
import { openSettings } from "./helpers/settings";

/** A host listing fixture isolates rendering from real gateway connectivity.
 * The runtime's readiness, retry and child ownership are covered in Rust. */
test("tunnel failures make a connected host amber and show each mapping", async ({ page }) => {
  let health: any = { phase: "up" };
  await page.route("**/api/hosts", async (route) => {
    const response = await route.fetch();
    const body = await response.json();
    body.hosts.push({
      ...body.hosts[0],
      id: 9001,
      kind: "tensorlake",
      name: "Tunnel host",
      alias: "Tunnel host",
      destination:
        "farhelm-host-k7q2-test-abcd@farhelm-host-k7q2-test-abcd.sandbox.tensorlake.ai",
      identity: null,
      incarnation: 1,
      state: {
        phase: "connected",
        identity: null,
        build_version: "0.1.0",
        old_version: false,
        newer_version: false,
        refresh: { status: "ok", sessions: 0 },
      },
      tensorlake: {
        sandbox: "farhelm-host-k7q2-test-abcd",
        recipe: "inference",
        size: { cpus: 2, memory_gb: 4, disk_gb: 20 },
        tunnels: {
          tunnels: [{ sandbox_port: 8080, host: "127.0.0.1", port: 9090 }],
          state: health,
        },
      },
    });
    await route.fulfill({ response, json: body });
  });
  await page.goto("/");
  await openHostsPanel(page);
  let host = page.locator('.host-row[data-host-id="9001"]');
  await expect(host.locator(".host-tunnel-details")).toHaveAttribute(
    "data-tunnel-state",
    "up",
  );
  await expect(
    host.getByText("127.0.0.1:8080 → 127.0.0.1:9090", { exact: true }),
  ).toBeVisible();
  await expect(host.locator(".host-status")).toHaveClass(/connected/);
  health = {
    phase: "retrying",
    reason: "remote port forwarding failed for listen port 8080",
    delay_secs: 4,
  };
  await page.reload();
  await openHostsPanel(page);
  host = page.locator('.host-row[data-host-id="9001"]');
  await expect(host.locator(".host-status")).toHaveClass(/unreachable/);
  await expect(host.locator(".host-tunnel-details")).toContainText(
    "retrying in 4s",
  );
  await expect(host.locator(".host-tunnel-details")).toContainText(
    "remote port forwarding failed",
  );
  await expect(page.locator('[data-rail-host-id="9001"] .host-rail-state'))
    .toHaveClass(/unreachable/);
});

/** Only product prefixes qualify. Confirmation must precede termination,
 * and cleanup removes snapshots by exact source while harness resources and
 * the current helm's saved recipe builder remain untouched. Failed cleanup
 * remains retryable after Settings closes, including during the request. */
test("Settings lists unknown Farhelm sandboxes and confirms snapshot cleanup", async ({ page, request }) => {
  const info = JSON.parse(
    await readFile(currentStackSlot().stackInfoPath, "utf8"),
  );
  const root = info.tensorlake_fake;
  const putConfig = async (config: unknown) => {
    const next = path.join(root, `config.${process.pid}.next`);
    await writeFile(next, JSON.stringify(config));
    await rename(next, path.join(root, "config.json"));
  };
  await putConfig({});
  // A worker reuses its stack between projects. The previous spec may leave
  // {}, which is the Rust fake's valid empty model (serde defaults). Match
  // that contract at this fixture boundary, preserving unrelated records.
  // Only a missing file is empty; malformed JSON and read failures must fail.
  const readModel = async () => {
    const raw = await readFile(path.join(root, "state.json"), "utf8").catch(
      (error) => {
        if (error.code === "ENOENT") return "{}";
        throw error;
      },
    );
    const model = JSON.parse(raw);
    return {
      ...model,
      sandboxes: model.sandboxes ?? [],
      snapshots: model.snapshots ?? [],
      counter: model.counter ?? 0,
    };
  };
  const putModel = async (model: unknown) => {
    const next = path.join(root, `state.${process.pid}.next`);
    await writeFile(next, JSON.stringify(model));
    await rename(next, path.join(root, "state.json"));
  };
  const settings = await request.put("/api/tensorlake", {
    data: { api_key: "test-key" },
  });
  expect(settings.ok(), await settings.text()).toBe(true);
  const tag = (await settings.json()).helm_tag;
  const otherTag = tag === "zzzz" ? "yyyy" : "zzzz";
  const recipe = "e2e-orphan-owner";
  expect(
    (await request.post("/api/tensorlake/recipes", {
      data: {
        name: recipe,
        recipe: {
          size: { cpus: 1, memory_gb: 1, disk_gb: 10 },
          source: { kind: "image", image: "tensorlake/ubuntu-systemd" },
          claude: null,
          codex: null,
          tunnels: [],
        },
      },
    })).ok(),
  ).toBe(true);
  const names = [
    `farhelm-host-${tag}-orphan-abcd`,
    `farhelm-host-${otherTag}-orphan-abcd`,
    "farhelm-sbx-test-1",
    `farhelm-build-${tag}-${recipe}`,
  ];
  const ids = names.map((_, n) => `e2eorphansbx${n}`);
  let model: any;
  try {
    model = await readModel();
    expect(
      model.sandboxes.some((s: any) => ids.includes(s.id)),
      "fixture owns fresh sandbox IDs",
    ).toBe(false);
    model.sandboxes.push(
      ...names.map((name, n) => ({
        id: ids[n],
        name,
        status: "running",
        image: "tensorlake/ubuntu-systemd",
        snapshot_id: null,
        resources: { cpus: 1, memory_mb: 1024, disk_mb: 10240 },
        timeout_secs: 900,
        auto_resume: false,
      })),
    );
    model.snapshots.push(
      ...ids.map((id, n) => ({
        id: `e2eorphansnap${n}`,
        sandbox_id: id,
        status: "completed",
        polls_left: 0,
      })),
    );
    await putModel(model);
    const listing = await request.get("/api/tensorlake/orphans");
    expect(listing.ok(), await listing.text()).toBe(true);
    expect(
      (await listing.json()).filter((s: any) => ids.includes(s.id)).map((
        s: any,
      ) => s.name).sort(),
    ).toEqual(names.slice(0, 2).sort());
    await page.goto("/");
    let dialog = await openSettings(page, "tensorlake");
    let list = dialog.getByRole("region", {
      name: "Sandboxes not on this helm",
      exact: true,
    });
    await expect(list).toBeVisible();
    let own = list.locator(`[data-sandbox-id="${ids[0]}"]`);
    let other = list.locator(`[data-sandbox-id="${ids[1]}"]`);
    await expect(own).not.toContainText("from another helm");
    await expect(other).toContainText("from another helm");
    await own.getByRole("button", { name: "Delete", exact: true }).click();
    await expect(list.locator(".orphan-delete-confirm")).toContainText(
      "files, sessions and snapshots will be deleted",
    );
    expect(
      (await readModel()).sandboxes.find((s: any) => s.id === ids[0]).status,
    ).toBe("running");
    await putConfig({
      fail: {
        delete_snapshot: { status: 503, message: "snapshot cleanup blocked" },
      },
    });
    await list.getByRole("button", { name: "Delete sandbox", exact: true })
      .click();
    await expect(list.getByRole("alert")).toContainText(
      "snapshot cleanup blocked",
    );
    expect(
      (await readModel()).sandboxes.find((s: any) => s.id === ids[0]).status,
    ).toBe("terminated");
    expect(
      (await readModel()).snapshots.some((s: any) => s.sandbox_id === ids[0]),
    ).toBe(true);
    await list.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(own).toBeVisible();
    await expect(list.getByRole("alert")).toContainText(
      "snapshot cleanup blocked",
    );
    await expect(list.locator(".orphan-delete-confirm")).toBeVisible();
    // Cancel and unmount Settings: recovery must come from the helm's
    // operation state, rather than this confirmation's captured sandbox ID.
    await list.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(
      own.getByRole("button", { name: "Retry cleanup", exact: true }),
    ).toBeVisible();
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    dialog = await openSettings(page, "tensorlake");
    list = dialog.getByRole("region", {
      name: "Sandboxes not on this helm",
      exact: true,
    });
    own = list.locator(`[data-sandbox-id="${ids[0]}"]`);
    other = list.locator(`[data-sandbox-id="${ids[1]}"]`);
    await expect(own.getByRole("alert")).toContainText(
      "snapshot cleanup blocked",
    );
    await expect(
      own.getByRole("button", { name: "Retry cleanup", exact: true }),
    ).toBeVisible();
    expect(
      (await readModel()).snapshots.some((s: any) => s.sandbox_id === ids[0]),
    ).toBe(true);
    // Also lose the requesting component before the API failure exists.
    // The helm's Running record is the named in-flight readiness boundary.
    await putConfig({ hold_operations: ["delete_snapshot"] });
    await own.getByRole("button", { name: "Retry cleanup", exact: true })
      .click();
    await list.getByRole("button", { name: "Delete sandbox", exact: true })
      .click();
    await expect.poll(async () =>
      (await (await request.get("/api/tensorlake/orphans")).json()).find((
        s: any,
      ) => s.id === ids[0])?.cleanup?.phase
    ).toBe("running");
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    await putConfig({
      fail: {
        delete_snapshot: {
          status: 503,
          message: "snapshot cleanup blocked after close",
        },
      },
    });
    await expect.poll(async () =>
      (await (await request.get("/api/tensorlake/orphans")).json()).find((
        s: any,
      ) => s.id === ids[0])?.cleanup?.phase
    ).toBe("failed");
    dialog = await openSettings(page, "tensorlake");
    list = dialog.getByRole("region", {
      name: "Sandboxes not on this helm",
      exact: true,
    });
    own = list.locator(`[data-sandbox-id="${ids[0]}"]`);
    other = list.locator(`[data-sandbox-id="${ids[1]}"]`);
    await expect(own.getByRole("alert")).toContainText(
      "snapshot cleanup blocked after close",
    );
    await putConfig({});
    await own.getByRole("button", { name: "Retry cleanup", exact: true })
      .click();
    await list.getByRole("button", { name: "Delete sandbox", exact: true })
      .click();
    await expect(list.locator(".orphan-delete-confirm")).toHaveCount(0);
    await expect(own).toHaveCount(0);
    model = await readModel();
    expect(model.sandboxes.find((s: any) => s.id === ids[0]).status).toBe(
      "terminated",
    );
    expect(model.snapshots.some((s: any) => s.sandbox_id === ids[0])).toBe(
      false,
    );
    expect(
      model.snapshots.filter((s: any) => ids.slice(1).includes(s.sandbox_id)),
    ).toHaveLength(3);
    await other.getByRole("button", { name: "Delete", exact: true }).click();
    await list.getByRole("button", { name: "Delete sandbox", exact: true })
      .click();
    // An empty discovery still needs visible proof that cleanup finished;
    // removing the last row must not remove the section's Refresh/status.
    await expect(list).toBeVisible();
    await expect(list.locator(".tensorlake-orphan")).toHaveCount(0);
    await expect(list.locator(".tensorlake-orphans-status")).toHaveText(
      /^None found · checked \d{2}:\d{2}$/,
    );
  } finally {
    await putConfig({});
    model = await readModel();
    model.sandboxes = model.sandboxes.filter((s: any) => !ids.includes(s.id));
    model.snapshots = model.snapshots.filter((s: any) =>
      !ids.includes(s.sandbox_id)
    );
    await putModel(model);
    await request.delete(`/api/tensorlake/recipes/${recipe}`);
    await request.put("/api/tensorlake", { data: { clear_api_key: true } });
  }
});
