---
id: "IMPL-034X7v8PmJKzfbj9uOlPeF"
type: "impl"
status: "completed"
title: "Media Render — Issue #21 設計契約文書化 + MR evidence 台帳"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/21"
  changes:
    added:
      - type: "file"
        path: "docs/specs/media-render.md"
      - type: "file"
        path: "docs/designs/media-render-design.md"
      - type: "file"
        path: "docs/plans/media-render.md"
      - type: "file"
        path: "docs/tasks/media-render-task.md"
      - type: "file"
        path: "docs/test-specs/media-render.md"
      - type: "file"
        path: "docs/impl/ir/media-render.md"
      - type: "file"
        path: "docs/impl/exp/media-render.md"
      - type: "file"
        path: "evidence/sv21/media-render-case-ledger.md"
      - type: "file"
        path: "evidence/sv21/media-render-verification.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
  implements:
    - "docs/tasks/media-render-task.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/media-render-task.md"
    - "docs/plans/media-render.md"
    - "docs/designs/media-render-design.md"
    - "docs/specs/media-render.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md"
    - "docs/designs/synvelis-driven-ruviform-target-architecture-v07.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/tasks/media-render-task.md"
  derived-by: []
  verifies:
    - "docs/test-specs/media-render.md"
  verified-by: []
  references:
    - "docs/specs/media-render.md"
    - "docs/designs/media-render-design.md"
    - "docs/plans/media-render.md"
    - "docs/tasks/media-render-task.md"
    - "docs/test-specs/media-render.md"
    - "evidence/sv21/media-render-case-ledger.md"
    - "evidence/sv21/media-render-verification.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/media-render.md"
    rejected: []
---

# Media Render — Issue #21 設計契約文書化 + MR evidence 台帳

## Summary

Issue #21「Media / Import / Export / Offline Render / UI /
Diagnostics v0.6 — Media & Render Lifecycle」の canonical
設計契約を docs/ 7 文書鎖（spec/design/plan/task/test-spec/
impl-ir/impl-exp）+ `evidence/sv21/`（case ledger・
verification）として作成した。MR-RCP-01・MR-001〜028 の
29 受入ケースを既存 Synvelis/Ruviform fixture・Issue・
Receipt へ写像し、Ruviform generic / Synvelis DAW policy
の Owner 境界と evidence 語彙を固定した。**docs/ 設計契約
+ evidence のみ**であり、実装コード・GUI・実 device/plugin/
OS 受入は対象外。

## Context

- 対象 Issue: synvelis #21
- ブランチ: `docs/21-media-render`
- 上流正本: `docs/designs/media-import-export-offline-render-
  ui-diagnostics-architecture.md`（v0.6 Approved）・
  `docs/designs/synvelis-driven-ruviform-target-architecture-
  v07.md`（v0.7.1）
- 確定済み縦断契約: l1/l2/x/issue-86/alib/vs-c/rcd-02/
  recording-take-comping/tim03/tim-02/tim-04/playback-graph/
  project-persistence/workspace-model/sv-gui-04/sf-04/syn-obs-01/
  syn-rf305
- 横断 Issue: #127（TIM-03）・#128（TIM-04）・#116/#86・
  RCP-00〜F01（#289〜#297）

## Implementation

### Changes

- `docs/specs/media-render.md`: MR-R-01〜19 要件・MR-AC-01〜10
  acceptance criteria・Deferred Design Concerns・Sources。
- `docs/designs/media-render-design.md`: Owner 分離表・15
  component・data/control flow・media capability/resolver・
  import state machine・render contract・output pipeline・
  export vs project-derived・failure taxonomy・MR mapping・
  risks・deferred・verification notes。
- `docs/plans/media-render.md`: goal/scope/upstream/file map/
  5 implementation streams/dependency graph/risk register/
  verification plan/rollback/task breakdown。
- `docs/tasks/media-render-task.md`: work/classification/
  upstream/dependencies/affected files/done-when/verification/
  out-of-scope。
- `docs/test-specs/media-render.md`: MR-T-R-01〜06 rules・
  29 case map・3 examples・guarantees・non-goals・risk。
- `docs/impl/ir/media-render.md`: 本記録。
- `docs/impl/exp/media-render.md`: experiment record（E1〜E4）。
- `evidence/sv21/media-render-case-ledger.md`: 29 case の
  fixture・oracle・Owner・status・証拠の台帳。
- `evidence/sv21/media-render-verification.md`: 検証記録。
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/
  README.md`: index 再生成（生成物）。

### Decisions During Implementation

- **D-1 7 文書鎖 + evidence**: Issue #18（playback-graph）の
  先例に従い、spec→design→plan→task→test-spec→impl-ir/exp
  の 7 文書鎖と `evidence/sv21/` を canonical 配置。
- **D-2 MR-R-01〜19 要件番号**: v0.6 baseline の 14 判断 +
  Issue #21 の横断 AC（#127/#128・#116/#86・MR-RCP-01・
  evidence honesty）を MR-R-01〜19 に整理。
- **D-3 MR-AC-01〜10**: Issue #21 の受入条件を 10 項目に
  対応付け、case ledger で被覆状況を記録。
- **D-4 Owner 境界表**: Ruviform generic（media-io/io-wav/
  exec/probe/dock/ui-data）と Synvelis DAW policy（project/
  source/asset/import/render/job/inspector/signal-flow）を
  責務別に固定。generic 不足は Ruviform Issue へ起票。
- **D-5 status 語彙**: `Pass`/`Fail`/`Unsupported`/`NotRun`/
  `Pass(部分)` を使用。headless Pass は `Pass(部分)`、
  未実装は `NotRun`、実機/GUI/Plugin/OS は別 status。
- **D-6 impl/exp ファイル名**: `docs/impl/ir/media-render.md`・
  `docs/impl/exp/media-render.md` を canonical 名として使用
  （playback-graph 先例に一致）。
- **D-7 index 再生成**: `doc_maintenance.js index apply`・
  `sync_impl_index.js` で生成、手編集しない。

## Related Experiments

### Adopted

- `docs/impl/exp/media-render.md`（E1〜E4: 文書鎖構造・
  MR 写像・Owner 境界・evidence 語彙の設計判断）

### Rejected

- なし

## Validation

`evidence/sv21/media-render-verification.md` を正本とする。
実行した検証:

- `mise run fmt-check` — （結果は verification に記録）
- `doc_status.js lint --type {spec,design,plan,task,test-spec}`
- `doc_maintenance.js index apply`
- `audit_impl_record.js --json`・`audit_experiment_log.js --json`
- `audit_docs.js --type all --json`
- `route_graph.js --focus media-render --json`
- `review-protocol`

## Risks

- **R-IR-1 headless Pass の過剰主張**: `Pass(部分)` を
  `Pass` に繰り上げると実機受入と混同される。→ evidence
  語彙で区別、未実装は `NotRun`。
- **R-IR-2 Owner 境界の曖昧さ**: Ruviform generic と Synvelis
  policy の分界が実装時に崩れる。→ MR-R-14 表を API/Fixture/
  Issue で照合。
- **R-IR-3 fixture 陳腐化**: 参照する fixture 名が実装と
  乖離。→ case ledger に fixture path を明記、lint で検証。
- **R-IR-4 asset-library 並行衝突**: #9 系 Worker が
  `docs/specs/alib-*.md` 等を編集中。本 slice は
  参照のみで編集しない。

## Follow-ups

- stem/bounce/freeze/render-in-place（#23 配下・MR-017/022）
- unified job orchestrator（#114/#23・MR-023）
- RCP-01/02/03（#290〜#297・MR-RCP-01）
- 実 plugin/OS/GUI 受入（実機 lane・MR-021/024〜027）
- asset-library 実 provider/preview（#9/#85〜#91・MR-005〜010）
- SRC/dither 実装（#20・MR-018）
