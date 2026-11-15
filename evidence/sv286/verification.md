# sv#286 verification evidence — CB-09 Difference Explorer headless view-model + ViewSpec

Slice: `synvelis-workspace` に Creative Branch Difference
Explorer の headless view-model（`variant_diff.rs`）と
driver-neutral `ViewSpec` projection を実装。pinned context・
branch authority・Semantic/Signal/Perceptual typed evidence を
入力に取り、typed action candidate → decode → authority 検証
付き apply → `VariantDiffIntent` の経路を提供。mock/Fake
driver が GPUI 実装と並行開始できる契約を固定する。

## Environment

| Field | Value |
| --- | --- |
| Synvelis commit | `48f1e1e5d807579d2af67ad97771b38eae34f6f0` (origin/main base) + this change set |
| OS | Windows (win32), NTFS |
| Toolchain | rustc 1.99.1 (58cf8f9c3, mise managed; CI pin 1.99.0 in mise.ci.toml) |
| Cargo.lock Ruviform SHA | `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`（変更なし） |
| Schema | `SCHEMA_VERSION = 14` |

## Commands run (canonical = mise)

| Command | Exit | Result |
| --- | --- | --- |
| `mise run fmt` | 0 | pass — `cargo fmt --all` で整形適用 |
| `mise run fmt-check` | 0 | pass（初回は variant_diff.rs/test file の整形差分で fail → `mise run fmt` 適用後に再 check で pass） |
| `mise run check synvelis-workspace` | 0 | pass（`cargo check --locked --all-targets -p synvelis-workspace`。clippy 修正後に再実行済み） |
| `mise run lint synvelis-workspace` | 0 | pass（`cargo clippy --locked --all-targets -p synvelis-workspace -- -D warnings`。doc-markdown/map-unwrap-or/cast-possible-truncation/assigning-clones/similar-names/assert-is-empty/field-reassign-with-default/default-trait-access の計 12 件を修正して pass） |
| `mise run test synvelis-workspace` | 0 | pass — 全 test binary 成功。`cb09_diff_explorer` 33/33（`cb09_real_changeset_projects_domain_rows` が real `compare_snapshots` ChangeSet 投影・Variants bookkeeping 分離を含む）を含む全 binary `test result: ok`。review-protocol round の修正（apply 時 context 再検証・selected-row detail・adopt dedup・reason cap・summary 全計上）後に再実行済み |
| `mise run check-headless-deps` | 0 | pass — "no GUI framework deps in 12 headless crates"（`variant_diff.rs` は `ruviform_ui::view` の driver-neutral spec 型のみ使用・GUI/GPU/WebView 依存なし） |
| `mise run check-full` | 0 | pass（`cargo check --workspace --all-targets --locked` — `ViewType::VariantDiff` 追加による synvelis-app 等の exhaustive match 破壊なし） |
| `mise exec -- prek run --all-files` | 0 | pass（trailing-whitespace/end-of-file/check-yaml/check-toml/mixed-line-ending/byte-order-marker/detect-private-key/actionlint 全 pass） |

## Auxiliary evidence (canonical = mise, cargo direct)

`cargo test -p synvelis-workspace --test cb09_diff_explorer` —
33 passed / 0 failed（`mise run test synvelis-workspace` の
一部としても実行済み。direct cargo は開発中の反復確認用で、
正式記録は mise 経路）。

## Doc index 再生成

- `node .agents/skills/impl-doc/scripts/sync_impl_index.js
  --kind ir --apply` → `docs/impl/ir/README.md` に
  `cb-09-diff-explorer.md` 追加（exit 0）
- `node .agents/skills/doc-maintenance/scripts/
  doc_maintenance.js index apply --dir docs/test-specs` →
  `docs/test-specs/README.md` 更新（exit 0）
- `sync_impl_index.js --kind exp --apply` → IN-SYNC
  （exp README は `.jsonl` log のみ管理。cb-01〜06 の `.md`
  experiment memo と同じく index 対象外）

## NotRun

- `*-msrv` 系 — Daily Full CI 委譲。
- `mise run lint-full`/`test-full`/`build-full`/`verify-full` —
  タスク固有の必須検証に含まれず未実行（`check-full` は
  workspace 破壊検出のため実行済み）。
- 実 GPUI mount・GPUI-component 実レンダリング・focus/keyboard
  経路 — 本 slice は driver-neutral ViewSpec まで。実 driver
  consumer は別 slice。
- 実 plugin state inspect・PCM/RT signal 測定・real
  perceptual evaluation — signal/perceptual layer は mock seam
  （`EvidenceOrigin::Mock`）で provenance label を spec に
  露出する設計を test で固定。実測定は #288 系 gate。
- CB-08 orchestrator（#285）からの end-to-end 駆動・adopt
  実行（intent 発行までが本 slice の範囲）。
- 実 OS window/IME/accessibility — #288 系 gate。
- `test-rt`/`test-recording-real-input`/`test-recording-real-fs`/
  `test-project-real-fs` 等の real-device lane — 本 slice と
  無関係・hardware 依存のため未実行。

## Known limitation（修復しない・正直記録）

- Arrangement domain（CB-07/#284 未 merge）は adapter 不在の
  ため `EvidenceState::Unsupported{DomainAdapterUnmerged}` —
  fabricated diff を出さない設計上の非対応であり、test で
  固定済み。CB-07 merge 後に supported へ昇格する余地を残す。
- `VariantDiffViewState` の serde 形状は v1 — 将来 field 追加は
  `#[serde(default)]` で後方互換維持が前提。
- `variant_diff.rs` は view-model・action・ViewSpec projection
  を単一モジュールに収める（2054 行）。責務は型で分離済み
  だが、実測定 adapter 接続時に aux section 投影の拡張点と
  なる。
