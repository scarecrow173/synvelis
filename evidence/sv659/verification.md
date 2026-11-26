# sv659 — verification redesign slice1 verification record

Issue `scarecrow173/synvelis#659` slice 1（ledger/matrix 設計）の検証記録。本 slice は製品コード、proof harness、Property Test、runner、mise lane、CI/workflow を変更していない。したがってここに記録する `test-formal` の pass は既存検証の非破壊的な回帰確認であり、#659 の representative proof 完了や新規 Authorization/Recovery 検証の pass ではない。

- Date: 2026-11-26
- OS: Windows x64（Orca worktree）。Kani は本 slice では実走しない
- Branch: `design/659-verification-design`
- Base/HEAD at start: `b9c331fd03bce0826d062d333cd70c8da57daa83`（`origin/main` と一致）
- ローカル検証 = 変更範囲の検証。統合検証は merge 後の対象 SHA に対する必須 CI 結果が証拠であり、本記録はそれを代替しない。

## 参照した正本

- `POLICIES.md` §6.1〜§6.6 — assurance kind、対象、同期状態、失敗原因、実行 verdict を独立軸として扱う規範。
- `evidence/rf1014/verification.md` — 上記ポリシーを反映した証跡。runner/harness の再設計は行っていない記録。
- `docs/specs/formal-verification.md` — Issue #601 の lane、bound、tool-fit、mutation 手順。
- `evidence/sv601/verification.md` / `runs.jsonl` — 2 pass・5 not_run の Kani verdict、OOM/WSL2 VM crash、mutation 証跡。
- `crates/synvelis-gateway/tests/formal_journal/{main,wire,policy,strategies}.rs`。
- `crates/synvelis-recording/tests/formal_journal.rs`。
- `tools/run-formal-kani.mjs` と `.agents/skills/verify-synvelis/SKILL.md`。

## inventory カバー範囲

inventory の正本は `docs/designs/issue-659-verification-redesign.md` §5、機械可読な補助は `inventory.json` である。対象 4 ファイルの test/proof 関数名を抽出し、matrix 上の harness/test 名がすべて実在することを確認した。

| 対象 | 件数 |
| --- | ---: |
| gateway journal Property/Example Test | 14 |
| gateway wire Property Test | 9 |
| gateway policy Property Test | 12 |
| recording journal/CRC32/session Property/Example Test | 15 |
| Property/Example Test 合計 | 50 |
| 既存 Kani proof | 7 |
| matrix rows 合計 | 57 |

確認結果: `inventory-check.txt` で `matrix_rows: 57`、`missing_names: 0`、`exit_equivalent: 0`。

## 設計判断の要約

1. Journal/CRC32/session の代表的重要 Property は `K-RJ-01`、`K-RJ-02`、`K-RJ-03`、`K-RJ-04`、`K-GJ-01` とする。`K-GJ-02`/`K-GJ-03` は pass 済みの回帰 proof として維持する。
2. 固定長 payload、固定 kind、`≤44 byte` stream、Chunk 除外、exact/over cap、session 4×4 全域を独立した保証制約として扱う。局所 proof の成功を任意長・永続化 recovery・製品全体へ転用しない。
3. Authorization は WireRequest 解釈、bind/Operation 解釈、純粋認可判定、外部 store/oracle/clock の 4 層に分離する。serde 結合部は Property Test、pure scan/decision core は次 slice の bounded proof 候補とする。
4. Recovery/操作順序は crash point をまたぐため TLA+ または同等の ordering model が必要と判定した。model 本体と atomic action の仮定は次 slice で確定する。
5. 失敗原因は `environment_absent`、`path_mapping`、`compile`、`symex`、`solver`、`memory`、`parallelism`、`infrastructure`、`product_oracle_failure` に分類する設計とした。runner 実装は変更していない。
6. Ruviform 還元は製品非依存の runner/evidence schema に限定される見込みだが、`rf#1015` との重複範囲が未確定なため本 slice では起票しない。次 slice で差分確定後に判断する。

## 実行した検証

| Command | OS | Exit | Verdict | 証跡/備考 |
| --- | --- | ---: | --- | --- |
| inventory name/count check（Python、補助） | Windows x64 | 0 | pass | `inventory-check.txt`。57/57 の matrix 名が対象 source に実在 |
| `mise run test-formal` | Windows x64 | 0 | pass | `test-formal.log`。gateway 35/35、recording 15/15、計 50/50 pass。既存 lane の回帰確認 |
| `mise run fmt-check` | Windows x64 | 1 | fail（環境/ツール起因） | `fmt-check.log`。`cargo fmt --all -- --check` が `os error 206`（ファイル名または拡張子が長すぎます）で起動失敗。Windows aggregate fmt の既知パターンであり、本 slice は Rust source を変更していない |
| `mise exec -- prek run --all-files` | Windows x64 | 0 | pass | `prek.log`。whitespace/EOF/YAML/TOML/line-ending/BOM/key/actionlint hooks pass |
| per-package `cargo fmt -p <package> -- --check`（補助証拠） | Windows x64 | 0 | pass | `fmt-check-aux.log`。workspace 15 package。canonical は失敗した `mise run fmt-check` であり、本行は代替ではなく補助 |
| `mise run test-formal-kani` / TLA+ | — | — | unsupported（本 slice scope外） | 実走しない。sv601 時点の verdict は 2 pass・5 not_run を引き継ぐが、本 slice で新たな証明証拠はない |

## 失敗・未実施の正直な状態

- `fmt-check` は exit 1。原因は aggregate `cargo fmt --all` の Windows コマンド/パス長制限を示す `os error 206` であり、rustfmt 差分が検出された失敗ではない。製品コードも Rust test も変更していないため、本 slice 起因の formatting regression ではないと切り分ける。per-package fmt は補助証拠として 15/15 package pass（`fmt-check-aux.log`）。ただし canonical verdict は引き続き exit 1 の Fail である。
- Kani/TLA+ は slice scope 外で未実施。したがって #659 の Required proof はまだ完了していない。
- Authorization/Recovery の matrix は設計であり、pure core 抽出、TLA+ model、mutation 実験は未実装である。
- docs index（`docs/**/README.md`）は生成物であり、共通契約に従って編集・再生成していない。統合時に Coordinator が整備する。

## 次 slice へ残す事項

- K-RJ-01/02/03/04・K-GJ-01 の選択実行と資源計測、既存 pass 2 件の回帰。
- runner の raw log/cause/resource schema、選択実行、KANI_JOBS 再実行設計の実装判断。
- Authorization pure scan/decision core の抽出可否、Required proof の実装、PTで十分とする最終理由。
- Recovery ordering model の atomic action、sync/crash 仮定、反例または証明証跡。
- mutation 実験と検出能力証跡。
- `rf#1015` と重複しない Ruviform 還元 Issue の要否確定。
