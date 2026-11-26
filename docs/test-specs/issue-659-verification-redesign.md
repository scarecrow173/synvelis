---
id: "TSPEC-sv659VerificationRedesign01"
type: "test-spec"
status: "draft"
title: "Issue #659 Journal・Authorization 形式的検証再設計 slice1 検証仕様"
created: "2026-11-26"
updated: "2026-11-26"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/659"
  changes:
    added:
      - type: "file"
        path: "docs/designs/issue-659-verification-redesign.md"
      - type: "file"
        path: "docs/test-specs/issue-659-verification-redesign.md"
      - type: "dir"
        path: "evidence/sv659/"
    modified:
      - type: "file"
        path: "docs/specs/formal-verification.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/designs/issue-659-verification-redesign.md"
    - "docs/specs/formal-verification.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "evidence/sv659/verification.md"
    - "evidence/sv659/inventory.json"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-659-verification-redesign.md"
  derived-by: []
  verifies:
    - "docs/specs/formal-verification.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/601"
    - "https://github.com/scarecrow173/synvelis/issues/662"
  defers: []
  deferred-by: []
---

# Issue #659 Journal・Authorization 形式的検証再設計 slice1 検証仕様

## Purpose

本書は `docs/designs/issue-659-verification-redesign.md` で確定した ledger/matrix 設計を、本 slice でどう検証したか、および次 slice が実装する検証へどうトレースするかを定める。slice 1 は設計と inventory のみであり、新しい proof、Property Test、TLA+ model、mutation 実験は存在しない。したがって「実装済み」「証明済み」と書ける対象はなく、既存 `test-formal` の非破壊確認と文書整合性の検証に限定される。

## 検証対象のトレース

| 設計要素 | 設計正本 | 本 slice の検証 | 次 slice の検証 |
| --- | --- | --- | --- |
| inventory 全件性 | design §5、`evidence/sv659/inventory.json` | 対象4ファイルの test/proof 名から 50+7 件へ数え上げ、JSON count を 14/9/12/15/7 と照合 | 新規 proof/test 追加時に同じ ID 規則で更新し、欠番・重複を機械検査する |
| Journal codec/CRC32/session | design §6 | 代表 proof と bound を文書で確認。実行はしない | K-RJ-01/02/03/04・K-GJ-01 の選択実行、wall-clock/memory/cause 記録、既存 pass 2 件の回帰 |
| Authorization 分離 | design §7 | 4層分離と Required/Deferred/PT 分類を文書で確認。実装しない | pure scan/decision core の抽出可否、抽出後の BE、PT oracle の mutation 検証 |
| Recovery/ordering | design §8 | crash point/ordering inventory の有無を確認。model 実装しない | atomic action と sync 仮定を固定し、TLA+ 等の最小 model と反例/証明証跡を作る |
| 失敗分類/再実行 | design §9 | runner 出力語彙と矛盾しない分類があることを確認。runner変更なし | raw log、cause class、KANI_JOBS、選択実行コマンドを evidence schema へ落とす |
| Ruviform 還元 | design §10 | 起票しない判断と `rf#1015` 確認方針を記録 | runner差分が確定したら重複確認のうえ起票/相互リンク |

## 本 slice の検証 matrix

| ID | 対象 | 方法 | Expected | 判定語彙 |
| --- | --- | --- | --- | --- |
| SV659-D1 | inventory 件数 | `inventory.json` の count を確認 | gateway journal 14、wire 9、policy 12、recording 15、Kani 7、総計 57 | Pass/Fail。未実施なら NotRun |
| SV659-D2 | matrix 必須列 | design §5 の各行を確認 | Property ID、旧契約、現行検証、必要保証、risk、assurance、subject/境界、領域/assume/bound/除外/未証明、verdict/cost、新検証案が読み取れる | Pass/Fail |
| SV659-D3 | 制約引き継ぎ | design §6/§11 を確認 | 固定長、payload cap、span count、Chunk除外、局所証明転用禁止が明示される | Pass/Fail |
| SV659-D4 | Authorization分類 | design §7 を確認 | Required/Deferred/PT十分と理由・残留リスクが Property 単位で示される | Pass/Fail |
| SV659-D5 | Recovery判定 | design §8 を確認 | 状態、crash point、ordering property、TLA+ 等の必要性判定と不足情報が示される | Pass/Fail |
| SV659-D6 | 失敗分類 | design §9 を確認 | OOM/WSL/並列/compile/symex/solver/memory と再実行方法が分類される | Pass/Fail |
| SV659-V1 | 既存 Property lane | `mise run test-formal` | 現行 lane を非破壊で実行し、exit codeと失敗有無を記録する。既存/環境起因は切り分ける | Pass/Fail/NotRun |
| SV659-V2 | format lane | `mise run fmt-check` | exit 0。文書のみ変更でも正規 lane を記録する | Pass/Fail |
| SV659-V3 | hooks | `mise exec -- prek run --all-files` | exit 0。生成 docs index を触らない | Pass/Fail |
| SV659-V4 | Kani/TLA+ | 実行しない | slice scope 外。実行しないことを evidence に書く | NotRun/Unsupported |

## Oracle detail

- inventory の oracle は単なる件数一致ではない。各 ID が実在する test/proof 名に対応し、現行 verdict・cost・未証明領域を `evidence/sv601` と現行 harness の事実へ引けることが条件である。
- Authorization oracle は製品の forbidden key 表や deny reason 表をそのまま正本にしない。`wire.rs` が採用した test-local literal list と subset pin の独立性を維持する。
- mutation は本 slice で実施しない。次 slice の mutation 計画は、CRC除去、seq検査除去、wire forbidden token削除、descriptor先攻順序、budget消費条件を含み、各 mutation が期待どおり Fail し、revert 後に緑へ戻るまでを証跡とする。
- `mise run test-formal` の Pass は既存 Property lane の回帰証拠であり、#659 で要求する representative proof の完了証拠ではない。

## Fail-closed vocabulary

- `pass`: コマンドまたは文書照合が実際に実行され、期待を満たした。
- `fail`: 実行され、期待に反した。timeout/OOM/環境不足は product failure と区別する。
- `not_run`: 実行していない、または環境不足で検証に入れなかった。exit 0 でも pass にしない。
- `unsupported`: slice の scope 上、実行対象ではない。Kani/TLA+ 実走は本 slice ではこれに相当する。

## 次 slice への必須トレース

次 slice は少なくとも次の証跡を `evidence/sv659/` 配下へ追加する。各 proof について harness ID、sync SHA、環境、tool version、選択コマンド、`KANI_JOBS`、wall-clock、peak memory（取得可能なら）、exit code、runner verdict、cause class、反例有無、mutation結果、未証明領域を記録する。成功した proof が bound の内側だけを保証することを、PR と evidence の両方へ明記する。
