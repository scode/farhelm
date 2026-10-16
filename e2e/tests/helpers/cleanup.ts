/**
 * Attempt every owned cleanup step even if a previous release fails.
 * An assertion failure remains the primary failure; cleanup errors accompany
 * it in stderr. With a successful body, cleanup failures fail the test together.
 */
export async function cleanupOwnedSteps(bodyFailed: boolean, steps: [string, () => unknown | Promise<unknown>][]) {
  const failures: Error[] = [];
  for (const [label, step] of steps) {
    try {
      await step();
    } catch (error) {
      failures.push(new Error(`cleanup failed: ${label}`, { cause: error }));
    }
  }
  if (failures.length) {
    const error = new AggregateError(failures, "owned fixture cleanup failed");
    if (bodyFailed) console.error(error);
    else throw error;
  }
}
