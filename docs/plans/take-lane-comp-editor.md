---
id: "PLAN-iVpbmseH193L48I6MLSzF8"
type: "plan"
status: "approved"
title: "Take Lane / Comp Editor — UI 詳細契約 文書チェーン・検証計画（synvelis#12 §14/§15 後続 slice — docs-only）"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/12"
  implements:
    - "docs/designs/take-lane-comp-editor-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/take-lane-comp-editor.md"
    - "docs/designs/take-lane-comp-editor-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/take-lane-comp-editor-task.md"
    - "docs/test-specs/take-lane-comp-editor.md"
    - "docs/impl/ir/take-lane-comp-editor.md"
    - "docs/impl/exp/take-lane-comp-editor.md"
    - "evidence/sv12/ui-contract-ledger.md"
    - "evidence/sv12/owner-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/take-lane-comp-editor-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/take-lane-comp-editor.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/12"
  defers: []
  deferred-by: []
---

# Take Lane / Comp Editor — UI 詳細契約 文書チェーン・検証計画（synvelis#12 §14/§15 後続 slice — docs-only）

## 目的

Issue #12 §14/§15 で合意済みの Take Lane / Comp Editor / Editor 統合 UX を
canonical UI 契約へ固定する docs-only slice の実行計画。spec → design →
plan → task → test-spec → impl/ir + impl/exp の文書チェーンと
`evidence/sv12/ui-contract-ledger.md` を作成し、触れた dir の管理 index を
再生成する。実装・実 GUI・数値プリセットは対象外。

## スコープ

### 対象（編集可）

新規ファイルのみ（厳密ファイル名）:

- `docs/specs/take-lane-comp-editor.md`
- `docs/designs/take-lane-comp-editor-design.md`
- `docs/plans/take-lane-comp-editor.md`（本書）
- `docs/tasks/take-lane-comp-editor-task.md`
- `docs/test-specs/take-lane-comp-editor.md`
- `docs/impl/ir/take-lane-comp-editor.md`
- `docs/impl/exp/take-lane-comp-editor.md`
- `evidence/sv12/ui-contract-ledger.md`

加えて、触れた dir の再生成管理 index（`docs/specs`・`docs/designs`・
`docs/plans`・`docs/tasks`・`docs/test-specs`・`docs/impl/ir`・
`docs/impl/exp` の `README.md`。`doc-maintenance index apply` /
`sync_impl_index` safe sync API 経由。手編集禁止）。

### 対象外（触らない）

- `crates/**` 全体（Rust コード・テスト・fixture の変更なし）
- `Cargo.toml` / `Cargo.lock` / `mise.toml` / `.github/workflows/` /
  `POLICIES.md` / `AGENTS.md` / `.devin/`
- `evidence/sv12/owner-matrix.md`（READ-ONLY 既存台帳）
- 他 slug の docs（初期 #12 chain・sv-gui-03-06 chain・ed-a/ed-b・
  ws-a・mda・rcd-04 等 — 全て READ-ONLY の契約入力）
- 数値 PoC 領域（#11 §9）・実 GUI 実装（#112/#114）・Creative Branch
  候補（#278/#279/#288）・production crates

## File Map

| Path | Responsibility | Notes |
| --- | --- | --- |
| `docs/specs/take-lane-comp-editor.md` | What/Why・REQ-TLC-01〜07・TL-AC-01〜12・Non-goals・Sources | Issue §14/§15 転記 + upstream depends-on |
| `docs/designs/take-lane-comp-editor-design.md` | 契約 C1〜C7・所有境界表・Editor Binding・HitTest/Tool matrix・Scenario/Oracle・Decision Log・Verification | upstream 正本への依存を明示し第二正本を作らない |
| `docs/plans/take-lane-comp-editor.md` | 本書。scope・file map・dependencies・risks・verification matrix・handoff・rollback | docs-only |
| `docs/tasks/take-lane-comp-editor-task.md` | T1〜Tn task 分解・完了条件・Verification（機械検査可能コマンド） | task-doc gate 準拠 |
| `docs/test-specs/take-lane-comp-editor.md` | F-LANE/F-SELECT/F-AUDITION/F-GESTURE/F-HANDLE/F-HITTEST/F-TOOL/F-EDITOR/F-FADE の Feature/Rule/Example/Guarantee | verifies spec+design |
| `docs/impl/ir/take-lane-comp-editor.md` | 実施記録（changes・decisions・validation） | task 開始時に作成・完了時に更新 |
| `docs/impl/exp/take-lane-comp-editor.md` | graph route・ID 生成・判断記録 | narrative 記録 |
| `evidence/sv12/ui-contract-ledger.md` | owner-matrix AC-28/29/30/31/32/36/38/39/41/47 ↔ test-spec case 対応表 | 「契約 fixed / 実機 Deferred」を区別 |
| `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/README.md` | 触れた dir の管理 index | 生成物。手編集禁止 |

## Dependencies

- upstream canonical docs（spec/design の depends-on に列挙）— 全て
  merge 済み・READ-ONLY の契約入力。
- `doc-driven-dev-graph` route は pre-existing broken relation により
  `blocked`（slice 起因でない）。sv#12/sv#120 先例と同型に、router の
  delegate 無しで文書チェーンを規約どおり作成し exp に記録する。
- 実行順序: spec → design → plan → task → test-spec → ir/exp →
  evidence ledger → index 再生成 → 検証 → review-protocol → commit/PR。

## Risks

| # | Risk | Likelihood | Impact | Mitigation |
| --- | --- | --- | --- | --- |
| 1 | upstream 契約との矛盾・行番号/AC 番号の実在しない引用 | med | med | depends-on 参照・引用は実在ファイル/実測 AC のみ。矛盾発見時は STOP して `orca orchestration ask` |
| 2 | 数値プリセット・OS 既定を誤って固定してしまう | med | med | design で明示的に deferred と記し、spec Non-goals に列挙 |
| 3 | owner-matrix AC と test-spec case の対応付け漏れ | med | med | ledger で AC-28/29/30/31/32/36/38/39/41/47 の 10 件を全件対応付け、件数照合 |
| 4 | pre-existing broken relation で graph route blocked | high（既知） | low | slice 起因でないことを exp/PR に記録。無関係な broken relation は修復しない（範囲外） |
| 5 | 契約 Pass を実機 Pass と誤記 | low | high | 状態語彙を `Contract fixed (design)`/`Deferred (#114/EXT-L3)` で分離。POLICIES 準拠 |

## Verification Matrix

| Step | Verification command or criteria | Pass condition |
| --- | --- | --- |
| 文書作成 | 8 新規ファイルが存在し front matter が lint を通る | `doc_status.js lint --type <type>` で新規分の error なし |
| index 再生成 | `doc_maintenance.js index plan` で全 touched dir in-sync | `index apply` 後 rescan で drift なし |
| impl index | `sync_impl_index.js --kind ir|exp` preview→apply | `docs/impl/{ir,exp}/README.md` が in-sync |
| fmt/pre-commit | `mise run fmt-check`、`mise exec -- prek run --all-files` | exit 0 |
| doc audit | `doc_status.js audit --type all --profile artifact` | 新規 broken relation なし（pre-existing は範囲外） |
| impl audit | `audit_impl_record.js`・`audit_experiment_log.js` | 新規分の error なし |
| ledger 整合 | ui-contract-ledger の AC↔case ID が test-spec に実在 | 全 10 AC が対応済み |
| review | `review-protocol` 1 cycle 以上 | blocker 0 |

## Downstream Handoff

- 実 GUI 実装 Owner（#112/#114 系）が本 chain の design 契約 C1〜C7・
  所有境界表・hit-test/tool matrix を消費して実装する。
- `evidence/sv12/ui-contract-ledger.md` が owner-matrix の UI 関連 AC の
  状態追跡を継続する。実機受入は #114/EXT-L3。
- 数値プリセット・OS 既定・MIDI 詳細・Creative Branch は後続 slice/Issue
  へ委譲（design §15）。

## Rollback

- docs-only slice のため、問題発生時は当該コミットの revert で完全に
  戻せる。生成 index は再生成で復元可能。外部状態・DB・公開 API への
  影響なし。不可逆 step なし。
