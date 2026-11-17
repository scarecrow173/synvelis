# sv513 acceptance ledger — variant/VCS apply op surface 拡張 (Refs synvelis#513)

Date: 2026-11-20. Branch: feat/513-variant-apply-ops. Lane: headless.

## Issue 受入条件 mapping

| AC | Disposition | Evidence |
| --- | --- | --- |
| `Removed` cascade apply | **Unsupported 維持**（ADR D1）— cascade 包含検査が lease 外の restore/journal 層契約に波及するため fail-closed 側に確定 | `docs/adr/issue-513-variant-apply-ops.md` D1; `removed_change_is_unsupported` pin 非回帰 |
| comp/clip domain typed change replay | **実装** — whole-entity transplant（ADR D2）。`supported_kind`+`entity_value`+write ordering 拡張で `apply_selected`/`target_fingerprint` が comping 4 kind を replay | `comping_changes_replay_through_apply_selected` (rcd_04) Pass; ledger VCS-APPLY-10 Blocked→Pass |
| candidate 内 plugin binding/preset 差替 | **binding: 実装** `ProjectOp::SetProcessorBinding`（ADR D3）。**preset: Unsupported 維持**（plugin 内部 opaque state、typed seam なし） | `cb_inst_plugin_binding_swap_is_scoped_typed_and_identity_synced` (cb04) Pass; capability 報告=強制一致維持 |
| set 内 branch 追加 op + N>2 + CB-MIDI-01 literal lane | **実装** — `VariantOp::AddBranch` + `variant::add_branch`（ADR D4）。同時 3 candidate の fork→edit→adopt を headless 検証 | `cb_midi_01_literal_three_simultaneous_candidates` (cb03) Pass — B の編集が A/C/live/raw native source を変更しない |
| routing 候補 op（RemoveProcessor/ReorderProcessor/send/sidechain/multi-output/mid-chain end） | **Unsupported 維持**（ADR D5）— main-path connection confinement の安全モデル未設計、CB-FX-04 不変条件を守るため op surface を広げない | capability 報告の `Unsupported` エントリ非回帰; `docs/adr/` D5 |
| `mise run test synvelis-project` 等の正規検証 pass・cb01〜cb11/vcs 系非回帰 | 検証結果は PR 本文に記録 | — |
| headless fixture lane Pass 判定・実機 lane NotRun 記録 | 本ファイル + PR 本文 | 下記 lane 簿記 |

## Lane 別 Pass/NotRun 簿記

| Lane | Status | Note |
| --- | --- | --- |
| headless fixture（cargo/mise test、synvelis-project） | **Pass** | canonical `mise run test synvelis-project` の結果は PR 本文に実測記録 |
| 実 OS（Windows 実機 DAW 起動・plugin host） | **NotRun** | owner gate: synvelis#288 系。headless Pass を実機 Pass に昇格しない |
| 実 Plugin（VST3/AU host 内 plugin state） | **NotRun** | owner gate: ruviform#630/#631 系。preset 差替は `Unsupported` のまま |
| 実 Render（audio render 出力） | **NotRun** | owner gate: synvelis#114 系 |
| 実 GUI（editor 操作・approval UX） | **NotRun** | owner gate: synvelis#288 系 |
| POSIX FS journal/restore fault lane | **NotRun** | 既存 `#[cfg(windows)]` pin のみ — 本 slice で変更なし |
| 3-way semantic merge / VCS Write 本体 | **Unsupported/対象外** | Issue「対象外」維持 |

## 検証証拠（補助 — cargo 直接実行）

- `cargo test -p synvelis-project --test vcs07_variant_diff` → 8/8 Pass
- `cargo test -p synvelis-project --test rcd_04_comp_cas` → 8/8 Pass
- `cargo test -p synvelis-project --test cb03_midi_clip` → 13/13 Pass
- `cargo test -p synvelis-project --test cb04_instrument_slot cb_inst_plugin` → 1/1 Pass
- canonical `mise run fmt-check/check/lint/test synvelis-project`:
  PR 本文にコマンド・exit code・結果を記録。
