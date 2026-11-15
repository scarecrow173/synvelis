---
id: "IMPL-034XJHWG7pGSDfwCv6AgS7"
type: "impl"
status: "completed"
title: "SV-GUI-03-07 keymap dispatch"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/112"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/src/keymap.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/sv112_keymap.rs"
      - type: "file"
        path: "docs/impl/ir/sv-gui-03-07-keymap-dispatch.md"
      - type: "file"
        path: "docs/impl/exp/sv-gui-03-07-keymap-dispatch.md"
      - type: "file"
        path: "evidence/sv112/keymap-dispatch-verification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/command.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/workspace.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-03-06-keyboard-gesture.md"
    - "docs/designs/sv-gui-03-06-keyboard-gesture-design.md"
    - "docs/test-specs/sv-gui-03-06-keyboard-gesture.md"
    - "docs/impl/ir/sv-gui-03-06-keyboard-gesture.md"
    - "docs/impl/exp/sv-gui-03-07-keymap-dispatch.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/sv-gui-03-06-keyboard-gesture.md"
  verified-by: []
  references:
    - "evidence/sv112/keymap-dispatch-verification.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/sv-gui-03-07-keymap-dispatch.md"
    rejected: []
---

# SV-GUI-03-07 keymap dispatch

## Summary

Issue #112(SV-GUI-03 tracker)の keymap dispatch slice として、
merge 済み sv-gui-03-06 contract(design §3.1 catalog・§3.2
KG-KEY-01〜15 割当表・§3.3 OS 正規化・§4.1 発火行列・§5 安全
契約・§6 メニュー網羅)の headless 実装を `synvelis-workspace`
に追加した。新規 `src/keymap.rs` が物理 key stroke → 論理
chord 正規化 → binding 照合 → §4.1 lane×`InputContext` ゲート
→ gesture-preview ゲート → `CommandContext` 構成 →
`WorkspaceManager::dispatch` までの §4.2 pipeline を担い、
`command.rs` `spec_for` に contract 14 id を登録、
`workspace.rs` `dispatch` に transport toggle・viewport・
edit.delete の実行経路と save/export/stop-return/record/
copy/paste の正直な `Unresolved` を追加した。headless 契約
試験 TS-EX-01〜11 を `tests/sv112_keymap.rs` で実走し全件
Pass。実 OS/実 GPUI/実 IME/実 Plugin GUI 検証は #114/#176
lane として NotRun/Deferred を明示する。

## Context

- Contract: `docs/designs/sv-gui-03-06-keyboard-gesture-design.md`
  (§3.1 contract command catalog・§3.2 既定バインド表・§3.3
  `Mod` OS 正規化・§4.1 発火行列・§4.2 pipeline・§4.5 outcome
  正直さ・§5 安全契約・§6 メニュー網羅・§8 KG-D-1〜8)
- Spec: `docs/specs/sv-gui-03-06-keyboard-gesture.md`
  (KG-R-01〜07・KG-AC-01〜07・Deferred Design Concerns)
- Test spec: `docs/test-specs/sv-gui-03-06-keyboard-gesture.md`
  (TS-EX-01〜11 が本 slice の headless 試験仕様、TS-GUI-01〜03
  は #114/#176 委譲)
- Contract chain ir/exp: `docs/impl/{ir,exp}/
  sv-gui-03-06-keyboard-gesture.md`(slice6 docs-only、
  completed — 本 slice がその実装側)
- Issue: https://github.com/scarecrow173/synvelis/issues/112
  (tracker — auto-close しない。実装 slice は Coordinator
  dispatch spec に基づく)
- Base: `origin/main` `194436b`(sv-gui-03-06 contract chain
  merge 済みを確認)

## Implementation

### Changes

#### Added

- `crates/synvelis-workspace/src/keymap.rs`(768 行)— keymap
  層。`OsFamily`(Win/Linux=Ctrl・macOS=Cmd の `Mod` 正規化)・
  `PhysicalModifiers`(bitmask)・`Key`/`KeyChord`/
  `LogicalModifiers`/`KeyStroke` の論理キー語彙・
  `Keymap::standard()` の §3.2 既定バインド表(KG-KEY-01〜15、
  `project.export`・`view.activate`・`project.rename`・
  `clip.mute`・`clips.delete-range` は既定バインドなし)・
  `dispatch_key` の §4.2 pipeline・`dispatch_escape` の §5.1
  transient 取消・`try_bind` の KG-C-5 リマップ競合拒否
  (`BindRejection::{Conflict,OsReserved,UnknownCommand}`)・
  `menu_coverage` の §6.2 coverage inventory・
  `accelerator_label` の CM-2 表記写像。
- `crates/synvelis-workspace/tests/sv112_keymap.rs`(978 行)—
  TS-EX-01〜11 の headless 契約試験。`ws_a.rs`/`ed_a.rs` の
  fixture 構造を踏襲。
- `docs/impl/exp/sv-gui-03-07-keymap-dispatch.md` — 実装時の
  探索ログ(keymap 層ゲート配置・`PhysicalModifiers` bitmask
  化・`edit.delete` domain 解決・TS-EX-10 境界)。
- `evidence/sv112/keymap-dispatch-verification.md` — 実行
  コマンド・OS/toolchain・exit code・TS-EX-01〜11 突合・
  Deferred 記録(新規ファイル。既存 `case-ledger.md`/
  `verification.md` は slice6 docs-only 証拠として不変)。

#### Modified

- `crates/synvelis-workspace/src/command.rs` — `spec_for` に
  contract 14 id を lane どおり登録: `edit.delete`/`edit.copy`/
  `edit.paste`(Editing)・`project.save`/`project.export`
  (Project)・`transport.toggle`/`transport.stop-return`/
  `transport.record`(Transport)・`view.zoom-in`/`view.zoom-out`/
  `view.scroll-{left,right,up,down}`(View)。語彙コメントを
  contract id の挙動に合わせて更新。
- `crates/synvelis-workspace/src/workspace.rs` — `dispatch` の
  kind 別分岐を拡張: Transport に `toggle`(live
  `session.transport().playing` で play/stop 解決)・
  `stop-return`/`record`(session API 不在 → `Unresolved`)・
  未知 id の `Rejected{UnknownCommand}`;View に viewport 系
  6 id の `dispatch_viewport` 委譲;Project に save/export の
  `Unresolved`;Editing に `edit.delete` の `dispatch_edit_delete`
  委譲と copy/paste の `Unresolved`。helper 2 本追加
  (`dispatch_edit_delete`・`dispatch_viewport`)。
- `crates/synvelis-workspace/src/lib.rs` — `pub mod keymap;`
  と keymap 公開型の re-export。
- `docs/impl/{ir,exp}/README.md` — `sync_impl_index.js`
  による生成 index 再生成(手編集なし)。

### Decisions During Implementation

- **suppression 配置 = keymap 層ゲート(KG-D-4 の選択)**:
  §4.1 行列を `keymap.rs` の `lane_fires` 1 箇所に集約。
  `dispatch` 側の既存 suppression(Project/Transport/Editing
  の `captures_input`)は非 keymap caller のために維持し、
  両者は構成上一致(コメント明記)。`dispatch` の suppression
  集合を拡張する案は Undo lane の context 別配送
  (`dispatch_undo`)と二重化・誤抑制の危険があるため棄却。
  Undo lane は keymap 層で Modal/PluginGui 抑制のみ行い、
  配送自体は `dispatch_undo` に委譲(design §4.3)。
- **`PhysicalModifiers` は bitmask**: struct 4 bool 案は
  clippy `struct_excessive_bools` に抵触したため、既存
  repo 慣行に合わせ bitmask(`u8`・`|`・`contains`)へ。
  `KeyChord::normalize` は「論理語彙で表現不能な修飾
  (非 primary の Ctrl/Cmd)を含む stroke」を `None` で
  reject — OS 予約判定は論理 chord を物理へ展開して評価。
- **`edit.delete` の domain 解決**: explicit_targets →
  selection snapshot → origin view active domain の順で解決
  し、`Clips` のみ `id="clip.remove"` に書換えて
  `self.dispatch` へ再入(対象解決・domain 検査・1 undo 単位
  が `clip.remove` と同一経路)。他 domain は推測せず
  `Unresolved{reason}`。
- **未実装経路は全て正直 `Unresolved`**: `project.save`/
  `project.export`(workspace 層に save/export op なし・
  `persist.rs` は WorkspaceSnapshot のみ)・`transport.
  stop-return`(playhead return API なし)・`transport.record`
  (#12 状態機械が正本・session API なし)・`edit.copy`/`edit.
  paste`(clipboard lane なし)。fake success・no-op success
  にはしない(§4.5)。
- **viewport は view-local・revision 中立**: `Viewport` の
  scroll/zoom を origin view のみ変更し
  `Applied{revision: None}`。origin view 未解決・未知 view
  は `Unresolved` で、他 view への推測 fallback なし(§5.2)。
  zoom 係数 1.25・scroll step 64px は契約非固定の実装定数。
- **`Esc` は完全 bare のみ reserved**: `PhysicalModifiers::
  NONE` の Esc のみ transient-cancel。修飾付き Esc は普通の
  chord。gesture 3 系統(`gesture`/`control_edge_gesture`/
  `routing_gesture`)は view ごと相互排他なので決定的
  tiebreak で `CancelReason::Escape` を各 cancel API へ。
  transient 無しは `NoTransient` no-op(dock drag/modal は
  本層の外)。
- **auto-repeat は契約 id 単位**: `view.scroll-*`・
  `edit.undo`・`edit.redo` のみ repeat を受理、他は
  `RepeatIgnored`(§5.3・KG-D-5)。
- **TextInput/Ime の表現**: §4.2 step1「印刷可能文字キーは
  text/IME へ先配送」を headless 層では「TextInput/Ime で
  primary modifier 無しの stroke は `TextLane`、Mod 付き
  combo のみ keymap 照合」として実装(TS-EX-04 と整合)。
- **TS-EX-10 の境界**: menu model 本体は `synvelis-app` の
  別 slice。keymap 層では §6.2 coverage inventory
  (`menu_coverage` — 全 contract id の menu 面対応+無配置
  理由)と accelerator 表記(`accelerator_label`)のみ提供し、
  menu-model 依存部分は Deferred として記録。

## Related Experiments

### Adopted

- `docs/impl/exp/sv-gui-03-07-keymap-dispatch.md` —
  「keymap 層ゲート 1 箇所集約 + `dispatch` 既存 suppression
  維持」「`PhysicalModifiers` bitmask」「`edit.delete`→
  `clip.remove` 再入」「未実装経路は `Unresolved`」の各
  判断を採用。

### Rejected

- `dispatch` suppression 集合拡張案 — Undo lane の
  context 別配送と二重化・誤抑制リスク。
- `edit.delete` を `dispatch` 内 domain 分岐で実装する案 —
  既存 required_domain 契約と責務重複。
- fake success での save/export/stop-return/record/copy/
  paste 実装 — §4.5 違反。

## Validation

`evidence/sv112/keymap-dispatch-verification.md` に全記録。
概要(全て exit 0、Windows/MSYS・rustc 1.99.1):
`mise run fmt-check`・`mise run check synvelis-workspace`・
`mise run lint synvelis-workspace`・`mise run test
synvelis-workspace`(sv112_keymap 11 件含む全 test Pass)・
`mise run test-rt`(workspace 全体 rt_ 契約)・`mise run
check-headless-deps`・`mise exec -- prek run --all-files`・
`node .agents/skills/impl-doc/scripts/audit_impl_record.js
--json`・`sync_impl_index.js --kind ir/exp --apply`。
実機/実 GPUI/実 OS/実 IME/実 Plugin GUI は NotRun(#114/
#176 lane)。

## Risks

- View/Application lane ゲートが keymap 層にのみ存在し、
  keymap を経由しない `dispatch` caller は §4.1 行列の
  View/Application 抑制を受けない → 現状 keymap が唯一の
  key→command 入口であり、`dispatch` は command API として
  contract どおりの suppression を既に持つ範囲で維持。
  将来別の key 入口が増えた場合は同じ `lane_fires` 相当を
  適用する必要あり。
- `edit.delete` の domain 解決は #11/#12 語彙確定待ちの
  暫定写像(Clips のみ)。他 domain 語彙が来たら拡張。
- `PhysicalModifiers` は物理修飾の bitmask で、非 US 配列の
  物理キー名解決は adapter 側の後続 slice。
- zoom/scroll の step 定数は契約非固定の実装判断 — UX 確定
  時に調整余地あり。
- menu coverage はデータ+検証のみで menu model との結線は
  未実施 → synvelis-app slice で配線時に再検証要。

## Follow-ups

- #114 GUI-L2: 実 GPUI key dispatch・実 OS 既定バインド・
  実 IME/実 Plugin GUI キー所有・OS 予約キー実機衝突
  (TS-GUI-01〜03)。
- #176 EXT-L3: 実 Plugin GUI でのキー所有検証。
- 後続 slice: リマップ永続化・設定 UI・chord/多段キー・
  非 US 配列の物理キー名解決・menu model 本体と accelerator
  表示配線(synvelis-app)・`project.save`/`export` の
  workspace/app op 実装・`transport.stop-return`/`record`
  の session API(#12 状態機械)・clipboard lane。
- docs index は `sync_impl_index.js` で生成済み。
