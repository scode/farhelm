import { readFileSync } from "node:fs";
import path from "node:path";
import type { BrowserContext } from "@playwright/test";

/** Browser-local key consumed by the UI's authenticated send path. */
export const DEVICE_SECRET_KEY = "farhelm.device-secret";
/** The single-stack capture configs' credential file (readme-hero, readme-video, docs-shots).
 *
 * The browser suite proper keeps one credential per stack slot instead
 * (`StackSlot.authStatePath` in stack-slots.ts), rewritten whenever the auth
 * spec rotates that stack's token.
 */
export const AUTH_STORAGE_STATE_PATH = path.resolve(
  __dirname,
  "../../.auth/storage-state.json",
);

/** Tells a stack script which credential file to remove with its stack. */
export const AUTH_STATE_ENV = "FARHELM_E2E_AUTH_STATE";

/** The storage-state subset needed to recover the harness device secret. */
interface StorageState {
  origins?: Array<{
    localStorage?: Array<{ name?: unknown; value?: unknown }>;
  }>;
}

/** Read the most recently persisted harness credential, including rotations.
 *
 * Playwright reloads its config in each worker. Reading the stack's storage
 * state there lets a later project on the same stack inherit a rotation
 * performed by an earlier project, even though worker environment changes
 * cannot flow back to the runner process. Rotation is per stack: each slot has
 * its own credential file, and a rotation on one stack leaves the others
 * alone.
 */
function storedDeviceSecret(authStatePath: string): string | undefined {
  let state: StorageState;
  try {
    state = JSON.parse(readFileSync(authStatePath, "utf8")) as StorageState;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return undefined;
    throw error;
  }

  for (const origin of state.origins ?? []) {
    for (const item of origin.localStorage ?? []) {
      if (
        item.name === DEVICE_SECRET_KEY &&
        typeof item.value === "string" &&
        item.value !== ""
      ) {
        return item.value;
      }
    }
  }
  return undefined;
}

/** Build the ambient request header used by every authenticated test client.
 *
 * The file is authoritative because auth.spec.ts can rotate the credential.
 * Keeping it as the only source also prevents a test-body refresh from
 * leaking a device secret through the test process environment. The suite
 * passes its slot's file; the single-stack capture configs use the default.
 */
export function harnessAuthorizationHeaders(
  authStatePath: string = AUTH_STORAGE_STATE_PATH,
): Record<string, string> | undefined {
  const secret = storedDeviceSecret(authStatePath);
  return secret ? { Authorization: `Bearer ${secret}` } : undefined;
}

/** Advance a running worker and its future contexts after token rotation. */
export function refreshHarnessAuthorization(
  headers: Record<string, string> | undefined,
  secret: string,
): void {
  if (!headers) {
    throw new Error("the Playwright worker has no harness Authorization headers");
  }
  headers.Authorization = `Bearer ${secret}`;
}

/** Force page traffic to use the product's own credential transports.
 *
 * Playwright applies config-level `extraHTTPHeaders` to browser contexts as
 * well as API request fixtures. Clearing that ambient bearer header makes an
 * in-page fetch prove the UI send path and a WebSocket prove the device
 * subprotocol instead of inheriting engine-specific upgrade behavior. This
 * does not change the separately constructed `request` fixture.
 */
export async function requireProductPageAuth(context: BrowserContext): Promise<void> {
  await context.setExtraHTTPHeaders({});
}
