---
id: "TSPEC-sv659VerificationRedesign01"
type: "test-spec"
status: "draft"
title: "Issue #659 Journal・Authorization 形式的検証再設計 検証仕様"
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
      - type: "file"
        path: "tools/run-formal-kani.mjs"
      - type: "file"
        path: ".agents/skills/verify-synvelis/SKILL.md"
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

# Issue #659 Journal・Authorization 形式的検証再設計 検証仕様

## Purpose

本書は `docs/designs/issue-659-verification-redesign.md` で確定した ledger/matrix 設計を、slice 1 でどう検証したか、および slice 2 の selective representative proof をどう判定したかを定める。slice 1 は設計と inventory のみであり、新しい proof、Property Test、TLA+ model、mutation 実験はなかった。slice 2 は runner interface と `K-RJ-03`/`K-RJ-04` の実走に限定し、Authorization、Recovery ordering、mutation、重い replay/round-trip proof は対象外である。

## 検証対象のトレース

| 設計要素 | 設計正本 | slice 1 の検証 | 後続 slice の検証 |
| --- | --- | --- | --- |
| inventory 全件性 | design §5、`evidence/sv659/inventory.json` | 対象4ファイルの test/proof 名から 50+7 件へ数え上げ、JSON count を 14/9/12/15/7 と照合 | 新規 proof/test 追加時に同じ ID 規則で更新し、欠番・重複を機械検査する |
| Journal codec/CRC32/session | design §6 | 代表 proof と bound を文書で確認。実行はしない | K-RJ-01/02・K-GJ-01 の選択実行、K-RJ-04 solver cost の原因調査、wall-clock/memory/cause 記録、既存 pass 2 件の回帰 |
| Authorization 分離 | design §7 | 4層分離と Required/Deferred/PT 分類を文書で確認。実装しない | pure scan/decision core の抽出可否、抽出後の BE、PT oracle の mutation 検証 |
| Recovery/ordering | design §8 | crash point/ordering inventory の有無を確認。model 実装しない | atomic action と sync 仮定を固定し、TLA+ 等の最小 model と反例/証明証跡を作る |
| 失敗分類/再実行 | design §9 | runner 出力語彙と矛盾しない分類があることを確認。slice 1 では runner 変更なし | raw log、cause class、KANI_JOBS、選択実行コマンドを evidence schema へ落とし、peak memory 計測と full cause classifier を実装する |
| Ruviform 還元 | design §10 | 起票しない判断と `rf#1015` 確認方針を記録 | runner差分が確定したら重複確認のうえ起票/相互リンク |

## slice 1 の検証 matrix

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
- mutation は slice 1/2 で実施しない。後続 slice の mutation 計画は、CRC除去、seq検査除去、wire forbidden token削除、descriptor先攻順序、budget消費条件を含み、各 mutation が期待どおり Fail し、revert 後に緑へ戻るまでを証跡とする。
- `mise run test-formal` の Pass は既存 Property lane の回帰証拠であり、#659 で要求する representative proof の完了証拠ではない。

## Fail-closed vocabulary

- `pass`: コマンドまたは文書照合が実際に実行され、期待を満たした。
- `fail`: 実行され、期待に反した。timeout/OOM/環境不足は product failure と区別する。
- `not_run`: 実行していない、または環境不足で検証に入れなかった。exit 0 でも pass にしない。
- `unsupported`: slice の scope 上、実行対象ではない。slice 1 の Kani/TLA+ 実走はこれに相当した。slice 2 の full lane と heavy replay/round-trip proof は scope 外の NotRun である。

## 後続 slice への必須トレース

後続 slice は少なくとも次の証跡を `evidence/sv659/` 配下へ追加する。各 proof について harness ID、sync SHA、環境、tool version、選択コマンド、`KANI_JOBS`、wall-clock、peak memory（取得可能なら）、exit code、runner verdict、cause class、反例有無、mutation結果、未証明領域を記録する。成功した proof が bound の内側だけを保証することを、PR と evidence の両方へ明記する。


## slice 2 selective representative proof の検証 matrix

slice 2 は `K-RJ-03` と `K-RJ-04` だけを実走する。full lane、全 7 proof 一括実行、`K-RJ-01`/`K-RJ-02`/`K-GJ-01` は対象外である。ここでの `Pass` は個々の検証が実行され期待どおり判定されたことを示し、Issue #659 全体の完了を示さない。

| ID | 対象 | 方法 | Expected | 結果 |
| --- | --- | --- | --- | --- |
| SV659-S2-R1 | selective interface | `KANI_PACKAGE=synvelis-recording KANI_HARNESS=<harness> KANI_JOBS=1 mise run test-formal-kani` | 1 package と 1 harness filter だけが cargo kani へ渡り、JSON verdict に lane/verdict/reason/package/packages/harness/jobs/environment/elapsed/exit code/harness 件数が出る | Pass。2 件の proof invocation と zero-match probe で確認 |
| SV659-S2-R2 | 設定エラー fail-closed | `KANI_PACKAGE=unknown mise exec -- node tools/run-formal-kani.mjs` | verification に入らず `configuration_error`・nonzero | Pass。exit 2、runner JSON verdict `fail` |
| SV659-S2-R3 | zero match fail-closed | `KANI_HARNESS=sv659_zero_match_probe` で selective lane | 0 harness を pass にしない。raw output から Kani の zero-match 挙動を引用できる | Pass。exit 1、`harnesses_matched=0`、raw output に `Please specify the fully-qualified name of a harness.` |
| SV659-S2-K3 | `K-RJ-03` CRC32 分割同値 | `KANI_PACKAGE=synvelis-recording KANI_HARNESS=crc32_split_equivalence_bounded KANI_JOBS=1 mise run test-formal-kani` | 固定 16 byte・split ≤16 で `VERIFICATION:- SUCCESSFUL` | Pass。0/131 failed（1 unreachable）、Verification Time 282.0517s、runner wall-clock 310.115s |
| SV659-S2-K4 | `K-RJ-04` session 4×4 exhaustive | `KANI_PACKAGE=synvelis-recording KANI_HARNESS=session_transitions_exhaustive KANI_JOBS=1 mise run test-formal-kani` | 300s proof budget 内に確定し、反例・unwinding assertion・resource 未完了を区別する | NotRun。solver が 300s を超えて確定せず、runner wall-clock 634.49s で外部停止。cause class `solver` |
| SV659-S2-N1 | 既存 Property lane 非回帰 | `mise run test-formal` | gateway 35/35・recording 15/15 | Pass。exit 0、50/50 pass |
| SV659-S2-N2 | 衛生レーン | `mise exec -- node --check tools/run-formal-kani.mjs`、`mise run fmt-check`、`mise exec -- prek run --all-files` | すべて exit 0 | Pass。いずれも exit 0 |
| SV659-S2-F1 | full lane | `KANI_HARNESS` なしの全 7 proof 一括実行 | slice 2 では実行しない | NotRun。メモリリスクと slice scope のため未実施 |

## slice 2 oracle detail

- `K-RJ-03` の oracle は製品実装 `crc32` と `Crc32::{new,update,finish}` の同値であり、test oracle の写しではない。Pass は固定 16 byte・任意 split ≤16 の Bounded Exhaustive に限る。16 byte 超、3 以上の分割、任意長の数学証明、暗号学的衝突耐性へ転用しない。
- `K-RJ-04` の oracle は `SessionState` 4×4 と acceptance gate の有限全域である。今回は solver 未完了なので、この oracle に対する保証は増えていない。将来 Pass しても、上位 recording workflow、crash recovery、永続化 ordering の証明ではない。
- `K-RJ-04` の exit 15 は外部停止を示す。`VERIFICATION:- FAILED` や反例がないため product_oracle_failure ではない。`format!` を含む illegal-transition error path が solver 入力を大きくしている可能性はあるが、完了ログがないため原因は未確定とする。
- runner JSON の `fail` は実行レーンの fail-closed 語彙である。外部停止や環境失敗を製品欠陥と混同しないよう、proof verdict と cause class は `evidence/sv659/formal-proof-runs.jsonl` で独立に記録する。
- peak memory は runner が RSS を計測しないため unmeasured とする。観測 sample は peak ではないので保証や比較に使わない。

## slice 2 結果の正本

machine-readable な invocation 証跡は `evidence/sv659/formal-proof-runs.jsonl`、raw output は `evidence/sv659/krj03-crc32-split-equivalence-bounded.txt`、`evidence/sv659/krj04-session-transitions-exhaustive.txt`、`evidence/sv659/kani-zero-match-probe.txt` とする。人間向けの要約は `evidence/sv659/verification.md` に記録する。
