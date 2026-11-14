# SV-277 Verification Record — CB-00 Creative Branch 統合設計（docs-only）

実施日: 2026-11-14 / OS: Windows (worktree `sv277-cb0`) /
Toolchain: mise 管理（`mise.toml` 正本。Rust は docs-only のため
cargo build/test 不要 — cargo fmt のみ fmt-check で実行）。

## 実施した検証（実測）

| # | コマンド | 結果 | exit |
| --- | --- | --- | --- |
| V-1 | `mise run fmt-check` | `cargo fmt --all -- --check` 成功（docs-only だが workspace fmt 契約として実行） | 0 |
| V-2 | `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index plan/apply --dir docs/specs` | index apply: 1 written (`docs/specs/README.md`) | 0 |
| V-3 | 同上 `--dir docs/designs` | index apply: 1 written (`docs/designs/README.md`) | 0 |
| V-4 | 同上 `--dir docs/adr` | index apply: 1 written (`docs/adr/README.md`) | 0 |
| V-5 | `node .agents/skills/doc-status/scripts/doc_status.js lint --dir docs/specs` | 184 docs / 173 findings（全て既存由来）。`creative-branch.md` 由来は `inconsistent-reciprocal-relation` warning 1 件のみ（depends-on 先 `vcs-01` が blocks の backlink を持たない既存の非対称。blocking ではない） | 0 |
| V-6 | 同上 `--dir docs/designs` | 192 docs / 252 findings（既存由来）。`creative-branch-design.md` 由来は同上 warning 1 件のみ | 0 |
| V-7 | 同上 `--dir docs/adr` | 54 docs / 14 findings（既存由来）。`cb-00-creative-branch.md` 由来 0 件 | 0 |
| V-8 | `node .agents/skills/adr-doc/scripts/audit_adr.js --dir docs/adr` | cb-00 は required section 欠落 error なし。`depends-on`/`related`/`derives-from` が docs/adr 外を指す旨の warning のみ（audit_adr は ADR dir 内のみを解決する既存ツール仕様。他 ADR も同パターン） | 0 |
| V-9 | `node .agents/skills/adr-doc/scripts/review_adr.js --dir docs/adr` | cb-00 に error なし（出力の error は全て既存 ADR 由来） | 0 |
| V-10 | `node .agents/skills/adr-doc/scripts/check_code_links.js --dir docs/adr` | 初回 `id.rs`/`Cargo.toml`/`Cargo.lock` の bare path warning → 修正後 `crates/...` 表記を module 参照へ変更し cb-00 findings 0 件（同ツールは ADR file dir 基準で path 解決するため repo-root 相対 `crates/` path は構造上 warning になる既存仕様） | 0 |
| V-11 | 既存コード照合（READ-ONLY）: `controller.rs` CAS/`ops.rs` Composite（nested reject 確認）/`id.rs` `define_id!`/`snapshot/types.rs`/`diff/`/`comp.rs`/`midi_comp.rs`/`plugin_state.rs`/`plugin_instance.rs`/`routing*.rs`/`migration.rs`/`store.rs` SCHEMA_VERSION=13 | design §2.1/§3.2/§7.2 の記述と一致することを目視確認 | — |
| V-12 | `route_graph.js`（doc-driven-dev-graph） | `status: blocked`（`artifact-graph`・多数の既存 `broken-relation:*`・`focus-required`）。本 slice 以前からの repo 全体の既存状態であり、新規 3 文書が原因ではない。範囲外のため修復せず記録 | — |

## review-protocol 結果

- 実施: cooperative pass（Issue #277 受入条件 CB-AC-01〜08 と
  spec/design/ADR の網羅性照合）+ adversarial pass（既存コード
  記述の正確性・relation target 実在性・状態語彙の一貫性・
  scope 参照の正確性を反例探索）。Validator=実測 grep/lint で
  各候補を検証、Judge=severity 判定を兼務した separated passes
  （subagent 不使用 — タスク契約「helper なし」に従う）。
- CONFIRMED 修正: design §5 補足の scope overlap 参照を
  `§4.2/§8` → `§8.2 CB-ID-03` へ精緻化（P3 相当の参照精度、
  修正済み）。ADR Implementation Plan の bare path 表記を
  module 参照へ修正（V-10）。
- 残存: P0/P1 blocker なし。P2/P3 は報告のみ（下記未対応参照）。
- 判定: ACCEPT WITH NON-BLOCKING NOTES（レビュー実施 2 pass、
  blocker 0）。

## NotRun / Unsupported（正直な記録）

- **NotRun**: 実装・cargo check/lint/test/build（docs-only、
  `crates/**` 変更禁止のため対象外）。
- **NotRun**: 実 GUI/実 Plugin/実 OS/PCM/RT/AudioLens Consumer
  受入 — CB-11（#288）・#114/#175/#176 系の独立 Gate で評価。
  本 slice では Unsupported ではなく NotRun（機能自体は
  設計済み・未実装）。
- **Unsupported（設計上）**: Semantic Merge/3-way Merge/
  Cherry-pick/部分 Merge/Blend/Mix（初期対象外、design §12）。
  非採用候補の常時 RT 駆動。範囲外 Send/Sidechain/多出力の
  分岐（初期は Reject/Unsupported）。
- **既存 repo 状態**: `doc_status.js lint` の既存 finding
  （173/252/14 件）と `route_graph.js` の blocked は本 slice
  以前からのものであり、範囲外として修復していない。

## Case/Fixture Ledger 参照

- Case ID 体系・Oracle 例・失敗行列: `docs/designs/
  creative-branch-design.md` §8（CB-RCP-01 先頭、CB-SM/CB-ID/
  CB-DOM/CB-PROC/CB-CMP/CB-ADP/CB-GATE）。
- 状態遷移表 T-1〜T-10 と fixture 採取要件: 同 §4.3。
- Fixture の実装は CB-01（#278）以降の Issue が所有
  （docs-only のため本 slice では実行 fixture を作成しない）。
