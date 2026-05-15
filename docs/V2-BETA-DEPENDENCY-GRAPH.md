# V2-β Welle Dependency Graph

> **Status:** Phase-0 output, master-resident. **Effective 2026-05-13.** Companion to [`V2-BETA-ORCHESTRATION-PLAN.md`](V2-BETA-ORCHESTRATION-PLAN.md).
> **Purpose:** explicit dependency edges between V2-β wellen so that parallel-batch identification + serial-chain ordering is unambiguous.

## 1. Mermaid dependency diagram

```mermaid
graph TD
    Phase0[Phase 0: V2-β Orchestration Plan + Dependency Graph + Welle Template + ADR reservation<br/>HARD GATE — must land on master]

    Phase0 --> W9
    Phase0 --> W10
    Phase0 --> W11

    subgraph PB1[Phase 1: Parallel Batch 1 — docs-only, zero conflicts]
        W9[W9: Operator Runbook v2.0.0-alpha.1<br/>docs/OPERATOR-RUNBOOK-V2-ALPHA-1.md]
        W10[W10: Parallel-Projection Design ADR<br/>docs/ADR/ADR-Atlas-007-*.md]
        W11[W11: wasm-publish.yml dual-publish race fix<br/>.github/workflows/ + ADR-008]
    end

    W9 --> Phase2[Phase 2: Post-batch consolidation<br/>parent merges + writes shared-files commit]
    W10 --> Phase2
    W11 --> Phase2

    Phase2 --> Phase3[Phase 3: v2.0.0-alpha.2 ship<br/>version bump + signed tag + GitHub Release + npm]

    Phase3 --> W12
    Phase3 --> W13
    Phase3 --> W14

    subgraph PB2[Phase 4: Parallel Batch 2 — code wellen, zero conflicts]
        W12[W12: Read-API endpoints<br/>apps/atlas-web/src/app/api/atlas/<br/>6 routes; inline Cypher validator]
        W13[W13: MCP V2 tools<br/>apps/atlas-mcp-server/src/tools/<br/>5 tools; inline Cypher validator]
        W14[W14: Expanded event-kind support<br/>crates/atlas-projector/src/upsert.rs<br/>annotation_add, policy_set, anchor_created]
    end

    W12 --> W15
    W13 --> W15

    W15[W15: Cypher validator consolidation<br/>rule-of-three: extract shared module<br/>after W12 + W13 land]

    W14 --> W16
    W15 --> W16

    W16[W16: ArcadeDB pre-flight spike-doc<br/>docs/V2-BETA-ARCADEDB-SPIKE.md<br/>analog V2-α Welle 2<br/>ADR-Atlas-010]

    W16 --> W17a

    subgraph PB3[Phase 7: ArcadeDB driver — SERIAL multi-session]
        W17a[W17a: Planning + scaffold ✓ SHIPPED<br/>GraphStateBackend trait<br/>ADR-Atlas-011]
        W17b[W17b: ArcadeDB driver impl ✓ SHIPPED<br/>reqwest HTTP + Cypher per ADR-010 §4<br/>0 CRITICAL / 2 HIGH / 3 MEDIUM / 2 LOW all fixed in-commit]
        W17c[W17c: Docker-Compose CI + benchmark ✓ SHIPPED<br/>atlas-arcadedb-smoke workflow<br/>W17b Cypher hotfix + schema-bootstrap<br/>0 CRITICAL / 1 HIGH / 4 MEDIUM / 2 LOW all fixed in-commit<br/>byte-pin reproduces through live ArcadeDB]
    end

    W17a --> W17b
    W17b --> W17c

    W17c --> W18[W18 Phase A: Mem0g Layer-3 cache design ✓ SHIPPED<br/>spike + ADR-Atlas-012 + plan-doc<br/>8 binding sub-decisions<br/>0 CRITICAL / 2 HIGH / 10 MEDIUM / 4 LOW all fixed in-commit<br/>LanceDB embedded + fastembed-rs LOCKED]

    W18 --> W18b[W18b: Mem0g implementation ✓ SHIPPED<br/>NEW crates/atlas-mem0g/ ~2300 LOC<br/>W17a-pattern Phase-A-scaffold<br/>0 CRITICAL / 4 unique HIGH / 6 MEDIUM all fixed in-commit<br/>577 tests post-W18c-A retirement of pins_are_placeholder gatekeeper; clippy clean; byte-pin reproduces]

    W18b --> W18cA[W18c Phase A ✓ SHIPPED<br/>Mem0g supply-chain constants lifted<br/>9 compile-in pins BAAI/bge-small-en-v1.5 @ 5c38ec7c<br/>0 CRITICAL / 0 HIGH / 1 MEDIUM / 3-4 LOWs all in-commit<br/>embedder still fail-closed pending Phase B]

    W18cA --> W19[W19: v2.0.0-beta.1 ship ✓ SHIPPED<br/>signed tag 81d363e Good Ed25519<br/>GitHub Release prerelease<br/>npm latest = 2.0.0-beta.1<br/>Sigstore Build L3 provenance attached<br/>W11 race-fix validated 2nd time end-to-end; wasm-publish run-id 25919934805<br/>0 CRITICAL / 1 HIGH / 3 MEDIUM / 1 LOW PR #102 + 6/6 PASS PR #103]

    W18cA --> W18cB[W18c Phase B ✓ SHIPPED<br/>fastembed try_new_from_user_defined wired<br/>Layer 3 embedder OPERATIONAL<br/>11 compile-in pins (atomic 9→11 via 4th tokenizer pin discovery)<br/>read_and_verify TOCTOU-free primitive<br/>Mutex&lt;TextEmbedding&gt; + Pooling::Cls<br/>579 tests / clippy zero / mem0g-smoke 1m03s live HF<br/>0 CRITICAL / 2 HIGH code (1 dismissed Lesson #2) + 1 HIGH security TOCTOU / 4+2 MEDIUM all applied / 3+3 LOW]

    W19 --> Phase146[Phase 14.6 ✓ SHIPPED<br/>W19 consolidation: handoff §0z7 + DECISION-ARCH-W19<br/>+ master-plan §6 + orchestration + dep-graph + CHANGELOG<br/>NEW Lesson #19 pre-tag-push multi-perspective review]

    W18cB --> Phase147[Phase 14.7 ✓ SHIPPED<br/>W18c Phase B consolidation: handoff §0z8 + DECISION-ARCH-W18c-B<br/>+ master-plan §6 + orchestration + dep-graph + CHANGELOG<br/>mem0g-smoke promotion-candidate threshold MET 3 stable runs]

    W18b --> CounselGate{Nelson-led<br/>Counsel sign-off<br/>DECISION-COUNSEL-1<br/>blocks public materials NOT the tag itself}

    W18cB --> W18cC[W18c Phase C: V1-V4 verification gaps<br/>cross-platform CI Linux + Windows + macOS<br/>fastembed determinism + Lance v2.2 + LanceDB Windows]

    W18cC --> W18cD[W18c Phase D: LanceDB ANN/search body fill-in<br/>tokio::spawn_blocking-wrapped LanceDB calls<br/>NOT Handle::current().block_on() per spike §7<br/>activates /api/atlas/semantic-search real hits replacing 501 stub]

    style Phase0 fill:#ffcccc
    style Phase3 fill:#ccffcc
    style W18 fill:#ccffcc
    style W18b fill:#ccffcc
    style W18cA fill:#ccffcc
    style W19 fill:#ccffcc
    style Phase146 fill:#ccffcc
    style W18cB fill:#ccffcc
    style Phase147 fill:#ccffcc
    style W18cC fill:#ffffcc
    style W18cD fill:#ffffcc
    style CounselGate fill:#ffffcc
    style PB1 fill:#e6f3ff
    style PB2 fill:#e6f3ff
    style PB3 fill:#ffe6e6
```

## 2. Dependency edges (textual form)

```
Phase 0 (hard gate) → W9, W10, W11

W9, W10, W11 → Phase 2 (consolidation) → Phase 3 (alpha.2 ship)

Phase 3 → W12, W13, W14

W12, W13 → W15 (Cypher consolidation, rule-of-three)
W14, W15 → W16

W16 (spike) → W17a → W17b → W17c → W18 → W18b → W18c-A → W19 → Phase 14.6 → W18c-B → Phase 14.7

W18c-A → W18c-B SHIPPED 2026-05-15 (Layer 3 embedder OPERATIONAL post-Phase-B-SHIPPED; pin set atomically extended 9 → 11; TOCTOU eliminated via read_and_verify primitive)
W18c-B → W18c-C → W18c-D (engineering-pipeline parallel-trackable post-Phase-B-SHIPPED; Phase D activates /api/atlas/semantic-search real hits replacing 501 stub)

W18b → Counsel-Gate (Nelson-led, parallel) — blocks V2-β PUBLIC materials NOT the v2.0.0-beta.1 tag itself per DECISION-COUNSEL-1
```

## 3. File-area conflict matrix

### Phase 1 (Parallel Batch 1)

| Welle | Touches | Conflicts with |
|---|---|---|
| W9 | `docs/OPERATOR-RUNBOOK-V2-ALPHA-1.md` (NEW) | None |
| W10 | `docs/ADR/ADR-Atlas-007-*.md` (NEW) | None |
| W11 | `.github/workflows/wasm-publish.yml` + `docs/ADR/ADR-Atlas-008-*.md` | None |

**Anti-divergence enforcement:** None of W9/W10/W11 touch `CHANGELOG.md`, `docs/V2-MASTER-PLAN.md`, `.handoff/decisions.md`, `docs/SEMVER-AUDIT-V1.0.md`, or `.handoff/v2-session-handoff.md` — parent's consolidation commit does these.

### Phase 4 (Parallel Batch 2)

| Welle | Touches | Conflicts with |
|---|---|---|
| W12 | `apps/atlas-web/src/app/api/atlas/{entities,related,timeline,query,audit,passport}/route.ts` (NEW per route) | None at this level |
| W13 | `apps/atlas-mcp-server/src/tools/{query-graph,query-entities,query-provenance,get-agent-passport,get-timeline}.ts` (NEW) + `src/tools/index.ts` (modify to register) + `src/tools/types.ts` (extend) | Potentially `index.ts` shape conflicts — but only one welle touches it |
| W14 | `crates/atlas-projector/src/upsert.rs` (extend) + new tests | None at this level (welles 12+13 don't touch Rust) |

**Subtle Phase-4 risk:** W13 touches `apps/atlas-mcp-server/src/tools/index.ts` (registration site). W12 does NOT touch index.ts. Zero conflict between W12/W13/W14 at file level.

### Phase 5+ (Serial)

W15 through W19 are serial — each welle is one PR at a time. No parallel-conflict analysis needed.

## 4. ADR-number assignment (pre-allocated to prevent number-races)

| ADR # | Welle | Topic |
|---|---|---|
| 007 | W10 | Parallel-projection design |
| 008 | W11 | wasm-publish.yml race postmortem |
| 009 | W15 | Cypher-validator consolidation rationale |
| 010 | W16 | ArcadeDB backend choice + embedded-mode trade-off |
| 011 | W17a | ArcadeDB driver scaffold + trait design |
| 012 | W18 Phase A | Mem0g Layer-3 cache design (SHIPPED 2026-05-15) |
| 013 | W18c (reserved) | Mem0g implementation amendments — Nelson supply-chain constant lift ✓ SHIPPED (Phase A PR #100) + fastembed `try_new_from_user_defined` wiring ✓ SHIPPED (Phase B PR #105) + V1-V4 verification (Phase C pending) + LanceDB body fill-in (Phase D pending) |
| 014-017 | (reserved for V2-γ/V2-δ) | future |

Existing ADR high-watermark: `ADR-Atlas-006-multi-issuer-sigstore-tracking.md`. V2-β starts at 007.

## 5. Critical-path analysis

**Longest serial path:** Phase 0 → W14 → W16 → W17a → W17b → W17c → W18 → W18b → W18c-A → W19 → Phase 14.6 → W18c-B → Phase 14.7. **13 nodes ALL SHIPPED as of 2026-05-15.** v2.0.0-beta.1 LIVE end-to-end + **Layer 3 embedder OPERATIONAL** (W18c Phase B SHIPPED — `AtlasEmbedder::new` returns `Ok(_)` against real bge-small-en-v1.5 model dir; `embed()` returns 384-dim FP32 vector verified end-to-end via live mem0g-smoke CI). Counsel-Gate runs parallel-track Nelson-led (blocks V2-β PUBLIC materials, NOT the tag itself per `DECISION-COUNSEL-1`). Post-Phase-B-SHIPPED engineering-pipeline: W18c Phase C OR W18c Phase D (~2-3 sessions agent-only; **Phase D activates `/api/atlas/semantic-search` real hits** replacing 501 stub).

**Theoretical wall-clock (Atlas's 1-welle-per-session cadence):**

| Path | Sessions (sequential) | Sessions (with parallel) |
|---|---|---|
| Phase 0 | 1 | 1 |
| Phase 1 (W9+W10+W11) | 3 | 1 (3 parallel subagents) |
| Phase 2 (consolidation) | 1 | 1 |
| Phase 3 (alpha.2 ship) | 1 | 1 |
| Phase 4 (W12+W13+W14) | 3 | 1 (3 parallel subagents) |
| Phase 5 (W15 consolidation) | 1 | 1 |
| Phase 6 (W16 spike) | 1 | 1 |
| Phase 7 (W17a + W17b + W17c) | 3 | 3 (serial, can't parallelise) |
| Phase 8 (W18 Mem0g) | 1 | 1 |
| Phase 9 (W19 beta.1 ship) | 1 | 1 |
| **Total** | **16 sessions** | **12 sessions** |

Parallel dispatch saves ~25% wall-clock. Counsel-track runs in parallel with engineering but gates the final ship.

## 6. Rollback / re-plan triggers

If any of these conditions arise mid-execution, halt parallel dispatch and re-plan Phase 0 deliverables:

- **Per-welle byte-determinism CI pin breaks** after merge → blocked downstream work; root-cause first
- **Cross-welle consistency-reviewer surfaces a CRITICAL after parallel-batch ends** → consolidation commit refused; re-dispatch affected welle
- **ArcadeDB spike (W16) recommends NOT proceeding** → V2-β scope re-shaped; W17 becomes "alternative DB backend" or remains in-memory-only for beta.1
- **Counsel sign-off blocks at any V2-β-α.2 or beta.1 ship gate** → parallel-track Nelson-led work; engineering-pipeline pauses on the affected scope

---

## What this graph deliberately does NOT include

- **V2-γ welle dependencies** (Agent Passports, Regulator-Witness Federation, Hermes-skill v1) — separate planning phase post-V2-β
- **V2-δ welle dependencies** (Cedar policy at write-time, post-quantum hybrid co-sign) — V2-δ planning phase
- **Implementation-detail inter-file dependencies within a welle** — those are scoped in each welle's own plan-doc per the template in `.handoff/v2-beta-welle-N-plan.md.template`
- **Counsel-track sub-tasks** — Nelson-led parallel work; not engineering-pipeline dispatchable
