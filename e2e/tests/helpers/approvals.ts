// The approval cards an agent's `farhelm` command raises (SPEC.md,
// Agent-spawned sessions), as a spec answers them: through the real card in a
// real page, never through the helm's REST answer route, so the GUI half of
// the prompt is what the spec exercises.
import { expect } from "./evidence";
import { type APIRequestContext, type Page } from "@playwright/test";
import { listHosts, localHostId } from "./fleet";

/** How a spec answers a card; each is the card's own button. */
export type CardAnswer = "allow" | "always-allow" | "deny";

/**
 * Wait for exactly one approval card on `page` that contains every string in
 * `shows`, then answer it with `answer` and wait for it to go.
 *
 * Waiting on the card's own text is the readiness oracle: a card that appears
 * but describes something else (another session's request, a different
 * folder) fails here, naming what it did show, rather than being approved.
 */
export async function answerCard(
  page: Page,
  answer: CardAnswer,
  shows: string[],
  appearWithinMs = 20_000,
): Promise<void> {
  const card = page.locator(".approval-card");
  await expect(card, "an approval card appears for the agent's request").toHaveCount(1, {
    timeout: appearWithinMs,
  });
  for (const text of shows) {
    await expect(card, `the card shows ${JSON.stringify(text)}`).toContainText(text);
  }
  await card.locator(`.approval-${answer}`).click();
  await expect(card, "the answered card goes away").toHaveCount(0, { timeout: 20_000 });
}

/**
 * Set whether the local host runs agents' `farhelm` commands without asking,
 * and read the setting back.
 *
 * The suite shares one helm, so a spec that changes it must put it back; the
 * read-back makes a spec whose agent then behaves unexpectedly fail here,
 * naming the setting, rather than later on a card that did or did not appear.
 */
export async function setLocalCommandsWithoutAsking(
  request: APIRequestContext,
  commandsWithoutAsking: boolean,
): Promise<void> {
  const id = await localHostId(request);
  const response = await request.post(`/api/hosts/${id}/commands-without-asking`, {
    data: { commands_without_asking: commandsWithoutAsking },
  });
  if (!response.ok()) {
    throw new Error(`setting commands_without_asking failed (${response.status()}): ${await response.text()}`);
  }
  const local = (await listHosts(request)).find((host) => host.id === id);
  if (local?.commands_without_asking !== commandsWithoutAsking) {
    throw new Error(
      `the local host's commands_without_asking reads ${local?.commands_without_asking}, not ${commandsWithoutAsking}`,
    );
  }
}
