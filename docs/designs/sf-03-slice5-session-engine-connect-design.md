---
id: "DESIGN-034Uj7LfhkkACvyn53L7eN"
type: "design"
status: "approved"
title: "SF-03 slice5 設計 — session EnginePort・SessionRoutingEngine・canonical probe"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/106"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-03-slice5-session-engine-connect.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sf-03-slice5-session-engine-connect.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-03-slice5-session-engine-connect.md"
  derived-by:
    - "docs/plans/sf-03-slice5-session-engine-connect.md"
  verifies: []
  verified-by: []
  references:
    - "docs/designs/sf-03-slice4-act-acceptance-design.md"
  defers: []
  deferred-by: []
---

# SF-03 slice5 設計 — session EnginePort・SessionRoutingEngine・canonical probe

## Context

`ProjectSession`（synvelis-workspace）は `EngineStub` を engine として
保持し、committed revision ごとに `graph_builds` を増やすだけで
実 routing には接続していない。`RoutingEngine`（synvelis-player）は
slice 1–4 で semantic lowering・compile・publish・activation
receipt・last-good・reconcile・`ProcessorHost` 境界を完備している。
本 slice はこの 2 者の間に session 向け接続経路を作る。

依存方向の制約: `synvelis-workspace` は `synvelis-player` に依存
しない（session 基盤が media decode/plugin host を引き込まない）。
したがって trait は workspace 側に置き、`RoutingEngine` を内包する
実装は上位の composition root である `synvelis-app` に置く。

## Approaches（検討した設計）

### engine 保持の置き換え方

- **A: `ProjectSession` が `RoutingEngine` を直接所有** — workspace→
  player の新規依存が必要で、media/plugin 系を session 基盤へ
  引き込む。session の公開契約（`engine()` が stub を返す既存 test）
  も壊れる。却下。
- **B: `EnginePort` trait + 既定 `EngineStub` + app 側注入** —
  workspace は trait 契約だけを知り、player 依存を持たない。
  既存 test は `graph_builds()` 観測を維持できる。採用。

### `EnginePort` の通知内容

engine が build 判定に必要なのは committed `&Project` と
`ProjectRevision`。view/layout/focus は `Project` に含まれないため
`(&Project, revision)` の通知だけで semantic 判定は完結する
（equivalence は engine 内部の fingerprint が判定）。trait は
object-safe に保ち、証拠観測は `graph_builds()`/`last_built_revision()`
の 2 accessor に限定する（activation 等の詳細証拠は具象 adapter 側の
API で提供）。

### `SessionRoutingEngine` の構造

`RoutingEngine` は `!Send`（plugin host・RT 状態）なので、session と
テストが同一 engine を観測できるよう `Rc<RefCell<Inner>>` ベースの
cloneable handle とする。`EnginePort` impl は handle が inner を
borrow して `on_project_revision` を実行する。`Inner` は
`RoutingEngine`・設定（media_root/engine config/prefetch/budget/
instance/epoch/generations）・`next_request_id`・通知カウンタ・
`last_prepare_error` を保持する。

### `RoutingDeps` の組み立て

`deps.clips` は lowering が生成する playback 順と一致必須。
`plan_playbacks(project, canonical_of, engine_config)` を adapter が
先に呼び、その `PlaybackPlan` 列で `resolver::prepare` して
`PreparedClip` を作る。engine 内部の lowering は同じ
`canonical_of`（per-build cache）と同じ engine config で再計算する
ため決定的に一致する。

`canonical_of` は `Option<AudioFormat>` を返す必要があるが、
`resolver::prepare` は `PreparedClip`（prefetch 資源込み）を返すため
probe には重い。player に `resolver::canonical_format`（verified
open を `prepare` と共有する小さい probe）を追加し、adapter は
per-build の `RefCell<BTreeMap<SourceId, Option<AudioFormat>>>`
cache 経由で `canonical_of` を供給する。cache は build ごとに新規
なので project snapshot 内での digest 変更は起きず、probe 結果は
その build に対して正直。

### generation の扱い

- `engine_config_generation`: adapter 設定の engine geometry が
  変わらない限り固定値（既定 1）。geometry 変更は adapter 再生成で
  表現する。
- `source_format_generation`: build ごとに consult された
  `(SourceId → AudioFormat)` map を保持し、前回 map と差があれば
  +1 する。「この build が消費した canonical format の世代」という
  key 契約に忠実で、未参照 source の変化は build key に影響しない。

### 失敗の扱い

- `engine.prepare`/`publish` の `Err(RoutingFault)` は engine が
  receipt に記録済み。adapter は追加処理しない（last-good は engine
  契約）。
- adapter 側の `resolver::prepare` 失敗（probe 成功後の稀な失敗）は
  `deps.clips` を完成できないため engine を呼ばず、
  `last_prepare_error: Option<SessionEngineError>` に記録して
  return する。live plan は engine 側で untouched のまま残る。
- `Equivalent` は publish せず receipt 維持（engine 契約）。

### session 側の変更

- `EnginePort` trait を `session.rs` に追加
  （`on_project_revision(&mut self, project: &Project, revision)`・
  `graph_builds`・`last_built_revision`）。
- `ProjectSession.engine: Box<dyn EnginePort>`、既定
  `EngineStub`、`set_engine` で注入。`apply_internal`/`undo`/`redo`
  は `self.controller.live()` と revision を渡す。
- `EngineStub` は trait impl を持ち、pub field 観測は維持。
  既存 test の `.graph_builds` field アクセスは `.graph_builds()`
  メソッド呼出しへ機械的に追従。

### app 側の変更

- `src/engine.rs` 新設: `SessionEngineConfig`・
  `SessionEngineError`・`SessionRoutingEngine`（Clone handle）。
  `EnginePort` impl + `set_processor_host`/`run`/`receipt`/
  `reconcile`/`rt_status`/`source_map`/`take_processor_control`/
  `last_prepare_error` の公開面。
- `app.rs` に `attach_routing_engine` を追加し、現在の session に
  engine を接続する最小の composition 経路を提供。
- `AudioRuntimeSlot`・service op・GUI 経路は変更しない。

## Components and Boundaries

| 層 | 変更 | 責務 |
| --- | --- | --- |
| synvelis-workspace `session.rs` | `EnginePort` trait・`Box<dyn EnginePort>` 化・通知に `&Project` 追加 | session の engine 境界契約。player 非依存を維持 |
| synvelis-workspace tests | `.graph_builds` → `.graph_builds()` | 観測契約の追従 |
| synvelis-player `resolver.rs`/`lib.rs` | `canonical_format` probe 追加 | `prepare` と同一 verified-open で canonical format のみ取得 |
| synvelis-app `engine.rs`（新規） | `SessionRoutingEngine` 等 | `EnginePort` 実装・`RoutingDeps`/request 組立・engine 公開面 |
| synvelis-app `app.rs`/`lib.rs`/`Cargo.toml` | `attach_routing_engine`・module export・deps 追加 | composition root としての接続点 |
| synvelis-app `tests/sf03_session_engine.rs`（新規） | e2e | publish→Active→last-good→equivalence→stale の session 経路実測 |

## Data and Control Flow

```
session.apply/undo/redo (commit 成功)
  → engine.on_project_revision(&Project, revision)   [EnginePort]
    → graph_builds+=1, last_built_revision=revision
    → per-build canonical_of cache + plan_playbacks
    → source_format_generation 更新
    → resolver::prepare per plan → PreparedClip列
       (失敗 → last_prepare_error, engine untouched)
    → RoutingCompileRequest{next_id, revision, Realtime, budget, instance, epoch}
    → RoutingEngine::prepare(request, project, revision, deps)
       Built → publish(prepared, project, revision)
       Equivalent → 何もしない（receipt 維持）
       Err(fault) → engine が Failed receipt 記録（last-good 維持）
```

activation は別途 `run(blocks, policy)` が RT generation を observe
したときのみ `Active` receipt になる。session 経路は publish までを
同期駆動し、RT 駆動は呼出し側（test/将来の audio callback）が行う。

## Non-Functional Constraints

- headless: `synvelis-app` の default feature で GPUI/WebView/
  GPU 系を引き込まない。`SessionRoutingEngine` は `#[cfg]` なしの
  通常モジュール。
- RT 安全性: `on_project_revision` は非 RT 文脈でのみ呼ばれる
  （session apply は UI/command thread）。file I/O・prefetch 準備・
  compile は既存どおり prepare 側の責務。
- 単一 writer: project 変更は `ProjectController` 経由のみ。
  `controller_mut()` による bypass は engine 通知を伴わないため、
  stale 検出は engine の reconcile/key 照合に委ねる（既存契約）。
- 時間演算: Tick/TickDuration の checked 演算規約に従う
  （本 slice で時間演算の追加はない見込み）。

## Risks and Trade-offs

- `plan_playbacks` を adapter と engine lowering で 2 回呼ぶ →
  決定的で canonical_of cache により probe も 1 回。コストは
  prepare 相当の非 RT 処理で許容。
- `canonical_of` probe 失敗（None）は `MissingSource` skip として
  engine lowering が fault 化 → adapter は clip を作らず整合。
- `EnginePort` を `&Project` 受け取りにすると EngineStub 実装は
  project を無視するだけ → 既存挙動と同等。
- `Rc<RefCell>` handle は `!Send` → session は既に単一 thread 前提
  （ControlSession の共有 cell と同じ前提）。
- `synvelis-app` への `synvelis-player`/`ruviform-media-reader`/
  `ruviform-audio-core` 依存追加 → headless deps 検査で GUI 混入が
  ないことを確認する。

## Deferred Design Concerns

- media_root/engine config のプロジェクト単位決定規則、
  `AudioRuntimeSlot` への実接続、実デバイス clock epoch の供給 |
  GUI/実機 slice | 実機検証タスク | session 接続だけでは実再生に
  至らない。
- `syn.*` service op としての routing 状態公開 | GUI slice で op
  面を設計 | GUI slice | headless は session 直結で検証済み。
- 複数 session/engine の同時保持・project 切替時の世代連続性 |
  現状 session ごとに engine を再 attach する運用で回避 | 複数
  project 同時再生が必要になった時点 | 世代衝突の可能性。

## Verification Notes

- `synvelis-app/tests/sf03_session_engine.rs` で
  session.apply→publish→run→Active、失敗→Failed+last_good、
  rename→Equivalent、stale revision→拒否/reconcile mismatch を
  実測する。
- 既存 workspace test は `.graph_builds()` 追従のみで挙動不変を
  確認する。
- `check-headless-deps` で app default の依存閉包に GUI 系が
  入らないことを確認する。
