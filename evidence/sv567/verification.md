# sv567 verification — 検証記録

- 対象 Issue: `synvelis#567`（OPEN 維持）
- Branch: `bugfix/567-quota-pend-receipt`（base `origin/main`
  `b73018bd`）
- 実行環境: Windows 11 (Git Bash) / mise 管理 toolchain
  （rustc は `mise.toml` pin — `mise exec` 経由）。
  canonical 検証は `mise run`、cargo 直接実行は補助証拠。
- ローカル検証 = 変更範囲の検証。統合検証（merge 後対象 SHA の
  CI 証拠・PR `verify (windows-latest)`）は別途 — 本書の
  Pass はローカル実測のみを示す。

## 修正前再現（補助証拠）

| コマンド | OS | 結果 |
| --- | --- | --- |
| `mise exec -- cargo test -p synvelis-render --test rcp03_render_policy async_pol06_concurrency_quota_pends_then_resumes`（helper thread へ 200ms sleep を一時挿入した決定的再現パッチ適用時） | Windows | **exit 101 / FAILED** — CI と同一 signature `quota pend stays receipt-visible under async`（rcp03_render_policy.rs:2155、計装で +1 行シフト）。再現後パッチは revert 済み |

詳細: `evidence/sv567/reproduction.md`。

## 修正後検証（canonical = mise）

| コマンド | OS | toolchain | exit | 結果 |
| --- | --- | --- | --- | --- |
| `mise run check synvelis-render` | Windows | mise pin rustc | 0 | Passed — `cargo check` dev profile Finished |
| `mise run lint synvelis-render` | Windows | 同上 | 0 | Passed — clippy Finished、warning なし |
| `mise run test synvelis-render` | Windows | 同上 | 0 | Passed — 全 test binary 緑（`rcp03_render_policy` 30 tests 含む全 suite `test result: ok`） |
| `mise run fmt-check` | Windows | 同上 | 0 | Passed — `cargo fmt --all -- --check` 差分なし |
| `mise run check-headless-deps` | Windows | node (mise) | 0 | Passed — `no GUI framework deps in 12 headless crates` |
| `mise exec -- prek run --all-files` | Windows | prek (mise) | 0 | Passed — 全 hook Passed/Skipped |

## flake 検証ループ（補助証拠）

目的: 修正の決定性を実測で補強する。構造的決定性の根拠は
(d) の happens-before 論証（IR 参照）であり、N 回 pass をもって
決定的とは主張しない — 反復はその補助。

| 実行 | 内容 | 結果 |
| --- | --- | --- |
| 直接バイナリ loop ×3 並列 | `target/debug/deps/rcp03_render_policy-7cd06cb37fe933be.exe async_pol06` を 3 shell × 14 iteration = **計 42 iteration**。各 iteration は async_pol06 filter 5 テスト（`concurrency_quota_pends_then_resumes` 含む）を実行。3 loop の同時実行で CPU 競合・scheduler thread 競合を人工的に付加 | **42/42 pass・0 fail**（LOOP_A/B/C 各 pass=14 fail=0） |
| 単発確認 | `mise exec -- cargo test -p synvelis-render --test rcp03_render_policy async_pol06`（修正直後） | 5 passed / 0 failed |

環境注記: 直接バイナリ実行は cargo lock を介さないため 3 loop が
真に並列に走り、CI の contended runner に近い競合条件を再現した。

## 未実施（NotRun）と理由

| 項目 | 区分 | 理由・後続 |
| --- | --- | --- |
| `mise run check-full` / `lint-full` / `test-full` / `build-full` / `verify-full`・Feature Matrix・`*-msrv`・3OS 横断 | NotRun | `POLICIES.md` §5 — 日常必須検証に含めない。scoped equivalent = `mise run test synvelis-render` で充足。統合検証は merge 後対象 SHA の CI（Daily/手動 Full CI・PR lane）に委譲 |
| 実 PCM / 実 plugin / 実 GUI lane | NotRun | headless 実測のみが本 slice の対象。GUI-L2/EXT-L3 は対象外 |
| `synvelis#566` 担当領域の ubuntu lint-full dead_code | Unsupported（本 slice 範囲外） | 並行 Worker sv#566 の占有パス。同一手動 CI run の失敗は本 slice と無関係 |

## PR CI（統合検証の先行証拠 — merge 前の対象 SHA）

`ci.yml` は `workflow_dispatch` 専用（pull_request trigger 撤去済み）
のため、Worker が branch ref で dispatch して PR head SHA を検証:

- run `44278685565`（`workflow_dispatch` on
  `bugfix/567-quota-pend-receipt`、`expected_sha`
  `fd2e2acea945465c3ab238cfd978a89d89438982` = PR head）
- **`verify (windows-latest)`: Passed（24m32s、全 step 緑 —
  `mise run test-full` の `rcp03_render_policy` suite を含む）**
  → Issue AC「Windows CI lane green」を充足
- `verify-macos`: Passed（16m13s）/ `msrv-verify`: Passed（12m16s）
- `verify (ubuntu-latest)`: **Failed** — `lint-full` で
  `crates/synvelis-cli/src/session.rs` の dead_code
  （`VAULT_SERVICE`・`vault_user`、main 既存）により fail。
  sv#566 の占有パスで本 slice 範囲外。`evidence`・
  `ci-summary` の fail はこの leg の fail-closed cascade
- その他 lane（`ui-verify` ×3・`gw10-verify` ×4・`rt-verify`・
  `dep-guard`）: 全て Passed

merge 後 main の CI 証拠は Coordinator が確認する。

## 判断根拠の要約

- 根因: **test-design race**（scheduler defect ではない）。
  決定的再現（helper 遅延 → CI 同一 signature）と receipt
  記録経路のコード検証（`push_receipt` 同期 append・
  `job_receipts` filter のみ）で確定。
- 修正: scheduler 境界に nonblocking `enqueue_intent` を追加
  （public API 追加の justification は IR「Decisions」参照）。
  test は helper thread を廃し `enqueue ≺ release` の
  happens-before を program order で構造保証。
- assertion は不変: B に `Pending{QuotaExceeded}`・`Resumed`
  receipt、両 job `Published`、executor calls==2。
