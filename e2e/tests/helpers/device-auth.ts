import { readFileSync } from "node:fs";
import path from "node:path";
import type { BrowserContext } from "@playwright/test";

/** Environment handoff from global setup to newly launched workers. */
export const DEVICE_SECRET_ENV = "FARHELM_E2E_DEVICE_SECRET";
/** Browser-local key consumed by the UI's authenticated send path. */
export const DEVICE_SECRET_KEY = "farhelm.device-secret";
/** Shared state rewritten whenever the auth spec rotates the credential. */
export const AUTH_STORAGE_STATE_PATH = path.resolve(
  __dirname,
  "../../.auth/storage-state.json",
);

/** The storage-state subset needed to recover the harness device secret. */
interface StorageState {
  origins?: Array<{
    localStorage?: Array<{ name?: unknown; value?: unknown }>;
  }>;
}

/** Read the most recently persisted harness credential, including rotations.
 *
 * Playwright reloads its config in each worker. Reading the shared storage
 * state there lets a later project inherit a rotation performed by an earlier
 * project, even though worker environment changes cannot flow back to the
 * runner process.
 */
function storedDeviceSecret(): string | undefined {
  let state: StorageState;
  try {
    state = JSON.parse(readFileSync(AUTH_STORAGE_STATE_PATH, "utf8")) as StorageState;
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
 * The environment fallback covers a first run before any storage state exists
 * and is also the explicit handoff from global setup to Playwright workers.
 */
export function harnessAuthorizationHeaders(): Record<string, string> | undefined {
  const secret = storedDeviceSecret() ?? process.env[DEVICE_SECRET_ENV];
  return secret ? { Authorization: `Bearer ${secret}` } : undefined;
}

/** Publish a newly minted credential to children of the current process. */
export function exposeHarnessDeviceSecret(secret: string): void {
  process.env[DEVICE_SECRET_ENV] = secret;
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
  exposeHarnessDeviceSecret(secret);
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
