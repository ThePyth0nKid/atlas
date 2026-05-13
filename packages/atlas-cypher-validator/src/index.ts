/**
 * @atlas/cypher-validator — public API surface.
 *
 * Single export point. Consumers import from `@atlas/cypher-validator`
 * (no sub-path imports). This keeps the surface flat and the
 * dependency graph simple for tree-shakers.
 *
 * Extracted in V2-β Welle 15 from W12 (atlas-web) and W13
 * (atlas-mcp-server) inline copies. See ADR-Atlas-009 for rationale.
 */

export type { CypherValidationResult } from "./validator.js";
export { validateReadOnlyCypher, CYPHER_MAX_LENGTH } from "./validator.js";
