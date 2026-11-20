# sv601 — formal verification D 層 verification record

Issue `scarecrow173/synvelis#601` の検証記録。slice 1 は journal wire
codec（proptest + Kani）、slice 2 は gateway authorization boundary
（`WireRequest::parse` + `GrantPolicy` proptest）。canonical =
`mise` lane（`test-formal` / `test-formal-kani`）。cargo 直接実行は
補助証拠（mutation 注入確認）として明記済み。

- Date: 2026-11-24
- OS: Windows (x86_64) — proptest/通常 lane。Kani lane は WSL2 Ubuntu
  （native `cargo kani` 不在 → WSL distro 列挙 → `/mnt/g` パス写像 →
  WSL ネイティブ target dir）
- Toolchain: rustc 1.99.1 (58cf8f9c3 2026-10-13) / cargo 1.99.1（Windows）;
  Kani Rust Verifier 0.68.0 / CBMC 6.11.0（WSL2）
- Repo SHA: `58b62f2b52630351497591c7a595a8dbc850c33d`（ブランチ起点。
  `feature/601-formal-journal`、base = `origin/main`）
- ローカル検証 = 変更範囲の検証。統合検証は merge 後の対象 SHA に
  対する必須 CI 結果が証拠（本記録はそれを代替しない）。

## 検証対象

- `synvelis-gateway` journal wire codec
  （`crates/synvelis-gateway/src/journal.rs`）: `encode_frame` / `replay` /
  `MAX_PAYLOAD` / seq 連番 / CRC32 — `tests/formal_journal.rs`。
- `synvelis-recording` journal wire codec
  （`crates/synvelis-recording/src/journal.rs`）: `encode_record` /
  `replay_journal` / `JournalAppender` — `tests/formal_journal.rs`。
- `synvelis-recording` session 状態機械（`src/session.rs`）と
  `src/crc32.rs` の incremental/oneshot 同値性 — 同ファイル内。
- 生産コードの意味論は変更なし（mutation 注入は一時編集・全 revert、
  最終 `git diff` の src/ 差分ゼロで確認）。

## Actions → resulting state

| # | Action (command) | Resulting state |
| --- | --- | --- |
| 1 | `mise run test-formal` | gateway 14/14 + recording 15/15 pass・exit 0。記録: `test-formal.log`（canonical・local artifact） |
| 2 | `mise run test-formal-kani` | 環境解決: native `cargo kani` 不在 → WSL2 Ubuntu（Kani 0.68.0 + CBMC 6.11.0）採用、`$HOME/.cache/synvelis-kani/target` の WSL-native target dir で `cargo kani --tests -p synvelis-gateway -p synvelis-recording` 実走。attempt 1（`-j4`）で 2/7 harness が `VERIFICATION:- SUCCESSFUL`（610・642 checks）まで到達したが、sibling CBMC workload 併存下で global OOM → oom-kill。以降 WSL2 VM が cbmc 起動のたび hard-crash（unclean-boot journal ×3・`-j2` 再試行・単発 light harness いずれも solver 起動直後に `E_UNEXPECTED`）→ lane 未完了。記録: `test-formal-kani-oom-attempt.log` / `test-formal-kani-vmcrash-attempt.log` / `test-formal-kani.log`（すべて local artifact） |
| 3 | mutation A 注入: `replay` の CRC 照合（`crc32(...) != crc_stored` → `BadChecksum`）を除去（`crates/synvelis-gateway/src/journal.rs` 一時編集・coordinator 承認 occupancy） | `single_byte_corruption_never_verifies_past_it` が shrink つき反例で FAILED（13 passed / 1 failed、exit 101）。payload byte 破損が CRC 不在で verified prefix を素通りすることを検出。記録: `mutation-fail-gateway.log`（local） |
| 4 | mutation A の完全 revert → `cargo test --locked -p synvelis-gateway --test formal_journal` 再実行 | 14/14 green・`git diff` src/ ゼロ。記録: `mutation-revert-green-gateway.log`（local） |
| 5 | mutation B 注入: `replay_journal` の seq 照合（`seq != next_seq` → `SequenceGap`）を除去（`crates/synvelis-recording/src/journal.rs` 一時編集・順次単発） | `encode_record_nonzero_seq_is_gap` + `resumed_appender_frames_gap_as_fresh_stream` が minimal `start=1` で FAILED（13 passed / 2 failed、exit 101）。記録: `mutation-fail-recording.log`（local） |
| 6 | mutation B の完全 revert → `cargo test --locked -p synvelis-recording --test formal_journal` 再実行 | 15/15 green・両 mutation 後の `git diff --stat src/` ゼロ（zero committed src diff）。記録: `mutation-revert-green-recording.log`（local） |
| 7 | `mise run fmt-check` / `check` / `lint` / `test`（両 crate） | 全 pass（clippy `-D warnings` clean・`--all-targets` に新規 test target を含む・パッケージ全テスト 231 passed / 0 failed） |
| 8 | `mise run check-headless-deps` / `mise run check-ruviform-rev` | 全 pass（vendor tree が stale だったため `mise run sync-ui-deps` で `84a68f25d5` へ再同期してから一致確認 — gitignore 生成物のみ、commit 差分なし） |
| 9 | `mise exec -- prek run --all-files` | 全 hook pass（whitespace/EOF/yaml/toml/line-ending/BOM/key/actionlint） |

## Lane / verification results

| Command | OS | Exit | Verdict | 備考 |
| --- | --- | --- | --- | --- |
| `mise run test-formal` | Windows x64 | 0 | pass | proptest 29 properties 全 pass（gateway 14 + recording 15） |
| `mise run test-formal-kani` | WSL2 Ubuntu x64（Windows host 経由） | n/a | not_run | lane としては環境起因で未完了（attempt ×3）。harness 単位の内訳は下表 — 2 pass・5 not_run。**反例・`VERIFICATION:- FAILED` は一切なし**。fake pass ではない |
| `cargo test … formal_journal`（gateway mutant） | Windows x64 | 101 | fail（期待どおり） | `single_byte_corruption_never_verifies_past_it` が shrink 反例を提示 |
| `cargo test … formal_journal`（gateway revert 後） | Windows x64 | 0 | pass | 変異なしで緑に復帰 |
| `cargo test … formal_journal`（recording mutant） | Windows x64 | 101 | fail（期待どおり） | seq gap 検査除去で 2 property が `start=1` 反例を提示 |
| `cargo test … formal_journal`（recording revert 後） | Windows x64 | 0 | pass | 変異なしで緑に復帰 |
| `mise run fmt-check` | Windows x64 | 0 | pass | |
| `mise run check synvelis-gateway` / `synvelis-recording` | Windows x64 | 0 | pass | `--all-targets`（新規 test target 含む）・workspace `check-cfg` で `cfg(kani)` 許容 |
| `mise run lint synvelis-gateway` / `synvelis-recording` | Windows x64 | 0 | pass | clippy `-D warnings` clean |
| `mise run test synvelis-gateway` / `synvelis-recording` | Windows x64 | 0 | pass | パッケージ全テスト pass（合計 231） |
| `mise run check-headless-deps` | Windows x64 | 0 | pass | 12 headless crates に GUI 依存なし |
| `mise run check-ruviform-rev` | Windows x64 | 0 | pass | `sync-ui-deps` 後に lockfile rev 一致 |
| `mise exec -- prek run --all-files` | Windows x64 | 0 | pass | repo hooks 全量 |

### Kani harness 単位 verdict（coordinator 裁定: harness 粒度で記録）

attempt 1（`-j4`）で 2 harness が完走 SUCCESSFUL。残 5 は attempt
1-3 いずれも solver 起動中に環境断（OOM → VM crash）のため
`not_run` — host/VM 起因であり proof 起因ではない。

| Harness（crate） | Verdict | Detail |
| --- | --- | --- |
| `nonzero_seq_is_gap_bounded`（gateway） | pass | `0 of 610 failed`・`VERIFICATION:- SUCCESSFUL`（31.4s） |
| `encode_replay_roundtrip_bounded`（gateway） | pass | `0 of 642 failed`・`VERIFICATION:- SUCCESSFUL`（45.5s） |
| `replay_contract_bounded`（gateway） | not_run | `environment_blocked` — solver 起動後に OOM/VM crash |
| `encode_replay_roundtrip_bounded`（recording） | not_run | `environment_blocked` — 同上 |
| `replay_journal_contract_bounded`（recording） | not_run | `environment_blocked` — 同上 |
| `session_transitions_exhaustive`（recording） | not_run | `environment_blocked` — 同上 |
| `crc32_split_equivalence_bounded`（recording） | not_run | `environment_blocked` — 同上（単発 probe も VM crash を再現） |

環境断の内訳（`wsl2-vm-crash.log`・local artifact）: attempt 1 は 4 solver 並行 +
sibling CBMC workload で global OOM → oom-kill。以降 WSL2 VM が
cbmc 起動のたび hard-crash（unclean-boot journal ×3・`-j2` でも
軽量単発 harness でも solver 起動直後に `E_UNEXPECTED`）— VM
リセット（`wsl --shutdown`）は全テナントに影響するため
coordinator 裁定でユーザー判断へ送り、retry は別作業として扱う。

## Bounds and assumptions（Kani）

`docs/specs/formal-verification.md` の「Kani の bound と仮定」が
正本。要約:

- `encode_replay_roundtrip_bounded`（両 crate）: payload ≤ 4 byte・
  単 record。`encode_*` → `replay` で `CleanEnd` + seq/payload 一致。
  in-cap 全サイズ・exact-cap 境界・複数 record stream は proptest
  `in_cap_roundtrips` / `encoded_stream_clean_end` 等に委譲。
- `nonzero_seq_is_gap_bounded`（gateway）: seq ∈ 1..=4 の単 record
  が `SequenceGap` で停止することを尽きる。`u64` 全域の gap は
  proptest に委譲。
- `replay_contract_bounded`（gateway）/ `replay_journal_contract_
  bounded`（recording）: ≤44 byte の任意 byte 列で (a) panic しない
  (b) stop が closed enum set 内 (c) `prefix_len ≤ input_len`
  (d) `CleanEnd` ⇔ 全入力検証済み (e) verified prefix 自身が
  clean replay。長大 stream・実 drain 経路は proptest concrete に委譲。
- `session_transitions_exhaustive`（recording）: 4 状態 × 4 遷移の
  全組合せを尽き、`require_transition` と合意する合法遷移のみ受理。
- `crc32_split_equivalence_bounded`（recording）: ≤32 byte の
  全 split 点で incremental `Crc32` == oneshot `crc32`。

`crc32` は bit-wise 実装のまま symbolic 入力上で unwind される
（table なし・allocation なし）— SAT 解は重いが bound 内で完結。

## Honest notes

- Kani lane は「任意入力の全網羅」ではなく bounded symbolic の
  尽きる検証。上表の委譲先（proptest concrete coverage・cap 境界・
  複数 record stream）と合わせて契約をカバーする構成であり、
  bound 外を証明済みとは主張しない。
- Kani harness は test file 内 `#[cfg(kani)]` モジュールであり、
  通常 build/test/lint の依存グラフ・意味論へ影響しない（workspace
  `check-cfg` で `cfg(kani)` を宣言済み）。
- `cargo kani --tests` は両 crate の全 test target を対象に CBMC
  起動するため、既存 integration test の helper（`tests/common/`）
  も compile 対象になり、`register_tool` 系 unstable feature 警告や
  unsupported foreign function 警告を emit する。これらは harness
  の証明対象外の警告であり `VERIFICATION:- FAILED` ではない。
- Kani compile 出力で `crates/synvelis-recording/tests/common/
  mod.rs:1315` の `unused variable: i` 警告が出るが、既存 helper の
  pre-existing 警告で本 slice の変更とは無関係（通常 lint lane は
  clean）。
- WSL2 lane は共有マシンの `/mnt/g` 経由 checkout でも動作するが、
  target dir は WSL ネイティブ `$HOME/.cache/synvelis-kani/target`
  に隔離（`cargo kani` の `/mnt` 上 build は著しく遅いため）。
- 変更ファイルに生産 `src/` 差分なし。`Cargo.lock` は proptest
  dev-dep 解決分のみ（本番グラフ非波及 — `proptest` は両 crate の
  `dev-dependencies` のみ）。

---

# slice 2 — authorization boundary（`WireRequest::parse` + `GrantPolicy`）

- Date: 2026-11-21
- Branch: `feature/601-formal-authz`（base = `origin/main` `6c670bb0`）
- Commits: `097e8c5`（`formal_journal` → `formal_journal/` へ module 分割、
  `strategies.rs` に共有 `arb_*` / `encode_stream` を集約・test target 名保持）
  + `3598bb8`（wire/policy 両 property suite 追加）
  + 本記録更新 commit
- OS / Toolchain: slice 1 と同一（Windows x86_64 / rustc 1.99.1）。

## 検証対象（slice 2）

- `crates/synvelis-gateway/src/wire.rs` — `WireRequest::parse`:
  top-level 自己申告 authority field / implicit-target field の
  拒否、closed 三分岐 error、allow-list 構造契約。
- `crates/synvelis-gateway/src/policy.rs` — `GrantPolicy`:
  `evaluate_commit` / `evaluate_effects` / `PolicyEvaluator::decide`
  の closed decision set、deny reason 契約、unbound agent 一律拒否、
  human permit、delegated-apply budget 単調消費、scope/effects 不変条件。
- テスト配置: `crates/synvelis-gateway/tests/formal_journal/{main,
  strategies,wire,policy}.rs`。`--test formal_journal` entry point
  と slice-1 suite（journal 14 + recording 15）は不変。
- 生産コードの意味論は変更なし（mutation は一時編集・全 revert、
  最終 `git diff` の src/ 差分ゼロで確認）。

## Actions → resulting state（slice 2）

| # | Action (command) | Resulting state |
| --- | --- | --- |
| 1 | `mise run test-formal` | gateway 35/35 + recording 15/15 pass・exit 0（slice-1 の 14 property に wire 9 + policy 12 を追加） |
| 2 | `cargo test --locked -p synvelis-gateway --test formal_journal wire::authority_fields_rejected`（mutation 探査 run: `"token"` 除去済み・ただし suite が `FORBIDDEN_AUTHORITY_FIELDS` 自己参照で生成していた時点） | **mutation 未検出（pass）** — 生成器が生産 const を参照していたため、entry 削除は contract を縮退させず test 空間だけ縮小。検出不能な property であることが判明（survived mutant）。記録: `logs/mutation-probe-undetected-wire.log`（local） |
| 3 | suite 修正: `AUTHORITY_FIELDS` / `IMPLICIT_TARGET_FIELDS` を test-local な literal copy として定義し注入生成を切替え、`documented_forbidden_lists_stay_in_product`（documented ⊆ product の subset pin）を追加 → `cargo test … --test formal_journal` | 35/35 green（修正のみ・mutation 未適用） |
| 4 | mutation 適用: `FORBIDDEN_AUTHORITY_FIELDS` から `"token"` のみ除去（`src/wire.rs` 一時編集・coordinator 承認 occupancy: 1 ファイル・当該 1 entry のみ・即 revert） | `wire::authority_fields_rejected` が **FAILED**、shrink 済み最小反例 `field = "token", injected = Null`（`expected SelfDeclaredAuthority, got MalformedRequest("unknown field `token`…")`）。entry 削除が契約違反として検出されることを確認。記録: `logs/mutation-wire-token-authority_fields_rejected.log`（local） |
| 5 | mutation 完全 revert（`git restore`）→ `cargo test --locked -p synvelis-gateway --test formal_journal` 再実行 | 35/35 green・`git status` src/ 差分ゼロ。記録: `logs/mutation-wire-token-revert-green.log`（local）。mutation 由来の `proptest-regressions/` seed ファイルは未追跡のまま削除（commit 対象外の mutation artifact） |
| 6 | `mise run fmt-check` / `check synvelis-gateway` / `lint synvelis-gateway` / `cargo test --locked -p synvelis-gateway`（全 target） | 全 pass（clippy `-D warnings` clean、package 全 35 test target・0 failed） |
| 7 | `mise run check-headless-deps` / `mise exec -- prek run --all-files` | 全 pass（12 headless crate に GUI 依存なし / repo hooks 全量） |

## Lane / verification results（slice 2）

| Command | OS | Exit | Verdict | 備考 |
| --- | --- | --- | --- | --- |
| `mise run test-formal` | Windows x64 | 0 | pass | proptest 50 properties 全 pass（gateway 35 + recording 15）。slice-1 suite 不変 |
| `cargo test … authority_fields_rejected`（探査 run・自己参照 generator） | Windows x64 | 0 | pass（= mutation survived） | 生成器の自己参照で entry 削除を検出不能 → suite 側の contract pin 不備として修正（下記 Honest notes） |
| `cargo test … authority_fields_rejected`（mutant・正式） | Windows x64 | 101 | fail（期待どおり） | shrink 済み `field = "token"` 反例で `SelfDeclaredAuthority` 契約違反を検出 |
| `cargo test … formal_journal`（revert 後） | Windows x64 | 0 | pass | 変異なしで緑に復帰・src/ 差分ゼロ |
| `mise run fmt-check` | Windows x64 | 0 | pass | |
| `mise run check synvelis-gateway` | Windows x64 | 0 | pass | `--all-targets`（`formal_journal` dir-module 化後の test target 含む） |
| `mise run lint synvelis-gateway` | Windows x64 | 0 | pass | clippy `-D warnings` clean |
| `cargo test --locked -p synvelis-gateway` | Windows x64 | 0 | pass | package 全 35 test target・0 failed |
| `mise run check-headless-deps` | Windows x64 | 0 | pass | 12 headless crates に GUI 依存なし |
| `mise exec -- prek run --all-files` | Windows x64 | 0 | pass | repo hooks 全量 |
| `mise run test-formal-kani`（wire/policy 対象の新規 harness） | — | n/a | not_run | slice 2 では Kani harness を追加しない（下記）。slice-1 harness の 5 件 not_run/`environment_blocked` は前セクションの裁定を継承 |

## Bounds and assumptions（slice 2 proptest）

`docs/specs/formal-verification.md` が正本。要約:

- JSON 生成: 深さ ≤ 4・container ≤ 6 要素の bounded `serde_json::Value`
  （任意キー + 既知 wire キー + forbidden キーの混合辞書）。256 cases。
- wire suite（9 properties）: no-panic + closed Err set、authority/
  implicit 注入（test-local literal list 由来）、authority 優先順位、
  ネスト禁止キー非検査、独立 allow-list model との受理一致、
  unknown-key / 必須キー欠落の Malformed、documented ⊆ product pin。
- policy suite（12 properties）: 64 cases（spine 生成の組合せ爆発を
  抑えるため既定 256 から低減）。bound-spine 生成器が有効
  binding に到達しつつ expiry/revocation/epoch/permission/data access/
  scope/effects/budget の各 defect 分岐を意図的に探索。
- delegated-apply budget: 1 commit = 1 unit・単調・枯渇で
  `"apply budget exhausted"`・preflight/非 commit/denial は非消費。
- Kani promotion: wire/policy は `serde_json` 値・`Arc<String>`・
  dynamic trait object（`PolicyStore`/`Clock`/`Oracle`）を持つため
  現行 bounded-proof target 外 — 新規 harness なし、昇格判断は
  promotion 表に記録（spec 正本）。

## Honest notes（slice 2）

- **探査 run での survived mutant について**: 初回の mutation 注入は
  suite が `FORBIDDEN_AUTHORITY_FIELDS` を generator の出所として
  参照していたため検出できなかった（削除 entry が test 空間からも
  消える自己参照問題）。これは property の欠陥であり、test-local
  literal list + subset pin property で修正済み。両ログを証拠として
  残す（`logs/mutation-probe-undetected-wire.log` /
  `logs/mutation-wire-token-authority_fields_rejected.log`）。
  同型の自己参照は他の生成器にも無いことを確認済み（deny reason は
  `DENY_REASONS` literal copy・wire model checker は生産 table 非参照）。
- wire の allow-list model は test-local な独立実装であり、生産の
  allow-list helper を import しない（生産側の table 縮退を
  model/product 双方で同時に隠さないため）。documented ⊆ product
  の subset pin は「product が contract を拡張する」方向は許容し
  「contract から削る」方向のみ fail する。
- policy suite は各 generated case ごとに `InMemoryPolicyStore` /
  `ManualClock` / `StaticOracle` / `GrantPolicy` / fixture `Gateway`
  を新規構築し、store 状態の持ち越しを排除している。
- `decide` ≡ `evaluate_commit`（Commit stage）は全 generated case で
  一致を確認。`evaluate_effects` は非 commit stage で budget を
  消費しないことを property として固定。
- 生産 `src/` の committed 差分はゼロ（mutation は全 revert 済み）。
- slice-1 Kani 環境断（WSL2 VM crash）は未解決のまま継承 —
  本 slice では新規実行なし、verdict 変更なし。
- ローカル検証は merge 後 CI の代替ではない（slice-1 と同じ注記）。
