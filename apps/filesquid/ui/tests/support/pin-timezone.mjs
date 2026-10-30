/**
 * Vitest global setup: run every test in UTC. The Drive view words dates by local calendar day
 * ("Yesterday", the Recent day groups), and the fixtures state their times in UTC, so a machine
 * in another zone would otherwise see a different day. The variable is set in the main process
 * before the workers start, so they inherit it.
 *
 * @returns Nothing.
 */
export default function pinTimezone() {
  process.env.TZ = "UTC";
}
