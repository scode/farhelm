/**
 * Record the README demo video (docs/readme-video/SPEC.md).
 *
 * NOTE: the beats below realise a THROWAWAY sample intent that exists only
 * to show the machinery working; see the note at the top of `intent.md`.
 * The structure (stage, open, install the overlay, record, numbered beats,
 * finish) is what a real video keeps; the beats themselves get replaced.
 *
 * This file is the COMPILED form of `docs/readme-video/intent.md`: the
 * intent says what happens and what the annotations say, and this spec says
 * how to make that happen against the UI as it is today (selectors, waits,
 * pacing). When the UI changes, this file is rewritten to realise the same
 * intent; when the intent changes, this file follows. Each beat below is
 * numbered as in the intent so the two can be read side by side, and the
 * annotation strings are copied from it verbatim.
 *
 * Not a test in the ordinary sense: it produces an MP4, not a verdict, and
 * nothing in CI runs it. The waits are still real readiness checks, so a
 * beat that cannot happen on the current UI fails the capture rather than
 * recording a video of nothing happening.
 */
import { expect, test, type APIRequestContext, type Page } from "@playwright/test";
import { mkdirSync } from "node:fs";
import path from "node:path";
import { waitForTermText } from "../tests/helpers/term";
import { waitForSessionReady } from "../tests/helpers/terminal-readiness";
import { loadScenario } from "../readme-hero/scenario";
import { openStagedFleet, readStackInfo, sessions, stageFleet } from "../readme-hero/stage";
import { Director } from "./overlay";
import { VIDEO_DIR, VIDEO_STACK_INFO_PATH } from "./paths";
import { Recorder } from "./recorder";

/** Where the MP4 goes. The capture script sets it; a bare run lands under target/. */
const OUTPUT_ENV = "FARHELM_VIDEO_OUTPUT";

/** Poll the helm until session `id` is classified `state`: the supervisor's verdict, not the page's. */
async function waitForStatus(request: APIRequestContext, id: string, state: string, timeout = 60_000) {
  await expect.poll(
    async () => (await sessions(request)).find((row) => row.id === id)?.status?.state,
    { timeout, intervals: [500], message: `session ${id} must reach ${state}` },
  ).toBe(state);
}

/**
 * Fail unless xterm's own input element has focus. Keys typed anywhere else
 * go nowhere useful, and the failure would otherwise surface much later as
 * a transcript line that never appeared, hiding the cause.
 */
async function expectTerminalFocused(page: Page) {
  await expect.poll(
    () => page.evaluate(() => document.activeElement === (window as any).__farhelmTerm?.textarea),
    { timeout: 5_000, message: "the terminal must have keyboard focus before typing into it" },
  ).toBe(true);
}

test("record the README demo video", async ({ page, request }) => {
  const scenario = loadScenario(VIDEO_DIR);
  const info = readStackInfo(VIDEO_STACK_INFO_PATH);
  const output = process.env[OUTPUT_ENV] ||
    path.resolve(__dirname, "../../target/readme-video/readme-video.mp4");
  mkdirSync(path.dirname(output), { recursive: true });

  const fleet = await stageFleet(page, request, scenario, VIDEO_DIR, info);
  await openStagedFleet(page, request, scenario, VIDEO_DIR, fleet);

  const migrateId = fleet.ids.get("migrate auth tokens") as string;
  const list = page.locator(".session-list");
  const migrateRow = page.locator(`[data-session-id="${migrateId}"]`);

  const director = await Director.install(page);
  // The title card goes up before recording starts, so the first frame of
  // the video is the card rather than a flash of the bare app.
  const title = await director.card("farhelm", "Every coding agent, on every machine, in one place.");
  const recorder = await Recorder.start(page, { output });

  // Beat 1: title card.
  await director.hold(1_500);
  recorder.mark("title card");
  await director.hold(1_500);
  await title.dismiss();
  await director.hold(800);

  // Beat 2: the session list. The box is pushed down below the open
  // terminal's transcript so it covers none of it; the ring keeps it clear
  // what the arrow means.
  const ring = await director.highlight(list, { pad: 2 });
  let note = await director.callout(list, "Every session on every host, with live status.", { side: "right", dy: 160 });
  recorder.mark("session list callout");
  await director.hold(2_800);
  await Promise.all([ring.dismiss(), note.dismiss()]);

  // Beat 3: the waiting session, spotlit.
  const spot = await director.highlight(migrateRow, { spotlight: true, pad: 4 });
  note = await director.callout(migrateRow, "This one is waiting on you.", { side: "right" });
  recorder.mark("waiting session spotlight");
  await director.hold(2_500);
  await Promise.all([spot.dismiss(), note.dismiss()]);

  // Beat 4: open it. The row's open control is the click target the UI
  // offers for selecting a session; the question is the transcript's own.
  // The callout points at the question rather than at the pane: an arrow
  // from outside a target that fills most of the screen has nowhere to go.
  const question = "Do you want to apply the migration to staging?";
  await director.click(migrateRow.locator(".session-row-open"));
  await waitForSessionReady(page, migrateId);
  await waitForTermText(page, question, 20_000);
  await director.hold(600);
  // Nudged down so the box clears the end of the line above the question.
  note = await director.callout(
    await director.terminalText(question),
    "The agent's real terminal, right in the browser.",
    { side: "right", dy: 50 },
  );
  recorder.mark("terminal callout");
  await director.hold(2_800);
  await note.dismiss();

  // Beat 5: answer. The pointer clicks the option it is choosing, which
  // also (re)focuses xterm, and the key press is what actually answers.
  await director.click(await director.terminalText("1. Yes"));
  await expectTerminalFocused(page);
  await director.hold(400);
  await director.press("1");
  await waitForTermText(page, "./scripts/migrate --env staging 0042", 10_000);
  await director.hold(1_200);

  // Beat 6: the status catches up. The supervisor samples sessions in turn,
  // so the flip can take several seconds; that stretch is cut from the
  // video rather than shown as dead air. The cut ends on what the viewer
  // sees, the row's own badge, not just on the helm's verdict: the list
  // repaints only after its next refetch, and resuming in between would
  // point the callout at a row still showing waiting.
  recorder.pause();
  await waitForStatus(request, migrateId, "running");
  await expect(migrateRow.locator(".status-badge.running")).toBeVisible({ timeout: 15_000 });
  recorder.resume();
  // Shifted right, out of the sidebar, so the box does not sit on the row
  // whose status it is talking about.
  note = await director.callout(
    migrateRow.locator(".session-status-slot"),
    "Status follows the agent: working again.",
    { side: "right", dx: 260 },
  );
  recorder.mark("status callout");
  await director.hold(2_800);
  await note.dismiss();

  // Beat 7: the follow-up. The migration's spinner runs for a fixed stretch
  // before the prompt appears; that wait is cut too. Keys typed between the
  // question line and the fake agent's prompt are not lost (its terminal is
  // already raw, so they wait in its input), so the question is a sufficient
  // readiness point.
  recorder.pause();
  await waitForTermText(page, "Anything else before production?", 40_000);
  recorder.resume();
  const caption = await director.caption("Type to it like you would locally.");
  recorder.mark("typing caption");
  await director.hold(600);
  await expectTerminalFocused(page);
  await director.type("also update the rollout doc");
  await director.hold(400);
  await director.press("Enter");
  await waitForTermText(page, "Done. The rollout doc now covers staging and rollback.", 10_000);
  await director.hold(1_800);
  await caption.dismiss();

  // Beat 8: end card.
  await director.card("farhelm", "Watch your agents. Answer them from anywhere.");
  await director.hold(1_200);
  recorder.mark("end card");
  await director.hold(2_000);

  const marks = await recorder.finish();
  console.log(`README demo video written to ${output}`);
  for (const mark of marks) console.log(`  ${mark.at.toFixed(2)}s  ${mark.label}  (${mark.still})`);
});
