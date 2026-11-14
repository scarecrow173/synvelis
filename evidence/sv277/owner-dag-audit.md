# SV-277 Owner / DAG Audit — Issue #277 CB-AC-07

既存 Synvelis/Ruviform Issue との Owner 重複・DAG 循環がないことの
設計時監査。文書・Issue タイトルの照合に基づく（実装状態の監査ではない）。

## Synvelis 側

| Issue | Owner 領域（docs/specs・designs との対応） | Creative Branch との関係 | 判定 |
| --- | --- | --- | --- |
| #10 | Project Model（Track/Lane/Clip/Source/typed ID/非破壊）— `project-model.md` PM-R-* | VariantScope が指す typed stable ID・PM-R-08/09 の scope 系要件を継承 | 重複なし。CB depends-on |
| #12 | Recording/Take/Comp（Raw/Take/Comp/Recovery/Journal）— `recording-take-comping.md`・`vs-*` | Raw Source 非破壊・参照非所有を継承。Take/Comp 候補 adapter は CB-02(#279) | 重複なし。#12=raw 所有、CB=候補参照のみ |
| #17 | Project/Persistence（Snapshot/Save/Recovery/versioned migration）— `project-persistence.md` | `SnapshotIdentity`・versioned reader・head CAS を再利用 | 重複なし。depends-on |
| #7 | VCS（read-only Snapshot/Semantic Diff/Apply）— `vcs-*.md` | Semantic Diff engine を Compare 層で再利用。CB≠VCS branch は `vcs-integration.md` で分離済み | 重複なし。VCS Off で CB 動作 |
| #104 | SF-01 Semantic Routing Model | ProcessingRange の semantic 境界 ID（ProcessorId/SemanticPortId）を再利用 | 重複なし。depends-on |
| #105 | SF-02 Routing Commands/Editing | routing_tx preflight→plan→commit パターンを Adopt preflight が踏襲 | 重複なし。depends-on |
| #106 | SF-03 Semantic→Runtime lowering/activation | Plan Publish/Active・last_good・generation 契約を Adopt/Runtime Receipt 分離が再利用 | 重複なし。depends-on |
| #121 | CTL-01 Control owner/ParameterTarget | Instrument/TrackBus scope の Control/Automation 参照を typed ID で指す | 重複なし。depends-on |
| #122 | CTL-02 Plugin binding/Routing atomic commit | Plugin instance/state 独立・atomic commit 契約を継承 | 重複なし。depends-on |
| #128 | TIM-04 Plugin State snapshot consistency | `StateCaptureQuality`/`PluginStateFailureKind`/Receipt 語彙を Missing Plugin 判定に再利用 | 重複なし。depends-on |
| #114/#175/#176 | GUI/GPUI・Plugin Dock・製品 E2E | Difference Explorer は CB-09(#286) の GPUI scenario、実機受入は #114/#176 系 Gate | 重複なし。実機は別 Gate |
| #23 | Execution Plan（横断 DAG/Gate） | SV-CB node/edge は `execution-plan-design.md` に既存。本設計はその正本化 | 重複なし。depends-on |
| #278〜#288 | CB-01〜CB-11 実装 Issue（本 Issue が正本） | design §9.2 の Start/Integration/Acceptance 表どおり分解 | 重複なし（新規分解の所有者は本 Issue） |
| #289 | RCP-00 render checkpoint contract | §6/§7.4 で cutset 検証・Compare 専用 vs Verified Artifact 区別を接続 | 重複なし。references |
| #294 | RCP-05 Branch/VCS・cross-consumer/RT integration | cross-branch CacheHit・closure 検証・consumer 証拠集約の接続先 | 重複なし。references |

## Ruviform 側（別リポジトリ責務）

| Issue | Owner 領域 | Creative Branch との関係 | 判定 |
| --- | --- | --- | --- |
| ruviform#308 | Generic probe 再 Binding | Compare の Aligned/Approximate 判定が利用する整列契約 | 重複なし。再実装禁止 |
| ruviform#309 | Generic 時間整列/alignment | pinned_context の alignment 判定に利用 | 重複なし。再実装禁止 |
| ruviform#294 | consumer-verify 拡張 | RCP/Consumer 横断証拠集約先 | 重複なし。references |
| ruviform#562 | Generic Alternate Evaluation Context PoC | Signal/Perceptual 層の Generic capability gap 判定先 | 重複なし。Gap のみ upstream Issue |

## DAG 循環監査

- CB-00(#277) →start CB-01(#278) →start {CB-02〜07(#279〜284),
  CB-08(#285), CB-09(#286 mock), CB-10(#287), CB-11(#288 fixture)}
  →acc CB-GATE(#288)。`execution-plan-design.md` の既存 edge と一致。
- RCP 系: `RF#562/#196/#198 + SV-SF-03(#106)/SV-TIM-04(#128)/
  SV-CB-00(#277)/SV-MEDIA →int SV-RCP-01〜06`。#277 は integration
  の入力であり、#294 側から #277 への逆依存 edge は存在しない。
- 本設計は新規 edge を追加しない（文書化のみ）。循環なし。

## 判定

- Owner 重複: なし。Synvelis=Branch 意味論/Scope/Controller/
  Lineage/Compare orchestration/UX/Gateway、Ruviform=Generic
  Time/Graph/Plan/Render/Probe/Driver-neutral UI、AudioLens=
  f64/長尺 Consumer（独立 Gate）。
- DAG 循環: なし。
- AudioLens Consumer 非回帰・通常 DAW Gate A 独立・VCS/AI OFF は
  design §9.2 の各 Issue Acceptance/Start 条件に明記済み。
