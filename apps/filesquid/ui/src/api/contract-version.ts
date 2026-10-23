/**
 * The version of the FileSquid API contract (`crates/api/openapi.json`, `info.version`) this
 * package's generated types target. Settings, About shows it as "API: {status} · contract
 * {version}" (UI-S-033, owner answer 12). The contract drift test fails when it differs from the
 * committed contract, so it is updated together with `npm run contract:types`.
 */
export const CONTRACT_VERSION = "0.24.0";
