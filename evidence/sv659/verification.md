# sv659 — verification redesign verification record

Issue `scarecrow173/synvelis#659` の検証記録。slice 1 は ledger/matrix 設計、slice 2 は selective representative proof の最小 interface と `K-RJ-03`/`K-RJ-04` の実走を対象とする。ここに記録する pass は個々の実行レーンまたは bounded proof の結果であり、Issue #659 全体、製品全体、任意長ジャーナル、永続化 recovery の証明ではない。

- Date: 2026-11-26
- OS: Windows x64（Orca worktree）。Kani proof は WSL2 `Ubuntu` で実行
- ローカル検証は変更範囲の検証である。統合検証は merge 後の対象 SHA に対する必須 CI 結果が証拠であり、本記録はそれを代替しない

## slice 1 の識別情報

- Branch: `design/659-verification-design`
- Base/HEAD at start: `b9c331fd03bce0826d062d333cd70c8da57daa83`（当時の `origin/main` と一致）
- slice 1 は製品コード、proof harness、Property Test、runner、mise lane、CI/workflow を変更しなかった

## slice 2 の識別情報

- Branch: `test/659-formal-proof-impl`
- Base at branch creation (`origin/main`): `01c99ea213f5c98bfb307c35c5d98a389cfec34f`
- runner implementation SHA: `24d2e2a25291d4e8ec75c7f1ea41bad049421be5`
- slice 2 は `tools/run-formal-kani.mjs`、design/test-spec/spec/skill、`evidence/sv659/` だけを変更し、製品コード、proof harness、Property Test、bound、assume、unwind、`mise.toml` は変更しなかった

## 参照した正本

- `POLICIES.md` §6.1〜§6.6 — assurance kind、対象、同期状態、失敗原因、実行 verdict を独立軸として扱う規範
- `evidence/rf1014/verification.md` — 上記ポリシーを反映した証跡
- `docs/specs/formal-verification.md` — Issue #601 の lane、bound、tool-fit、mutation 手順
- `evidence/sv601/verification.md` / `runs.jsonl` — 2 pass・5 not_run の Kani verdict、OOM/WSL2 VM crash、mutation 証跡
- `crates/synvelis-gateway/tests/formal_journal/{main,wire,policy,strategies}.rs`
- `crates/synvelis-recording/tests/formal_journal.rs`
- `tools/run-formal-kani.mjs` と `.agents/skills/verify-synvelis/SKILL.md`

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
3. Authorization は WireRequest 解釈、bind/Operation 解釈、純粋認可判定、外部 store/oracle/clock の 4 層に分離する。serde 結合部は Property Test、pure scan/decision core は後続 slice の bounded proof 候補とする。
4. Recovery/操作順序は crash point をまたぐため TLA+ または同等の ordering model が必要と判定した。model 本体と atomic action の仮定は後続 slice で確定する。
5. 失敗原因は `environment_absent`、`path_mapping`、`compile`、`symex`、`solver`、`memory`、`parallelism`、`infrastructure`、`product_oracle_failure` に分類する。full cause classifier は未実装であり、本 slice では証跡側で cause class を記録する。
6. Ruviform 還元は製品非依存の runner/evidence schema に限定される見込みだが、`rf#1015` との重複範囲が未確定なため本 slice では起票しない。

## slice 1 で実行した検証

| Command | OS | Exit | Verdict | 証跡/備考 |
| --- | --- | ---: | --- | --- |
| inventory name/count check（Python、補助） | Windows x64 | 0 | pass | `inventory-check.txt`。57/57 の matrix 名が対象 source に実在 |
| `mise run test-formal` | Windows x64 | 0 | pass | `test-formal.log`（local artifact）。gateway 35/35、recording 15/15、計 50/50 pass。既存 lane の回帰確認 |
| `mise run fmt-check` | Windows x64 | 1 | fail（環境/ツール起因） | `fmt-check.log`（local artifact）。`cargo fmt --all -- --check` が `os error 206` で起動失敗。rustfmt 差分の失敗ではない |
| `mise exec -- prek run --all-files` | Windows x64 | 0 | pass | `prek.log`（local artifact）。whitespace/EOF/YAML/TOML/line-ending/BOM/key/actionlint hooks pass |
| per-package `cargo fmt -p <package> -- --check`（補助証拠） | Windows x64 | 0 | pass | `fmt-check-aux.log`（local artifact）。workspace 15 package。canonical は失敗した `mise run fmt-check` であり、本行は代替ではない |
| `mise run test-formal-kani` / TLA+ | — | — | unsupported（slice 1 scope 外） | 実走しなかった。slice 1 で新たな証明証拠はない |

slice 1 の `*.log` はリポジトリの `.gitignore` で追跡されないため local artifact として扱い、コマンド・exit code・結果を本文書へ記録した。docs index（`docs/**/README.md`）は生成物であり、共通契約に従って編集・再生成していない。

## slice 2 — selective interface

`tools/run-formal-kani.mjs` に次の最小 env interface を実装した。

- `KANI_PACKAGE`: 指定時は `synvelis-gateway` または `synvelis-recording` の 1 件だけを対象にする。省略時は従来どおり両 package。
- `KANI_HARNESS`: 指定時は `cargo kani` へ `--harness <値>` を 1 回だけ渡す。省略時は従来どおり filter なし。
- 未知 package、空値、空白のみの harness は verification に入らず `configuration_error`・exit 2 で停止する。
- 複数 filter、glob、正規表現、exact 指定の独自文法は作らない。
- 既存 `KANI_JOBS` の既定 4 と正整数 validation、native cargo kani → WSL2 distro 検出、WSL native target dir、環境不在時 `not_run`/`environment_blocked` exit 0、真正 verification failure exit 1、1 行 JSON verdict 語彙 `pass`/`fail`/`not_run` を維持する。
- JSON verdict は lane/verdict/reason に加え、package/packages、harness、jobs、environment、elapsed seconds、exit code、matched harness count を出力する。
- stdout と stderr は別々に capture し、stdout 先・stderr 後で replay する。raw artifact は両 stream を保存するが、厳密な interleaving は保存しない。

negative case は `runner-negative-cases.txt` に記録した。

| case | command | exit | result |
| --- | --- | ---: | --- |
| unknown package | `KANI_PACKAGE=unknown mise exec -- node tools/run-formal-kani.mjs` | 2 | pass as a fail-closed test — `configuration_error`、verification 未開始 |
| whitespace harness | `KANI_PACKAGE=synvelis-recording KANI_HARNESS="   " mise exec -- node tools/run-formal-kani.mjs` | 2 | pass as a fail-closed test — `configuration_error`、verification 未開始 |

## slice 2 — invocation evidence

machine-readable invocation record の正本は `formal-proof-runs.jsonl`（`schema: syn.sv659.formal-proof-run/v1`）、raw output artifact は次の 3 件である。text artifact はリポジトリ hook により行末空白だけがトリムされるが、出力内容の省略や要約はしていない。

- `krj04-session-transitions-exhaustive.txt`
- `krj03-crc32-split-equivalence-bounded.txt`
- `kani-zero-match-probe.txt`

環境は WSL2 `Ubuntu` on Windows 11、cargo 1.99.0、Kani Rust Verifier 0.68.0、CBMC 6.11.0、solver minisat、`KANI_JOBS=1`。native Windows には `cargo kani` がなかった。

| proof | canonical invocation | proof budget | result | cost |
| --- | --- | ---: | --- | --- |
| K-RJ-04 `session_transitions_exhaustive` | `KANI_PACKAGE=synvelis-recording KANI_HARNESS=session_transitions_exhaustive KANI_JOBS=1 mise run test-formal-kani` | wall-clock 300s | **NotRun / cause class `solver`** | runner wall-clock 634.49s、invocation externally stopped、exit 15 |
| K-RJ-03 `crc32_split_equivalence_bounded` | `KANI_PACKAGE=synvelis-recording KANI_HARNESS=crc32_split_equivalence_bounded KANI_JOBS=1 mise run test-formal-kani` | wall-clock 300s | **Pass** | Kani Verification Time 282.0517s、runner wall-clock 310.115s、exit 0 |

K-RJ-04 は `VERIFICATION:- FAILED`、counterexample、unwinding assertion を出さなかった。したがって product_oracle_failure ではない。solver が設計予算 300s 内に確定せず、runner wall-clock 634.49s で invocation が外部停止されたため、proof verdict は Pass ではなく NotRun、cause class は `solver` とする。illegal-transition error path の `format!` が solver 入力を大きくしている可能性はあるが、完了ログがないため詳細原因は未確定である。peak memory は runner が RSS を計測しないため unmeasured とし、疎な `ps` sample を peak として扱わない。

K-RJ-03 は `VERIFICATION:- SUCCESSFUL`、`0 of 131 failed`（1 unreachable）で、harness は `session_journal::kani::tests::crc32_split_equivalence_bounded` であった。この結果は fixed 16 byte payload を split point `0..=16` で 2 分割した場合だけの Bounded Exhaustive である。任意長、3 以上の分割、永続化 recovery、暗号学的衝突耐性の証明ではない。

zero-match probe は `KANI_HARNESS=sv659_zero_match_probe` で実行した。Kani は `Please specify the fully-qualified name of a harness.` を出し、runner は `Checking harness` 行 0 件を検出して `harnesses_matched=0`・exit 1 で終了した。0 harness を pass と記録しないことを確認した。

## slice 2 — lane verification

| command | OS/environment | exit | result |
| --- | --- | ---: | --- |
| `mise exec -- node --check tools/run-formal-kani.mjs` | Windows 11 / mise node 24.13.0 | 0 | pass |
| `mise run test-formal` | Windows 11 | 0 | pass — gateway 35/35、recording 15/15 |
| `mise run fmt-check` | Windows 11 | 0 | pass |
| `mise exec -- prek run --all-files` | Windows 11 / prek 0.3.3 | 0 | pass |
| full Kani lane without `KANI_HARNESS` | — | — | NotRun — memory risk と slice scope のため未実施 |
| K-RJ-01/K-RJ-02/K-GJ-01 | — | — | NotRun — heavy replay/round-trip proof は後続 slice |

## 保証範囲と残件

K-RJ-03 は製品実装 `crc32` と incremental `Crc32` API の局所的な bounded proof であり、全体ジャーナル形式・任意長入力・製品全体へ転用しない。K-RJ-04 が将来 pass しても `SessionState` 4×4 と acceptance gate の有限全域だけの保証であり、recording workflow、crash recovery、永続化 ordering の証明ではない。今回は solver 未完了のため、K-RJ-04 には新しい保証が追加されていない。

残件は K-RJ-01/K-RJ-02/K-GJ-01、K-RJ-04 の solver-bound 原因調査・分割/bound 再設計、Authorization pure-core verification、Recovery ordering model、mutation 実験、full cause classifier、全件 cost comparison、Sync Commit 整備である。docs index は Coordinator が統合時に整備する。

## Conclusion

slice 1 は scope 内で完了した。slice 2 は selective interface、fail-closed configuration、zero-match fail-closed を実装・確認し、K-RJ-03 を記録された bound 内で完了した。K-RJ-04 は証明を弱めず NotRun/solver と正直に分類した。Issue #659 全体は未完了であり、この記録は partial slice の証拠である。
