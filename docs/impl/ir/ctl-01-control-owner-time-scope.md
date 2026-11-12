---
id: "IMPL-034CTL01Kx9wMn2pQ7rVt4yB"
type: "impl"
status: "completed"
title: "CTL-01 ControlOwner/AutomationTimeScope 分離・Schema v7 実装記録"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/121"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/tests/ctl_01.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/ctl_01.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/control.rs"
      - type: "file"
        path: "crates/synvelis-project/src/project.rs"
      - type: "file"
        path: "crates/synvelis-project/src/ops.rs"
      - type: "file"
        path: "crates/synvelis-project/src/migration.rs"
      - type: "file"
        path: "crates/synvelis-project/src/store.rs"
      - type: "file"
        path: "crates/synvelis-project/src/routing.rs"
      - type: "file"
        path: "crates/synvelis-project/src/midi.rs"
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/request.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/snapshot.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/job.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/authorization.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/control.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/gateway_ui.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/controller_store.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/md_a.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/midi_source.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/pc_b_rt.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/routing_sf01.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/pc_a.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/pc_b.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/pc_c1.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/sv47_controls.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/ctl-01-task1-owner-time-scope-schema-migration.md"
  implemented-by: []
  depends-on:
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
    - "docs/adr/ctl-01-control-owner-time-scope-schema-v7.md"
    - "docs/designs/ctl-01-control-owner-time-scope-design.md"
    - "docs/plans/ctl-01-control-owner-time-scope.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/ctl-01-e2e-control-owner-time-scope.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# CTL-01 実装記録: ControlOwner/AutomationTimeScope 分離・Schema v7

## Summary

Issue #121 の中核を実装した。`ControlScope` が兼務していた「パラメータ
の所有者」と「オートメーションの時刻座標」を、型付きの
`ControlOwner` / `AutomationTimeScope` / `ParameterRole` へ責務分離し、
永続スキーマを v7 へ上げた。`synvelis-project` の型・oracle・ops・
migration・評価、`synvelis-workspace` の録音 scope 解決、
`synvelis-gateway` の resource 射影まで一貫して配線し、全 gate
(fmt/check/lint/test/test-rt/build/test-msrv/gw10/test-ui)を
PASS にした(`check-ui` のみ merge 取込み由来の既存型エラーで
FAIL — CTL-01 非起因)。

## Context

- Issue: https://github.com/scarecrow173/synvelis/issues/121
- Spec: `docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md`
- ADR: `docs/adr/ctl-01-control-owner-time-scope-schema-v7.md`
- Design: `docs/designs/ctl-01-control-owner-time-scope-design.md`
- Plan: `docs/plans/ctl-01-control-owner-time-scope.md`
- Task: `docs/tasks/ctl-01-task1-owner-time-scope-schema-migration.md`
- Test spec: `docs/test-specs/ctl-01-e2e-control-owner-time-scope.md`

## Implementation

### 型・スキーマ (`synvelis-project/src/control.rs`)

- `ControlOwner { Project, Track(TrackId), Strip(StripId), Send(SendId),
  Processor(ProcessorId), Clip(ClipId), Note(NoteId) }` を新設。Bus は
  DSP を持たずパラメータは対応 Strip が所有するため、意図的に variant
  を持たない。
- `AutomationTimeScope { ProjectMusical, ProjectAbsolute, ClipLocal,
  NoteLocal }` を新設。`resolve_scope_pos(project, owner, time_scope,
  pos)` として座標解決を分離し、`ProjectAbsolute` は解決器不在を
  `ScopeUnavailable` で正直に報告する。
- `ParameterRole` を新設し、シングルトン役割(Fader/Pan/Mute/Solo/
  SendLevel)と繰り返し役割(Generic/ProcessorParam)を区別。
- `ParameterTarget` は `owner`/`role`/`time_scope` を永続し、
  `ModulationSource.owner` は `ControlOwner` へ移行。
- `ControlOwner` の Deserialize は legacy `{"scope": ...}` タグを受理
  するが、Serialize は新形式のみを出力する。

### Oracle (`control.rs` / `project.rs`)

- `validate_control_owner`(所有者存在)と `validate_control_binding`
  (owner×time_scope 行列・シングルトン `(owner, role)` 一意性)を共有
  oracle として公開し、書き込み時オペレーションと
  `Project::validate_references` が同一ルールを適用する。
- `validate_references` は全 target/source に加え、
  `SendConfig.gain_target` → `ControlOwner::Send(send.id)` の
  owner mapping を検証する。

### Ops (`ops.rs`)

- `AddParameterTarget` は `owner`/`role`/`time_scope` を受け、
  `AddModulationSource` は `ControlOwner` を検証する。
- `AddSend { gain_target }` は採用(adoption)セマンティクス: send ID は
  適用前に存在しないため、検証時はプロジェクト時間座標の既存 target
  のみ許容し、適用時に `owner = Send(send_id)`・`role = SendLevel`
  へ書き換える。他 send 使用中の target は拒否する。
- `RemoveParameterTarget` は lane/assignment/depth source に加えて
  `send.gain_target` 参照を保護。`RemoveSend`/`RemoveConnection` は
  send が所有する target をカスケード削除し、send 所有 target が
  孤立しない(RemoveClip が note を畳むのと同じライフサイクル規則)。
- Strip/Send/Processor 削除経路は routing cascade と連動し、
  dangling owner を残さないことを oracle で担保する。

### Migration (`migration.rs` / `store.rs`)

- `SCHEMA_VERSION = 7`。v6→v7 は `migrate_v6_control` が ID を一切
  割り当てず、`role = Generic`・owner 由来の `time_scope`
  (Clip→ClipLocal, Note→NoteLocal, その他→ProjectMusical)を決定的に
  投影する。`id_watermark` 不変。
- v6 以前に存在しない Strip/Send/Processor owner を持つ body は
  foreign state として `Error::Validation` で拒否する(v5→v6 の
  routing state 拒否と同じ規則)。

### 評価・Workspace

- `eval_inner` / `PreparedTarget` / compile / RT 評価は owner と
  time_scope を分離して保持し、clip/note の placement 解決は
  従来通り live。
- `synvelis-workspace`: 録音は `resolve_scope_pos(owner, time_scope)`
  で解決し、`ControlSession::target_scope` を
  `(ControlOwner, AutomationTimeScope)` を返す `target_binding` へ
  置き換えた。

### Gateway

- `ResourceKind`/`ResourceRef` に `Strip`/`Send`/`Processor` を追加し、
  parse/expect/kind/列挙(13種)/属性/メタデータ/prepare の tagged
  射影を拡張。`All` selector が新 kind を含む。
- パラメータのメタデータは `owner`/`role`/`time_scope` を型付き
  resource 参照として射影する。`strip_of_processor` を pub 化して
  Processor→Strip 導出を共有実装にした。

## Related Experiments

### Adopted

なし — `metadata.experiments.adopted` は空で、関連する
`docs/impl/exp/*.jsonl` 記録もない。

### Rejected

なし — `metadata.experiments.rejected` は空。

## Validation

実行環境: Windows、toolchain pin `rust 1.99.0`、MSRV `rust 1.89.0`。
すべて `mise run <task>` 経由。`origin/main` (`4c536b0`) マージ後に
再検証した。

| Gate | 結果 |
| --- | --- |
| `mise run fmt-check` | PASS (exit 0) |
| `mise run check` | PASS (exit 0、warning なし) |
| `mise run lint` | PASS (exit 0、`-D warnings`) |
| `mise run test` | PASS (exit 0、74 test binaries 全て ok、ctl_01: project 14 + gateway 2) |
| `mise run test-rt` | PASS (exit 0、rt_ filter) |
| `mise run build` | PASS (exit 0) |
| `mise run test-msrv` | PASS (exit 0、rust 1.89.0 全 workspace test) |
| `mise run check-ruviform-rev` | PASS (8fb665f1、22 lockfile sources、REV match) |
| `mise run check-headless-deps` | PASS (8 headless crates に GUI 依存なし) |
| `mise run lint-actions` | PASS (actionlint) |
| `mise run gw10` | PASS (G0–G6・GTV 全 PASS; SYS-02-ux/SYS-03-ux/SYS-05-powerloss/CONSUMER-* は NOT-RUN、SYS-07-rt は UNSUPPORTED — いずれも従来通り非ブロッキング) |
| `mise run test-ui` | PASS (vitest 37 tests) |
| `mise run check-ui` | FAIL — `ui/test/binding.test.ts` の TS 型分散エラー。`origin/main` マージ取込みの SV-GUI-02 lane 由来で CTL-01 非起因(本変更は ui/ 非接触)。対象 Issue 側で対応要。 |

## Risks

- `check-ui` の TS 型エラーは merge 取込みの既存問題(SV-GUI-02
  lane 由来)で CTL-01 非起因だが、最終マージ前に対象 lane での
  解消確認が望ましい(検証結果に記録済み)。
- 永続スキーマを v7 へ bump した — v6→v7 は `migrate_v6_control`
  の決定的投影(ID 非割当・`role = Generic`・owner 由来
  `time_scope`)で実装済みだが、v6 以前に存在しない owner を持つ
  body は foreign state として拒否される。Issue 記載の
  Acceptance 依存項目(#122/#108/#102・Undo/Save/VCS・AudioLens
  回帰)は本スコープ外であり、Issue #121 の Acceptance 完了は
  それらの lane 完了に依存する。

## Follow-ups

- #122/#108/#102・Undo/Save/VCS・AudioLens 回帰は Issue 記載の
  Acceptance 依存であり本スコープ外。
- `check-ui` の型エラーは merge 取込みの既存問題。CTL-01 起因ではない
  が、最終マージ前に対象 lane での解消確認が望ましい。
