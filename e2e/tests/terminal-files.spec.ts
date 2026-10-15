// Real two-host reads, xterm hover admission and browser saves. The request
// gate deliberately holds only the stat reply; bytes and routing remain real.
import { expect, test } from './helpers/evidence';
import { type Page } from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import { createSession, cleanupSession, listSessions } from './helpers/fleet';
import { attachSession, waitForTermText } from './helpers/term';
import { stackScratchDir } from './helpers/scratch';
import { routeGate } from './helpers/route-gate';

/** Locate live terminal cells after the test's gated invocation emits bytes.
 * The fixtures use ASCII labels; Unicode cell mapping has its own JS proof.
 * A blank in-grid cell clears xterm’s retained coordinate before re-entry.
 */
async function hoverPath(page: Page, needle: string) {
  let cell: { x: number; y: number } | null = null;
  await expect.poll(async () => {
    cell = await page.evaluate((needle) => {
      const term = (window as any).__farhelmTerm;
      const box = term.element.querySelector('.xterm-screen').getBoundingClientRect();
      for (let i = 0; i < term.rows; i++) {
        const text = term.buffer.active.getLine(term.buffer.active.viewportY + i)?.translateToString(true) || '';
        const column = text.indexOf(needle);
        if (column >= 0) return { x: box.x + (column + needle.length / 2) * box.width / term.cols, y: box.y + (i + 0.5) * box.height / term.rows };
      }
      return null;
    }, needle);
    return cell !== null;
  }, { timeout: 10000, message: `live output must contain ${needle}` }).toBe(true);
  const box = (await page.locator('#terminal .xterm-screen').boundingBox())!;
  // xterm retains the last buffer cell across mouseleave. Visit a known
  // blank in-grid cell before departing, so re-entry actually starts hover.
  await page.mouse.move(box.x + box.width - 2, cell!.y);
  await page.mouse.move(box.x + box.width - 2, box.y - 2);
  await expect(page.locator('#terminal .xterm-screen')).not.toHaveClass(/xterm-cursor-pointer/);
  await page.mouse.move(cell!.x, cell!.y);
  return cell!;
}

/** Host readiness is mandatory here: an unreachable SSH fixture cannot pass
 * as coverage of a download from the second supervisor.
 */
async function connectedHost(request: any, kind: string): Promise<number> {
  let found: any;
  await expect.poll(async () => {
    const response = await request.get('/api/hosts');
    expect(response.ok()).toBe(true);
    const { hosts } = await response.json();
    found = hosts.find((host: any) => host.kind === kind && host.state?.phase === 'connected');
    return !!found;
  }, { timeout: 20000, message: `${kind} fixture must be connected` }).toBe(true);
  return found.id;
}

/** Observe the actual rendered path row, rather than treating a running shell
 * or elapsed delay as proof that xterm redrew beneath the resting pointer.
 * Tooltip mutations retain only counters, including transient hide/restart
 * changes that would already be gone by the next Playwright assertion.
 */
async function observePathRedraws(page: Page, needle: string) {
  await page.evaluate((needle) => {
    const term = (window as any).__farhelmTerm;
    const row = Array.from({ length: term.rows }, (_, index) => index).find(index =>
      term.buffer.active.getLine(term.buffer.active.viewportY + index)?.translateToString(true).includes(needle));
    if (row === undefined) throw new Error('the live path row disappeared before observation');
    const tooltip = term.element.querySelector('.terminal-file-target');
    if (!tooltip || tooltip.hidden) throw new Error('hover must show checking before redraw observation');
    const state = { renders: 0, ticks: 0, lastTick: -1, hides: 0, restarts: 0 };
    const render = term.onRender(({ start, end }: { start: number; end: number }) => {
      if (start > row || end < row) return;
      const text = term.buffer.active.getLine(term.buffer.active.viewportY + row)?.translateToString(true) || '';
      const tick = /TICK (\d+)/.exec(text);
      if (!text.includes(needle) || !tick) return;
      state.renders++;
      if (Number(tick[1]) !== state.lastTick) {
        state.lastTick = Number(tick[1]);
        state.ticks++;
      }
    });
    const mutations = new MutationObserver(records => {
      for (const record of records) {
        if (record.type === 'attributes' && record.attributeName === 'hidden' && record.oldValue === null) state.hides++;
        if (record.type === 'childList' && [...record.addedNodes].some(node => node.textContent === 'checking…')) state.restarts++;
      }
    });
    mutations.observe(tooltip, { attributes: true, attributeOldValue: true, childList: true });
    (window as any).__pathRedraw = { state, dispose: () => { render.dispose(); mutations.disconnect(); } };
  }, needle);
}

for (const linkKind of ['plain', 'OSC']) {
  for (const targetKind of ['folder', 'file']) {
    /** A resting hover is one lookup through real redraws, before and after
     * its answer. Departure and return must still withdraw and refresh it.
     * Both link providers and both stat outcomes exercise the same contract.
     */
    test(`resting ${linkKind} ${targetKind} hover survives live redraws`, async ({ page, request }, testInfo) => {
      const directory = stackScratchDir('file-redraw-');
      const basename = targetKind === 'folder' ? 'redraw.dir' : 'redraw.txt';
      const filename = path.join(directory, basename);
      if (targetKind === 'folder') fs.mkdirSync(filename);
      else fs.writeFileSync(filename, 'redraw file bytes');
      expect(fs.statSync(filename).isDirectory()).toBe(targetKind === 'folder');
      const label = linkKind === 'plain' ? basename : 'OSC-REDRAW';
      const printed = linkKind === 'plain' ? label : `\\033]8;;file://${filename}\\007${label}\\033]8;;\\007`;
      // Echo is disabled so releasing a gate does not move the path to another
      // row. Only the trailing tick changes; the path and its cell range stay put.
      const session = await createSession(request, {
        title: `file-redraw-${linkKind}-${targetKind}-${Date.now()}`, cwd: directory,
        invocation: `sh -c 'stty -echo; read _gate; printf "TARGET ${printed} TICK 0000"; read _redraw; i=1; while [ "$i" -le 1000 ]; do printf "\\rTARGET ${printed} TICK %04d" "$i"; i=$((i+1)); sleep 0.08; done; sleep 300'`,
      });
      const gate = routeGate();
      let statRequests = 0;
      await page.route(`**/api/sessions/${session.id}/files/stat`, async route => {
        statRequests++;
        const response = await route.fetch();
        await gate.wait();
        // The unfixed reproduction aborts these requests on every redraw.
        // Late fulfillment is irrelevant; request counts are the independent oracle.
        try { await route.fulfill({ response }); } catch { /* hover was cancelled */ }
      });
      try {
        await page.goto('/');
        await attachSession(page, session.id);
        await page.keyboard.press('Enter');
        await waitForTermText(page, `TARGET ${label} TICK 0000`);
        await hoverPath(page, label);
        const tooltip = page.locator('#terminal .terminal-file-target');
        await expect(tooltip).toContainText('checking…');
        await expect.poll(() => statRequests).toBe(1);
        await observePathRedraws(page, label);
        const peer = (await listSessions(request)).sessions.find(row => row.id === session.id);
        expect(peer?.status?.state, 'the gated redraw peer must still be alive before input').toMatch(/running|idle/);
        await page.keyboard.press('Enter');
        const snapshot = () => page.evaluate(() => ({ ...(window as any).__pathRedraw.state }));
        await expect.poll(async () => (await snapshot()).ticks, { message: 'live bytes must redraw the hovered row' }).toBeGreaterThanOrEqual(3);
        const pendingStart = await snapshot();
        // sleep-ok: observe repeated redraws under one held stat reply; rendered tick changes below prove the stimulus continued.
        await page.waitForTimeout(250);
        const pendingEnd = await snapshot();
        expect(pendingEnd.ticks).toBeGreaterThan(pendingStart.ticks);
        const pendingRequests = statRequests;
        await testInfo.attach('pending-hover-redraws', { body: JSON.stringify({ pendingRequests, pendingStart, pendingEnd }), contentType: 'application/json' });
        gate.release();
        const result = targetKind === 'folder' ? 'folder: not downloadable' : 'click to download';
        await expect(tooltip).toContainText(result);
        const readyStart = await snapshot();
        // sleep-ok: observe an answered hover through further real redraws; this is a no-restart window, not a readiness wait.
        await page.waitForTimeout(300);
        const readyEnd = await snapshot();
        expect(readyEnd.ticks).toBeGreaterThan(readyStart.ticks);
        const evidence = { linkKind, targetKind, pendingRequests, answeredRequests: statRequests, pendingStart, pendingEnd, readyStart, readyEnd };
        await testInfo.attach('resting-hover-redraws', { body: JSON.stringify(evidence), contentType: 'application/json' });
        expect(evidence, 'one resting hover must keep its lookup and tooltip through live redraws').toMatchObject({
          pendingRequests: 1, answeredRequests: 1, readyEnd: { hides: 0, restarts: 0 },
        });
        await expect(tooltip).toContainText(result);
        if (targetKind === 'file' || linkKind === 'OSC') await expect(page.locator('#terminal .xterm-screen')).toHaveClass(/xterm-cursor-pointer/);
        else await expect(page.locator('#terminal .xterm-screen')).not.toHaveClass(/xterm-cursor-pointer/);
        const screen = (await page.locator('#terminal .xterm-screen').boundingBox())!;
        await page.mouse.move(screen.x + screen.width - 2, screen.y - 2);
        await expect(tooltip).toBeHidden();
        await hoverPath(page, label);
        await expect.poll(() => statRequests).toBe(2);
        await expect(tooltip).toContainText(result);
      } finally {
        gate.release();
        await page.evaluate(() => (window as any).__pathRedraw?.dispose()).catch(() => {});
        await cleanupSession(request, session.id);
        fs.rmSync(directory, { recursive: true, force: true });
      }
    });
  }
}

/** Changing only an OSC label is a new hover even when its target and cell
 * range stay fixed. The held old reply proves cancellation, and a real second
 * reply proves that unchanged URI/range alone cannot retain old evidence.
 */
test('changed OSC display text starts a fresh hover at the same cells', async ({ page, request }) => {
  const directory = stackScratchDir('file-label-');
  const filename = path.join(directory, 'report.txt');
  fs.writeFileSync(filename, 'label replacement');
  expect(fs.readFileSync(filename, 'utf8')).toBe('label replacement');
  const span = (label: string) => `\\033]8;;file://${filename}\\007${label}\\033]8;;\\007`;
  const session = await createSession(request, { title: `file-label-${Date.now()}`, cwd: directory,
    invocation: `sh -c 'stty -echo; read _gate; printf "TARGET ${span('OLD-LABEL')}"; read _replace; printf "\\rTARGET ${span('NEW-LABEL')}"; sleep 300'` });
  const gate = routeGate();
  let arrived = false;
  let routeDone = false;
  let statRequests = 0;
  page.on('request', req => { if (req.url().endsWith(`/sessions/${session.id}/files/stat`)) statRequests++; });
  await page.route(`**/api/sessions/${session.id}/files/stat`, async route => {
    const response = await route.fetch();
    arrived = true;
    await gate.wait();
    // The requestfailed event below establishes cancellation independently
    // of whether Playwright accepts fulfillment after that cancellation.
    try { await route.fulfill({ response }); } catch { /* old hover cancelled */ }
    routeDone = true;
  }, { times: 1 });
  try {
    await page.goto('/');
    await attachSession(page, session.id);
    const peer = async () => (await listSessions(request)).sessions.find(row => row.id === session.id);
    expect((await peer())?.status?.state, 'initial gated peer must be alive before input').toMatch(/running|idle/);
    await page.keyboard.press('Enter');
    await waitForTermText(page, 'TARGET OLD-LABEL');
    const cancelled = page.waitForEvent('requestfailed', req => req.url().endsWith(`/sessions/${session.id}/files/stat`));
    await hoverPath(page, 'OLD-LABEL');
    const tooltip = page.locator('#terminal .terminal-file-target');
    await expect(tooltip).toContainText('checking…');
    await expect.poll(() => arrived).toBe(true);
    expect(statRequests).toBe(1);
    expect((await peer())?.status?.state, 'replacement peer must still be alive before input').toMatch(/running|idle/);
    await page.keyboard.press('Enter');
    await waitForTermText(page, 'TARGET NEW-LABEL');
    expect((await cancelled).failure()).not.toBeNull();
    await expect.poll(() => statRequests).toBe(2);
    await expect(tooltip).toContainText('17 B · click to download');
    gate.release();
    await expect.poll(() => routeDone).toBe(true);
    await expect(tooltip).toContainText('17 B · click to download');
  } finally {
    gate.release();
    await cleanupSession(request, session.id);
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

/** Real sparse files establish each exact size boundary without transferring
 * them. The rendered stripe and text must agree with the host's byte answer,
 * and the new layout must keep both host and full path visible.
 */
test('file hover shows decimal size tiers and a red cap refusal', async ({ page, request }, testInfo) => {
  const directory = stackScratchDir('file-tiers-');
  const cases = [
    ['small.txt', 4800, '4.8 KB', 'small', '--ok'],
    ['medium.pdf', 1000000, '1 MB', 'medium', '--info'],
    ['large.bin', 10000000, '10 MB', 'large', '--warn'],
    ['limit.txt', 100000000, '100 MB', 'large', '--warn'],
    ['refused.txt', 100000001, '100 MB limit', 'danger', '--danger'],
  ] as const;
  for (const [name, size] of cases) {
    const fd = fs.openSync(path.join(directory, name), 'w');
    fs.ftruncateSync(fd, size); fs.closeSync(fd);
    expect(fs.statSync(path.join(directory, name)).size).toBe(size);
  }
  const session = await createSession(request, { title: `file-tiers-${Date.now()}`, cwd: directory,
    invocation: `sh -c 'read _gate; printf "${cases.map(([name]) => name).join('\\n')}\\n"; sleep 300'` });
  try {
    await page.goto('/');
    await attachSession(page, session.id);
    await page.keyboard.press('Enter');
    await waitForTermText(page, 'refused.txt');
    const tooltip = page.locator('#terminal .terminal-file-target');
    for (const [name, , size, tier, token] of cases) {
      await hoverPath(page, name);
      await expect(tooltip).toHaveAttribute('data-tier', tier);
      await expect(tooltip.locator('.terminal-file-name')).toContainText(name);
      await expect(tooltip.locator('.terminal-file-directory')).toHaveText(`this machine:${fs.realpathSync(directory)}/`);
      await expect(tooltip).toContainText(size);
      const colors = await tooltip.evaluate((element, token) => {
        const probe = document.createElement('span');
        probe.style.color = `var(${token})`;
        element.appendChild(probe);
        const expected = getComputedStyle(probe).color;
        probe.remove();
        return { expected, actual: getComputedStyle(element).borderLeftColor };
      }, token);
      expect(colors.actual).toBe(colors.expected);
      if (tier === 'danger') await expect(tooltip.locator('.terminal-file-refusal')).toContainText('too large to download');
    }
    await testInfo.attach('file-hover-cap-refusal', { body: await tooltip.screenshot(), contentType: 'image/png' });
  } finally {
    await cleanupSession(request, session.id);
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

for (const kind of ['local', 'ssh']) {
  /** A fresh hover must disclose the actual owning host before a real save.
   * Holding stat proves checking and click refusal without a timing sleep.
   * The post-replay Enter releases live output, rather than testing replay.
   */
  test(`file hover and complete browser download from ${kind} host`, async ({ page, request }) => {
    const host = await connectedHost(request, kind);
    const directory = stackScratchDir('file-download-');
    const basename = 'report.pdf';
    const content = `download from ${kind}\n`;
    fs.writeFileSync(path.join(directory, basename), content);
    // A lexical URL normalization would select the top-level report instead
    // of following link/.. on Unix. Distinct bytes prove the filesystem route.
    fs.mkdirSync(path.join(directory, 'target', 'subdir'), { recursive: true });
    fs.symlinkSync(path.join(directory, 'target', 'subdir'), path.join(directory, 'link'));
    fs.writeFileSync(path.join(directory, 'target', basename), 'symlink parent bytes');
    expect(fs.readFileSync(path.join(directory, basename), 'utf8')).toBe(content);
    const session = await createSession(request, { title: `file-${kind}-${Date.now()}`, host, cwd: directory,
      invocation: `sh -c 'read _gate; printf "FILE ${basename}:42:7.\\n\\033]8;;file://${directory}/link/../${basename}\\007DOTSEG-FILE\\033]8;;\\007\\n"; sleep 300'` });
    const gate = routeGate();
    let statRequests = 0;
    let received: any;
    await page.route(`**/api/sessions/${session.id}/files/stat`, async route => {
      statRequests++;
      const response = await route.fetch();
      received = await response.json();
      await gate.wait();
      await route.fulfill({ response });
    });
    try {
      await page.goto('/');
      await attachSession(page, session.id);
      await page.keyboard.press('Enter');
      await waitForTermText(page, `FILE ${basename}`);
      const point = await hoverPath(page, basename);
      const tooltip = page.locator('#terminal .terminal-file-target');
      await expect(tooltip).toBeVisible();
      await expect(tooltip).toContainText('checking…');
      await expect(tooltip).toHaveClass(/terminal-file-checking/);
      await expect(page.locator('#terminal .xterm-screen')).not.toHaveClass(/xterm-cursor-pointer/);
      let downloadRequests = 0;
      page.on('request', req => { if (req.url().endsWith('/files/download')) downloadRequests++; });
      await page.mouse.click(point.x, point.y);
      expect(downloadRequests).toBe(0);
      await expect.poll(() => received?.status).toBe('ready');
      expect(received.path).toBe(fs.realpathSync(path.join(directory, basename)));
      expect(received.size).toBe(Buffer.byteLength(content));
      expect(received.host).toBe(kind === 'local' ? 'this machine' : 'localhost');
      gate.release();
      await expect(tooltip.locator('.terminal-file-name')).toHaveText(`\uf1c1 ${basename}`);
      await expect(tooltip.locator('.terminal-file-directory')).toHaveText(`${received.host}:${path.dirname(received.path)}/`);
      await expect(tooltip.locator('.terminal-file-size')).toHaveText(`${Buffer.byteLength(content)} B · click to download`);
      await expect(tooltip).toHaveAttribute('data-tier', 'small');
      await expect(tooltip).toHaveCSS('border-left-width', '3px');
      await expect(tooltip).not.toContainText('click to open');
      await expect(page.locator('#terminal .xterm-screen')).toHaveClass(/xterm-cursor-pointer/);
      const saved = page.waitForEvent('download');
      await page.mouse.click(point.x, point.y);
      const download = await saved;
      expect(download.suggestedFilename()).toBe(basename);
      expect(await download.failure()).toBeNull();
      expect(fs.readFileSync((await download.path())!, 'utf8')).toBe(content);
      const notice = page.locator('#terminal .terminal-file-message');
      await expect(notice).toContainText(`downloaded ${basename}`);
      await expect(notice).toHaveClass(/showing/);
      const terminalBox = (await page.locator('#terminal .xterm').boundingBox())!;
      const noticeBox = (await notice.boundingBox())!;
      expect(noticeBox.y - terminalBox.y).toBeCloseTo(4, 0);
      await expect(notice).toHaveCSS('opacity', '0', { timeout: 8000 });
      await hoverPath(page, basename);
      await expect.poll(() => statRequests).toBe(2);
      await expect(tooltip).toContainText('click to download');
      const linked = await hoverPath(page, 'DOTSEG-FILE');
      await expect(tooltip.locator('.terminal-file-name')).toContainText(basename);
      await expect(tooltip.locator('.terminal-file-directory')).toHaveText(`${received.host}:${fs.realpathSync(path.join(directory, 'target'))}/`);
      const linkedSave = page.waitForEvent('download');
      // A browser modifier must still make an ordinary download. Observe the
      // actual request so a successful Blob alone cannot hide an open request.
      const linkedRequest = page.waitForRequest(req => req.url().endsWith('/files/download'));
      await page.keyboard.down('Meta');
      try { await page.mouse.click(linked.x, linked.y); } finally { await page.keyboard.up('Meta'); }
      expect((await linkedRequest).postDataJSON()).toMatchObject({ open: false, save_to_downloads: false });
      const linkedDownload = await linkedSave;
      expect(await linkedDownload.failure()).toBeNull();
      expect(fs.readFileSync((await linkedDownload.path())!, 'utf8')).toBe('symlink parent bytes');
    } finally {
      gate.release();
      await cleanupSession(request, session.id);
      fs.rmSync(directory, { recursive: true, force: true });
    }
  });
}

/** Missing, directory, permission and cap refusals must remain non-clickable,
 * while OSC files share admission and forbidden OSC schemes never execute.
 */
test('file refusals, OSC file save and forbidden OSC schemes', async ({ page, request }) => {
  const directory = stackScratchDir('file-refusals-');
  fs.writeFileSync(path.join(directory, 'ok.txt'), 'OSC file bytes');
  fs.mkdirSync(path.join(directory, 'folder.dir'));
  fs.writeFileSync(path.join(directory, 'private.txt'), 'unreadable');
  fs.chmodSync(path.join(directory, 'private.txt'), 0);
  const large = fs.openSync(path.join(directory, 'large.bin'), 'w');
  fs.ftruncateSync(large, 100000001); fs.closeSync(large);
  expect(fs.statSync(path.join(directory, 'large.bin')).size).toBe(100000001);
  expect(fs.statSync(path.join(directory, 'private.txt')).mode & 0o777).toBe(0);
  const script = `printf "missing.txt\\nfolder.dir\\nprivate.txt\\nlarge.bin\\n\\033]8;;file://${directory}/ok.txt\\007OSC-FILE\\033]8;;\\007\\n\\033]8;;javascript:window.__fileSchemeRan=1\\007OSC-JS\\033]8;;\\007\\n\\033]8;;data:text/html,evil\\007OSC-DATA\\033]8;;\\007\\n\\033]8;;https://right.example\\007https://wrong.example\\033]8;;\\007\\n"`;
  const session = await createSession(request, { title: `file-refusals-${Date.now()}`, cwd: directory,
    invocation: `sh -c 'read _gate; ${script}; sleep 300'` });
  await page.addInitScript(() => { (window as any).__fileOpened = []; window.open = ((uri: string) => { (window as any).__fileOpened.push(uri); return null; }) as any; });
  try {
    await page.goto('/');
    await attachSession(page, session.id);
    await page.keyboard.press('Enter');
    await waitForTermText(page, 'OSC-DATA');
    for (const [name, refusal] of [['missing.txt', 'not found'], ['folder.dir', 'folder: not downloadable'], ['private.txt', 'not readable'], ['large.bin', '100 MB limit']]) {
      await hoverPath(page, name);
      await expect(page.locator('#terminal .terminal-file-target')).toContainText(refusal);
      await expect(page.locator('#terminal .xterm-screen')).not.toHaveClass(/xterm-cursor-pointer/);
    }
    await hoverPath(page, 'OSC-FILE');
    await expect(page.locator('#terminal .terminal-file-target')).toContainText('click to download');
    await hoverPath(page, 'https://wrong.example');
    await expect(page.locator('#terminal .terminal-link-target-mismatch')).toBeVisible();
    const point = await hoverPath(page, 'OSC-FILE');
    await expect(page.locator('#terminal .terminal-file-target')).not.toHaveClass(/terminal-link-target-mismatch/);
    await expect(page.locator('#terminal .terminal-file-target')).toContainText('click to download');
    const saved = page.waitForEvent('download');
    await page.mouse.click(point.x, point.y);
    const download = await saved;
    expect(download.suggestedFilename()).toBe('ok.txt');
    expect(await download.failure()).toBeNull();
    expect(fs.readFileSync((await download.path())!, 'utf8')).toBe('OSC file bytes');
    for (const label of ['OSC-JS', 'OSC-DATA']) {
      const at = await hoverPath(page, label);
      await page.mouse.click(at.x, at.y);
    }
    expect(await page.evaluate(() => (window as any).__fileSchemeRan)).toBeUndefined();
    expect(await page.evaluate(() => (window as any).__fileOpened)).toEqual([]);
    expect(new URL(page.url()).pathname).toBe('/');
    const vanished = await hoverPath(page, 'OSC-FILE');
    await expect(page.locator('#terminal .terminal-file-target')).toContainText('click to download');
    fs.unlinkSync(path.join(directory, 'ok.txt'));
    expect(fs.existsSync(path.join(directory, 'ok.txt'))).toBe(false);
    await page.mouse.click(vanished.x, vanished.y);
    const notice = page.locator('#terminal .terminal-file-message');
    await expect(notice).toContainText('download failed:');
    await expect(notice).toHaveClass(/showing/);
    // A blank terminal cell dismisses the failure without starting another save.
    const screen = (await page.locator('#terminal .xterm-screen').boundingBox())!;
    await page.mouse.click(screen.x + screen.width - 2, screen.y + screen.height - 2);
    await expect(notice).toHaveText('');
    await expect(notice).not.toHaveClass(/showing/);
  } finally {
    await cleanupSession(request, session.id);
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

/** A pending hover must abort on departure. Late evidence cannot re-admit a
 * click; a later hover retries the real host and finds a newly written file.
 */
test('leaving cancels lookup and the next hover checks again', async ({ page, request }) => {
  const directory = stackScratchDir('file-cancel-');
  const session = await createSession(request, { title: `file-cancel-${Date.now()}`, cwd: directory,
    invocation: `sh -c 'read _gate; printf "fresh.txt\\n"; sleep 300'` });
  const gate = routeGate();
  let arrived = false;
  let routeDone = false;
  await page.route(`**/api/sessions/${session.id}/files/stat`, async route => {
    const response = await route.fetch();
    arrived = true;
    await gate.wait();
    // Aborted browser requests may refuse fulfillment. That is the intended
    // cancellation, established separately by the requestfailed event below.
    try { await route.fulfill({ response }); } catch { /* already cancelled */ }
    routeDone = true;
  }, { times: 1 });
  try {
    await page.goto('/');
    await attachSession(page, session.id);
    await page.keyboard.press('Enter');
    await waitForTermText(page, 'fresh.txt');
    const failed = page.waitForEvent('requestfailed', req => req.url().endsWith('/files/stat'));
    await hoverPath(page, 'fresh.txt');
    await expect(page.locator('#terminal .terminal-file-target')).toContainText('checking…');
    await expect.poll(() => arrived).toBe(true);
    const box = (await page.locator('#terminal .xterm-screen').boundingBox())!;
    await page.mouse.move(box.x + box.width - 2, box.y - 2);
    expect((await failed).failure()).not.toBeNull();
    gate.release();
    await expect.poll(() => routeDone).toBe(true);
    await expect(page.locator('#terminal .terminal-file-target')).toBeHidden();
    fs.writeFileSync(path.join(directory, 'fresh.txt'), 'new file');
    expect(fs.readFileSync(path.join(directory, 'fresh.txt'), 'utf8')).toBe('new file');
    await hoverPath(page, 'fresh.txt');
    await expect(page.locator('#terminal .terminal-file-target')).toContainText('8 B');
    await expect(page.locator('#terminal .xterm-screen')).toHaveClass(/xterm-cursor-pointer/);
  } finally {
    gate.release();
    await cleanupSession(request, session.id);
    fs.rmSync(directory, { recursive: true, force: true });
  }
});
