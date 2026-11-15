# Acceptance Ledger — synvelis#267 AUTX-08 headless common E2E slice

Date: 2026-11-15. Slice: AUTX-08 Leg A contract freeze
（`docs/test-specs/autx-08-common-e2e.md`）+ Leg B same-wire
3-surface contract test（`crates/synvelis-workspace/tests/
autx08_e2e.rs`）。`tests/rig/mod.rs` へ `compose()`/
`GatewayStack` を抽出し、`boot()` はその薄い wrapper とした —
3 surface が同一 compose recipe の独立 stack を得るため。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/267
（OPEN・`Refs` のみ・Close しない）。本 slice は #267 全体の
cross-gate 受入を完了しない — headless common contract の freeze
と 3-surface 等価証明のみ。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis SHA | `scarecrow173/sv267-autx08-e2e` HEAD（ledger は同 commit 内） |
| OS / CPU | Windows 11 / Intel Core i9-9900K |
| Toolchain | mise 管理 Rust（`mise run` / `mise exec -- cargo` 経由） |
| 検証 lane | OS-L1 headless のみ（GUI-L2/EXT-L3 は対象外） |

## 判定語彙

- **Pass (headless)**: `cargo test -p synvelis-workspace --test
  autx08_e2e` が green（実測は `verification.md`）。
- **NotRun**: 実 OS/実 GUI/実 provider/実 CLI binary/実 MCP
  transport/実 credential store 依存の判定 — 本 slice では
  実行していない。
- **Deferred**: 後続 lane（#267 受入・#114 GUI-L2・live adapter
  #264/#265）へ明示的に送った部分。
- **Unsupported**: 製品層が提供しない機能 — 本 slice では
  新規に発生しない。

## 受入条件 → 証拠対応

| AC | 判定 | 証拠 |
| --- | --- | --- |
| T-08-1: GUI/MCP/CLI/AI の同一 CapabilityRef→同一 Outcome/Error（AUTX-E2E-SAME-01） | Pass (headless・3 surface) / AI lane Deferred | `autx08_same_wire_lands_same_outcome_across_surfaces`（GUI↔CLI wire semantic field 一致 + 3 surface の `GatewayOutcome`/`structuredContent` 完全一致）、`autx08_unavailable_revision_fails_identically_across_surfaces`（同一 typed error + `ExternalError` 投影一致）、`autx08_agent_without_authority_fails_closed_gui_human_permits`（fail-closed + human/agent 境界）。**AI/automation lane は本 slice では未実施** — live `step_wire`（`synvelis-automation/src/live.rs`）は独立 wire producer であり、#264 接続後の別検証（下記 Deferred）。catalog lowering seam は GUI leg で代表済みだが AI caller identity/attestation は未行使。 |
| T-08-2: 独立 Policy/Commit oracle（AUTX-E2E-ORACLE-01） | Pass (headless) | 全 leg が `compose(None)` の独立 stack（独自 `InMemoryPolicyStore`/`CommitGate`+journal/`DirectoryBundleStore`/`ProposalStore`/ledger）を使用 — 共有 mutable authority state なしで同一 verdict。`autx08_idempotent_replay_names_same_operation` で per-instance ledger の `DuplicateRequest.original` も一致。 |
| T-08-3: 実 OS 証拠の Required/Deferred/NotRun 分離記録 | Pass (headless) | `case-definitions.md` §Evidence schema が lane を分離記録、`verification.md` が実測値のみを記録、`acceptance-ledger.md` 本表が NotRun/Deferred を明示。本 slice は headless 側のみを Pass とし、実 OS 行を NotRun のまま残す。 |

## NotRun / Deferred 一覧

- 実 Windows GUI-L2（実 GPUI window・実 OS 入力・実
  windowing/session lifetime）: **NotRun** — #114/#267 実 OS
  受入 lane。headless Pass を実機 Pass に昇格しない
  （POLICIES.md §2-3、`docs/test-specs/autx-08-common-e2e.md`
  Non-goals）。
- EXT-L3（実 provider/plugin・実 OS credential store・実 MCP
  transport process・実 `synvelis` CLI binary）: **NotRun** —
  `HeadlessSessionHost`/`McpHost`/`LocalCliAdapter` は in-process
  の trusted composition であり、実 transport・実 binary の
  E2E は別 lane。
- `synvelis-automation` live adapter からの catalog 実供給
  （#264）・workflow coordinator live run（#265）・macro preset
  live 投入・live `step_wire` の 3-surface 等価: **Deferred** —
  本 slice は engine 登録 capability と合成 catalog document のみ。
  `step_wire` は独自の request-id・consistency 写像・
  idempotency 伝播を持つ独立 producer のため、GUI catalog
  lowering での等価は AI lane の代理にならない。
- `synvelis-cli` の `build_request`/`claim_for` wire 生成と
  GUI lowering の field 一致: **Deferred** —
  `synvelis-workspace` は `synvelis-cli` に依存せず、CLI leg は
  `HeadlessSessionHost` + rig wire で host/attestation を検証。
  binary envelope は `cli_e2e.rs` が担う。cross-producer の
  wire field 一致は別 crate での検証または共有 wire-builder
  抽出が必要。
- GUI `lower` → `Gateway::handle` の production dispatch hop:
  **Deferred** — `syn.autx.action` の lowered `requests` を
  gateway lane へ渡す consumer は #264 mount wiring で接続予定。
- commit/reconcile の multi-process・実 filesystem fault・実
  provider を含む durable E2E: **Deferred** — `CommitGate` は
  実 journal/store 上で動作するが、OS 障害注入は別 lane。
- MCP `proposal.apply` の idempotency 引数面・control channel
  human authority・全 capability 総当たり: **Deferred** —
  adapter contract（GW-08）と gateway contract test 群の責務。
- `*-msrv` / `*-full` / Feature Matrix / 3OS 横断 lane:
  **Deferred** — Daily Full CI 委譲。
