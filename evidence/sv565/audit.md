# sv565 audit — SetProcessorBinding `sync_processor` PluginInstanceId 退役 cascade

対象: `scarecrow173/synvelis` issue `#565`（Refs のみ、close 判定は Coordinator）
ブランチ: `chore/565-processor-binding-audit`（base `b73018bd` = origin/main）
監査日: 2026-11-18

## 監査対象と方法

`SetProcessorBinding` 実行時に旧 `PluginInstanceId` が退役・
置換される経路を main コードで再確認し、persisted identity の
不変条件 3 軸（再読み込み・復元・外部参照）への影響を照合
した。コード参照は全て base SHA `b73018bd` 時点。

## 退役経路（確認済み）

`ProjectOp::SetProcessorBinding` の apply arm
（`crates/synvelis-project/src/ops.rs` — `proc.binding` 差替後に
`project.plugin_instances.sync_processor(proc)?` 呼出）→
`PluginInstanceIdentities::sync_processor`
（`crates/synvelis-project/src/plugin_instance.rs`）が
`Resolved(plugin)` → `ensure`、`Unresolved` → `remove` に分岐。

- `ensure`: entry の `PluginRef` が一致すれば id 維持、
  descriptor/version/`capability_revision` のいずれかが異なれば
  `allocate` で新 id 発行（rebind = 新 authored identity）。
- `remove`: entry 削除。watermark は後退しないため発行済み id
  は二度と再発行されない（`allocate` の単調 watermark）。
- 同一 primitive を共有する経路: `InsertProcessor`・
  `RemoveProcessor`（`remove` 直接）・VCS apply
  `ObjectKind::Processor` 書込み（`apply/entity.rs`）・
  v12→v13 backfill（`migration.rs`、`ProcessorId` 順で決定的）。
- post-apply oracle `validate_against` は
  `Project::validate_references`（`project.rs`）内で
  `ProjectController::apply` と `load` の双方から実行され、
  store が `routing.processors` の全射的投影であること
  （全射・id 範囲・一意性・plugin 一致・Unresolved 非所持）を
  検査する。

## 不変条件 3 軸の照合結果

### (1) 再読み込み — 健全

- schema v13 の `Project::plugin_instances` が persisted
  authority。serde default で旧 schema も deserialize 可能。
- v12→v13 backfill は同一 `sync_processor` primitive を再利用し
  `ProcessorId` 順の決定的発行で冪等。
- save→load roundtrip・oracle 拒否系は既存 pin
  （`plugin_instance_persist.rs`）が担保。
- 本 slice で `SetProcessorBinding` op 経路の pin を追加
  （apply 経路 pin は既存、op 経路は未 pin だった）。

### (2) 復元 — 健全（fail-closed）

- `resolve` は missing entry・`PluginRef` drift
  （`capability_revision` 含む）・範囲外 id を全て `None` に
  畳み、「missing」と「stale」を区別しない。
- 消費側は typed fail-closed: player
  `crates/synvelis-player/src/routing_control.rs` は `None` →
  `without_control` evidence、project
  `crates/synvelis-project/src/host_param.rs` は `None` is data
  で `PluginMismatch` が instance 比較より先に報告される
  ordering を明記。
- undo/redo は project 全体 snapshot を復元するため store も
  同 revision の状態に戻る。

### (3) 外部参照 — dangling は構造的に起きない

- `PluginInstance` は wire `ResourceKind`（16 variants）にも
  `ENUMERATED_KINDS`（15 kinds）にも非存在 — `ResourceRef`
  で名指せず、op target/precondition/effect resource として
  外部から参照する経路が存在しない。
- `op_facts`/`derive_effects` も同じ観測モデルを使うため退役
  そのものは effect に現れない。ただし `Processor` の
  `detail_of` は `binding` フィールドを含む `Processor` 全体を
  serialize するため、rebind 自体は `Processor` `Write` 効果の
  before/after detail で観測可能 — `insert-processor` の
  「ports/plugin-instance identity は internal structure」
  posture と整合。
- VCS diff/apply 面: `ObjectKind::PluginInstance` は
  `plugin_instances.instances` を `ProcessorId` key の
  keyed-synthetic collection として比較（`watermark` は
  allocator bookkeeping で比較対象外）。VCS adopt の `Removed`
  は typed `Unsupported` fail-closed。`write_plugin_instance`
  の transplant は byte-faithful + watermark の
  `max(live, after)` 単調 reseed。
- variant 経路も同じ primitive に収束: `SetProcessorBinding`
  は instrument-slot/processing-range/strip-Inserts mask の
  candidate edit op として受理され、適用は `ProjectOp::apply`
  → `sync_processor`。adopt は `plan_adopt_writes` が scope
  object のみを write 計画し `ObjectKind::PluginInstance` を
  含まないため、Processor write → `sync_processor` のみで
  store を live 側で再導出する（candidate が mint した id は
  transplant されず、`ensure` が live store で再発行 —
  plugin 不変なら live id 維持、rebind なら新 live id。
  candidate↔live の id 乖離は意図された semantics）。
  `write_plugin_instance` の byte-faithful transplant +
  watermark `max(live, after)` 単調 reseed が走るのは VCS
  `apply_selected`/`write_all` が `PluginInstance` change を
  明示選択した経路のみ。

## 隣接発見（本 slice 範囲外・follow-up 要）

review-protocol の adversarial review で、監査 charter
（`plugin_instances` store の `PluginInstanceId` 退役 cascade）
の範囲外だが同じ「rebind で persisted plugin identity が
stale 化する」クラスの既存問題が 2 件確認された。いずれも
main 既存で本 slice が導入したものではなく、修正は別 Issue
での検討事項として記録する。

### ADJ-1: `SetProcessorBinding` が `SemanticPort.binding` を更新しない（P2）

`SetProcessorBinding` の apply arm（`ops.rs`）は `proc.binding`
と `plugin_instances` のみを更新し、processor の
`SemanticPort.binding`（`PortBinding::PluginDeclared` /
`Unresolved`、`InsertProcessor` 時に `processor_port_binding`
で一度だけ導出・persisted）は rebind 後も旧 plugin のまま残る。
影響:

- `Resolved(A)`→`Unresolved` rebind 後も port は
  `PluginDeclared` のまま → `routing_tx.rs` の
  `resolved_endpoints` が設計した `UnresolvedPort` 拒否を
  迂回する（同一論理状態が `InsertProcessor`+`Unresolved`
  経路では `PortBinding::Unresolved` port として拒否される
  のに対し、rebind 経路では受理される — 2 経路で発散）。
- `Resolved(A)`→`Resolved(B)` rebind で port stamp の
  `plugin`/`capability_revision`/`advertised` が plugin A の
  まま残り、`rcp_selection` の declared-port 報告が事実と
  矛盾する persisted 不整合になる（save→load でも oracle が
  検査しない）。
- 補足: `vcs06_plugin_instance_matrix.rs` の fixture が
  port stamp を手動で「real rebind op が生成する形」と
  コメントしているが、実 op は port stamp を更新しない
  — fixture と実 op 出力が乖離している。

`plugin_instances` store 自体の不変条件は健全（本監査の
結論どおり）だが、persisted plugin identity の stale 化は
`SemanticPort.binding` 側にも存在する。別 Issue での
検討事項。

### ADJ-2: `port_capability_generation` が同一 `capability_revision` の rebind を検出しない（P3）

`routing_tx.rs` の `port_capability_generation` は
`ProcessorBinding::Resolved` と `PortBinding::PluginDeclared`
の `capability_revision` 値のみを合算する fingerprint。
`Resolved{A,rev:3}`→`Resolved{B,rev:3}`（descriptor/version
のみ異なり revision 同一、例: capability rev を維持したままの
plugin 更新）では generation が変化せず、doc comment の
「Any reconfigure or rebind changes it」が成立しない —
旧 plugin 前提で author された `RoutingEditIntent` が
`StalePortCapability` を経ずに受理され得る。ADJ-1 と複合
（port stamp が plugin A のままなら generation の入力自体も
stale）。command-level `expected_revision` CAS が主経路を
担保するため影響は限定的。別 Issue での検討事項。

## 判定

| 論点 | 判定 |
| --- | --- |
| store cascade への `unsupported`/op_facts 追加 | **不要** — 外部参照 dangling の実害が構造的に存在せず、過剰拒否は既存受理の回帰 |
| `ResourceKind`/`ENUMERATED_KINDS` への `PluginInstance` 追加 | **非対象** — wire/snapshot/VCS 全体に波及する拡張で実害なし |
| posture コメント明記 | **採用** — `prepare.rs` op_facts arm・`prepare_engine.rs` op-table entry・`ops.rs` op 定義 doc comment の 3 箇所 |
| test pin | **採用** — op 経路 3 件（rebind 退役・同一 plugin 冪等・Unresolved 退役）+ wire pin 1 件（Write detail + candidate store 同期） |
| `PluginStateProbe` staleness | **`synvelis#128`（TIM-04）へ defer** — evidence が plugin/instance identity を持たない staleness 契約の問題。in-tree 実装は fixture のみで実害実測なし。port 契約の世代区別は TIM-04 の設計に属する |

## 結論

退役経路は main 上で不変条件付き実装済みであり、persisted
identity の不変条件 3 軸は全て健全。実害が確認されないため
`unsupported` は立てず、契約明記（コメント）と test pin で
着地した。production コードの動作変更はなし。
