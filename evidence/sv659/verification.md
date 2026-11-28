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

## slice 3 の識別情報

- sync SHA: `34e69fb27774f41f6aded1a24320be90ffd7477a`
- branch: `test/659-mutation-cost-matrix`
- Property mutation lane OS: Windows x64 native CLI、cargo 1.99.1
- selective proof OS: Windows x64 host 上の WSL2 Ubuntu、x86_64、16 logical CPUs、約 32056 MiB、cargo 1.99.0、Kani Rust Verifier 0.68.0、CBMC 6.11.0
- 実行順: baseline、K-GJ-02、K-GJ-03、K-RJ-03、M-GJ-CRC、M-RJ-SEQ、M-WIRE-TOKEN、M-POL-DENY-ORDER、M-POL-BUDGET、最終非回帰/衛生レーン

## slice 3 — baseline と selective proof cost

baseline は mutation 前の `mise run test-formal` で exit 0・wall-clock 47.560s、gateway 35/35・recording 15/15 pass だった（`slice3-baseline-test-formal.txt`）。これは proof pass ではなく、mutation の因果判断を可能にする property lane baseline である。

selective proof はすべて canonical な `mise run test-formal-kani` 入口に env selector を付けて `KANI_JOBS=1` で実行した。exit 0 だけでなく、raw output の `VERIFICATION:- SUCCESSFUL`、failed check 数、runner verdict、環境を確認した。

| proof | command selector | verdict | failed checks | Kani Verification Time | runner elapsed | shell wall-clock | raw output |
| --- | --- | --- | --- | --- | --- | --- | --- |
| K-GJ-02 | `KANI_PACKAGE=synvelis-gateway KANI_HARNESS=encode_replay_roundtrip_bounded KANI_JOBS=1` | Pass | 0/642 failed (6 unreachable) | 33.076668s | 104.125s | 105.486s | `slice3-kgj02-encode-replay-roundtrip-bounded.txt` |
| K-GJ-03 | `KANI_PACKAGE=synvelis-gateway KANI_HARNESS=nonzero_seq_is_gap_bounded KANI_JOBS=1` | Pass | 0/610 failed (10 unreachable) | 24.030256s | 67.862s | 69.082s | `slice3-kgj03-nonzero-seq-is-gap-bounded.txt` |
| K-RJ-03 | `KANI_PACKAGE=synvelis-recording KANI_HARNESS=crc32_split_equivalence_bounded KANI_JOBS=1` | Pass | 0/131 failed (1 unreachable) | 234.04366s | 288.605s | 289.802s | `slice3-krj03-crc32-split-equivalence-bounded.txt` |

3 件とも 300s の verification budget 内だが、runner elapsed/wall-clock には compile/loading が含まれるため Verification Time と混ぜない。K-GJ-02 はこの worktree で最初の gateway selective run、K-GJ-03 は直後の warm cache、K-RJ-03 は recording selective run として測定した。WSL load average sample は 1.24/0.72/0.32・cbmc 1 process だった。historical baseline は sv601 の K-GJ-02 45.5s・K-GJ-03 31.4s、slice2 の K-RJ-03 282.0517s/310.115s を `cost-mutation-matrix.json` に併記したが、sv601 値は現存 artifact から計測項目名を一意に復元できないため同指標比較の主証拠にはしない。slice2/slice3 の差分からも、環境差と cache 状態差があるため性能因果は断定しない。peak memory は runner が RSS を計測しないため `unmeasured` とし、疎な sample を peak と偽らない。

接続条件は限定的である。K-GJ-02 は固定 12 byte・seq0・単 record の GJ-P03 を補強するが、可変長、複数 record、full stream を証明しない。K-GJ-03 は固定 8 byte・単 record の任意非零 seq gap を示すが、複数 record 欠番や上位 resume を証明しない。K-RJ-03 は固定 16 byte・split ≤16 の RJ-P10 を示し、任意長、3 以上分割、暗号学的耐性を証明しない。上位 Journal/Recovery/製品全体契約を導くには、GJ-P05/RJ-P05 の stream roundtrip、resume/recovery ordering、Chunk/cap 境界、FS durability などの接続条件が残る。

## slice 3 — mutation matrix

各 mutation は 1 件ずつ適用し、canonical `mise run test-formal` の mutant run、完全 revert、revert 後の同 lane、production diff check の 3 点セットを記録した。複数 mutation を同時に重ねていない。最終的に生産 `src/` 4 件の diff はゼロである。

| mutation | 変異 | 検出 test | mutant | revert | verdict |
| --- | --- | --- | --- | --- | --- |
| M-GJ-CRC | gateway journal CRC mismatch 検査を `false &&` で無効化 | `single_byte_corruption_never_verifies_past_it`（GJ-P06）。shrink は payloads=[[]]・rec_pick 0・xor=1 | exit 101・49.787s・34/1 failed | exit 0・14.790s | killed |
| M-RJ-SEQ | recording journal seq 連続性検査を `false &&` で無効化 | `encode_record_nonzero_seq_is_gap`（RJ-P04）と `resumed_appender_frames_gap_as_fresh_stream`（RJ-P06）。最小値は seq=1/start=1 | exit 101・9.712s・13/2 failed | exit 0・6.130s | killed |
| M-WIRE-TOKEN | `FORBIDDEN_AUTHORITY_FIELDS` から `"token"` だけを除去 | `wire::documented_forbidden_lists_stay_in_product`（GW-P01）、`wire::authority_precedes_implicit_target`、`wire::authority_fields_rejected`（GW-P04）。shrink は field="token"・injected=Null | exit 101・10.408s・32/3 failed | exit 0・11.076s | killed |
| M-POL-DENY-ORDER | descriptor mismatch gate と trusted human short-circuit の順序を逆転 | `policy::descriptor_mismatch_denies_first`（GP-P03）。minimal input は human=true | exit 101・10.267s・34/1 failed | exit 0・10.037s | killed |
| M-POL-BUDGET | DelegatedApply Commit が `try_consume_apply` による 1 unit 消費を行わない | `policy::delegated_apply_commit_spend_is_monotone`（GP-P04）。shrink は budget=None・consumed 0/expected 1 | exit 101・10.226s・34/1 failed | exit 0・9.991s | killed |

survived/inconclusive はなかった。M-POL-BUDGET の直接検出は GP-P04 であり、GP-P05 は非消費側の接続 oracle として残るが、この mutant を検出した test として偽って記録しない。mutation killed は oracle の検出能力証拠であり、Property の assurance kind を引き上げない。oracle 強化や formal_journal test 変更は不要だった。proptest regression 出力はコミット対象に含めず、作業後に残っていないことを確認した。

machine-readable な正本は `formal-proof-runs.jsonl`、`mutation-runs.jsonl`、`cost-mutation-matrix.json`、raw output `slice3-*.txt`、mutation diff `slice3-*-mutation.diff` である。

## slice 3 — final non-regression and hygiene

| lane | command / OS | exit | wall-clock | 結果 |
| --- | --- | --- | --- | --- |
| final formal property lane | `mise run test-formal` / Windows x64 | 0 | 3.657s | Pass。gateway 35/35・recording 15/15。`slice3-final-test-formal.txt` |
| production diff hygiene | `git diff --exit-code -- crates/synvelis-gateway/src/journal.rs crates/synvelis-gateway/src/wire.rs crates/synvelis-gateway/src/policy.rs crates/synvelis-recording/src/journal.rs` / Windows x64 | 0 | — | 生産コード差分ゼロ。`slice3-final-production-diff-check.txt` |
| canonical fmt lane | `mise run fmt-check` / Windows x64 | 1 | 3.374s | 環境/工具制約で失敗。cargo fmt が workspace 全 target を 1 つの rustfmt 命令へ連結し、Windows で `ファイル名または拡張子が長すぎます。 (os error 206)` となった。`slice3-final-fmt-check.txt` |
| fmt auxiliary evidence | `mise exec -- cargo fmt -p synvelis-gateway -p synvelis-recording -- --check` / Windows x64 | 0 | 5.489s | 変更対象 crate 2 件の rustfmt check は pass。`slice3-final-fmt-aux.txt` |
| prek initial run | `mise exec -- prek run --all-files` / Windows x64 | 1 | 1.757s | 本 slice で追加した docs/test-spec の EOF newline だけを end-of-file-fixer が修正。コードや検証契約の失敗ではない。`slice3-final-prek.txt` |
| prek rerun | `mise exec -- prek run --all-files` / Windows x64 | 0 | 7.750s | Pass。`slice3-final-prek-rerun.txt` |

fmt の補助確認として短い subst alias から canonical lane を試したが、cargo metadata が canonical path を使うため同じ os error 206 だった（`slice3-final-fmt-check-subst-attempt.txt`）。この失敗は本 slice の Rust 差分に由来しない。本 slice の最終 diff は docs/evidence/skill のみで、生産 Rust コード差分はゼロである。変更対象 crate に対する rustfmt check は補助証拠として Pass したが、canonical fmt lane 自体は Windows x64 環境で Fail として正直に記録する。

full lane（全 7 proof 一括実行）、K-RJ-01/K-RJ-02/K-GJ-01/K-RJ-04、heavy replay/round-trip proof、Authorization pure core、Recovery ordering model は本 slice でも実行していない。

## slice 4 の識別情報

- branch: `test/659-session-proof-boundary`
- base/`origin/main` at start: `a5e29228e6a08940e7396325daab138295a5e894`
- code/proof sync SHA: `dbddb52f20d8a98e9f5a9d6e1a6571307cfb7659`
- OS: Windows x64。Kani proof は WSL2 Ubuntu（x86_64、16 logical CPUs、約 32056 MiB、cargo 1.99.0、Kani 0.68.0、CBMC 6.11.0）で実行
- scope: K-RJ-04 `session_transitions_exhaustive` の solver boundary 再設計と selective proof 実走のみ

## slice 4 — solver cost の切り分け

slice 2 baseline では K-RJ-04 は反例や unwinding assertion を出さず、solver が予算内に完了しない NotRun だった。slice 4 では proptest-only test code と illegal-transition error construction を分けて確認した。

まず proptest-only strategy/test を `cfg(not(kani))` で Kani codegen 対象から外した。この変更だけの selective proof は Pass せず、runner elapsed 336.099s・exit 15 で外部停止した。GOTO pretty-name map の補助抽出では proptest entry は 0 だったが、`fmt` 738、`format` 39、`alloc` 1109 entry が残っていた。したがって proptest-only test code は compile/codegen 上の無関係境界ではあるが、支配的な solver cost ではなかったと記録する。

次に `require_transition` の externally visible error text を保ったまま、16 pair の finite static lookup から同じ message を構成する実装へ変更した。再設計後の GOTO symbol map は total 1603、`fmt` 42、`format` 0、`alloc` 618 entry になり、proof は Verification Time 5.1956944s で完走した。この対照から、支配要因は `format!` 経由の illegal-transition error construction が solver 入力へ入ることだったと記録する。memory は unmeasured であり、OOM killer や WSL VM crash は観測していない。補助抽出は canonical ではなく、canonical proof は mise task である。

## slice 4 — selective proof result

| proof | canonical invocation | verdict | failed checks | Kani Verification Time | runner elapsed | shell wall-clock | raw output |
| --- | --- | --- | --- | --- | --- | --- | --- |
| K-RJ-04 | `KANI_PACKAGE=synvelis-recording KANI_HARNESS=session_transitions_exhaustive KANI_JOBS=1 mise run test-formal-kani` | Pass | 0/269 failed (4 unreachable) | 5.1956944s | 30.248s | 32s | `slice4-krj04-session-transitions-exhaustive.txt` |

raw output に `VERIFICATION:- SUCCESSFUL` があり、runner verdict は `pass`、exit code は 0、runner JSON の `harnesses_matched` は 1、`harnesses` は `proofs::session_transitions_exhaustive` のみだった。実行は外部 timeout wrapper つきで行ったが、wrapper は発火せず自然終了した。Kani Verification Time、runner elapsed、shell wall-clock は区別する。

この Pass は `SessionState` 4×4 pair、`Created→Open`・`Open→Closed`・`Closed→Finalized` の 3 edge、`accepts_registration` iff `Open`、`accepts_take_verification` iff `Open|Closed` の有限全域だけを示す。recording workflow、crash recovery、永続化 ordering、製品全体へ転用しない。

## slice 4 — canonical verification

| command | OS/environment | exit | result |
| --- | --- | ---: | --- |
| `mise run test-formal` | Windows x64 | 0 | Pass。gateway 35/35・recording 15/15。`slice4-redesigned-test-formal.txt` |
| `KANI_PACKAGE=synvelis-recording KANI_HARNESS=session_transitions_exhaustive KANI_JOBS=1 mise run test-formal-kani` | Windows x64 host / WSL2 Ubuntu | 0 | Pass。上記 selective proof |
| `mise run check synvelis-recording` | Windows x64 | 0 | Pass。`slice4-canonical-verification.txt` |
| `mise run lint synvelis-recording` | Windows x64 | 0 | Pass。`slice4-canonical-verification.txt` |
| `mise run test synvelis-recording` | Windows x64 | 0 | Pass。`slice4-canonical-verification.txt` |
| `mise run fmt-check` | Windows x64 | 1 | Fail（環境/工具制約）。`os error 206`。コード差分由来の rustfmt 差ではない |
| `mise exec -- cargo fmt -p synvelis-recording -- --check`（補助証拠、canonical = mise） | Windows x64 | 0 | Pass。canonical fmt lane の失敗を代替しない |

full lane（全 7 proof 一括実行）、K-RJ-01/K-RJ-02/K-GJ-01、Authorization pure core、Recovery ordering model は slice 4 の対象外であり、実行していない。

## 保証範囲と残件

K-RJ-03 は製品実装 `crc32` と incremental `Crc32` API の局所的な bounded proof であり、全体ジャーナル形式・任意長入力・製品全体へ転用しない。K-RJ-04 が将来 pass しても `SessionState` 4×4 と acceptance gate の有限全域だけの保証であり、recording workflow、crash recovery、永続化 ordering の証明ではない。今回は solver 未完了のため、K-RJ-04 には新しい保証が追加されていない。

残件は K-RJ-01/K-RJ-02/K-GJ-01、Authorization pure-core verification、Recovery ordering model、full cause classifier、peak memory 計測基盤、全件 cost comparison の残り、Sync Commit 整備である。slice 3 の 5 件 mutation matrix は完了し、いずれも killed だったが、これは oracle 検出能力の証拠であって assurance kind の引き上げではない。docs index は Coordinator が統合時に整備する。

## Conclusion

slice 1 は scope 内で完了した。slice 2 は selective interface、fail-closed configuration、zero-match fail-closed を実装・確認し、K-RJ-03 を記録された bound 内で完了した。K-RJ-04 は証明を弱めず NotRun/solver と正直に分類した。slice 3 は低コスト selective proof 3 件を再実行して cost を記録し、5 件の mutation matrix を killed として記録した。slice 4 は K-RJ-04 を有限全域の内側で Pass させたが、Issue #659 全体は未完了であり、この記録は partial slice の証拠である。
