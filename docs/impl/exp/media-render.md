---
id: "EXP-mr20261114mediaRenderD"
type: "experiment"
status: "completed"
title: "Media & Render Lifecycle — Issue #21 設計契約文書化と MR 写像の試行"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/21"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/media-render.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/media-render-design.md"
    - "docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md"
    - "docs/designs/synvelis-driven-ruviform-target-architecture-v07.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/media-render-task.md"
    - "docs/test-specs/media-render.md"
  defers: []
  deferred-by: []
---

# Media & Render Lifecycle — Issue #21 設計契約文書化と MR 写像の試行

## 問題設定

Issue #21 の Media / Import / Export / Offline Render / UI /
Diagnostics v0.6 設計契約を canonical 文書鎖として確立する
必要があった。上流 baseline（v0.6 architecture・14 項目合意・
MR-001〜028 matrix）と v0.7.1 target architecture、および
#24/#25/#26（L1/L2/X）の headless 実装済み契約は存在するが、
Issue #21 が要求する 29 受入ケース（MR-RCP-01・MR-001〜028）・
MR-R-01〜19 要件・MR-AC-01〜10・Ruviform/Synvelis Owner 境界・
evidence 語彙を一つの正本 + 台帳として固定する記述がなかった。
本 slice は docs のみで crate を変更しないため、(a) 7 文書鎖の
生成、(b) MR→既存 test/oracle/Issue/状態の写像、(c) doc
graph/index/review の整合確認が試行対象となった。

## 試行と結果

### E1: 上流契約と実装の照合

`docs/designs/media-import-export-offline-render-ui-diagnostics-
architecture.md`（v0.6 Approved）と `synvelis-driven-ruviform-
target-architecture-v07.md`（v0.7.1）を正本として読み、
MR-001〜028・MR-RCP-01 の各ケースを既存 fixture へ写像した。

- L1/L2/X（#24/#25/#26・closed）の headless fixture が
  MR-001〜004・MR-011〜020 の大部分を被覆することを確認。
- asset-library 系（#9/#85〜#91）が MR-005〜010・MR-028 を
  被覆することを確認（実 server は NotRun）。
- tim03/tim04（#127/#128）が MR-021 の plugin precision/state
  を被覆することを確認（mock のみ・実 plugin NotRun）。
- sf-04 probe 系（#107 系）が MR-024〜027 を被覆することを
  確認（headless・実 device NotRun）。
- gateway `job.rs`・workspace `ws_a.rs`/`workspace_model.rs`
  の `close_veto`（sv-gui-04 契約）が MR-023 を部分
  被覆することを確認（unified orchestrator 未実装）。
- stem/bounce/freeze（MR-017/MR-022）と RCP（MR-RCP-01）は
  未実装であり `NotRun` として記録。

### E2: Owner 境界の固定

Ruviform generic（media-io Reader/Decoder・io-wav Encoder/
Muxer・exec offline driver・probe/dock/clock/ui-data・
StagedFile）と Synvelis DAW policy（Source/Asset/Project/
Import/RenderSpec/JobOrchestrator/Inspector/SignalFlow）の
責務を design の Owner 表に固定した。generic runtime の不足は
Synvelis 重複実装ではなく Ruviform Issue（rf#253/#275/#294/
#308/#309 等）への起票として記録する方針を確認。

### E3: Evidence honesty の分類規則

headless fixture の Pass を `Pass(部分)` として記録し、実機/
GUI/Plugin/OS 受入に繰り上げない規則を確立した。`Pass`/
`Fail`/`Unsupported`/`NotRun`/`Pass(部分)` の 5 status を使い
分け、未実装（stem/bounce/freeze/orchestrator/RCP）を `NotRun`、
実機未検証を `Pass(部分)`、設計レビュー Pass と実測 Pass を
区別する。POLICIES.md の OS-L1/GUI-L2/EXT-L3・Required/
Deferred/NotApplicable と整合させた。

### E4: 文書鎖生成と index/relation 整合

spec→design→plan→task→test-spec→impl-ir/exp の 7 文書鎖を
canonical front matter（id/type/status/title/created/updated/
owners/relations）で生成し、relations（implements/
derived-by/verifies/references/depends-on 等）を相互に整合
させた。`docs/{specs,designs,plans,tasks,test-specs,impl/ir,
impl/exp}/README.md` は `doc_maintenance.js index apply`・
`sync_impl_index.js` で再生成し、手編集しない。`evidence/sv21/`
の case ledger と verification を正本として配置した。

## 採用した判断

- **A-1 7 文書鎖 + evidence/sv21/**: Issue #18（playback-graph）
  の先例に従い canonical 配置。spec/design/plan/task/test-spec/
  impl-ir/impl-exp の 7 文書と case ledger・verification を
  `evidence/sv21/` に配置。
- **A-2 MR-R-01〜19 + MR-AC-01〜10**: v0.6 の 14 判断 + Issue
  #21 横断 AC を MR-R-01〜19 に整理し、受入条件を MR-AC-01〜10
  に対応付け。
- **A-3 Owner 境界表**: Ruviform generic / Synvelis DAW policy
  を責務別に固定し、generic 不足は Ruviform Issue へ起票。
- **A-4 status 語彙**: `Pass(部分)` を headless Pass に使用、
  未実装は `NotRun`、実機/GUI/Plugin/OS は別 status。
- **A-5 impl/exp canonical 名**: `docs/impl/ir/media-render.md`・
  `docs/impl/exp/media-render.md`（playback-graph 先例一致）。

## 結論

Issue #21 の設計契約は docs/ 7 文書鎖 + `evidence/sv21/` として
確立された。29 受入ケースのうち 27 ケースが headless fixture
で `Pass(部分)`、2 ケース（MR-017 stem/bounce・MR-022
bounce/freeze）と MR-RCP-01 が未実装で `NotRun`。設計完了は
実装完了・実機受入を意味せず、後続実装 Issue（#23 配下・
#9/#85〜#91・#127/#128・#289〜#297・実機 lane）が Owner として
残る。
