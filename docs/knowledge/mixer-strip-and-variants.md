---
id: "KNOW-fzxZz9VZkO3cvNfeyzZWwP"
type: "knowledge"
status: "draft"
lifecycle: "draft"
knowledge_kind: "architecture"
title: "ミキサー・Strip ドメインとバリアント"
created: "2026-11-19"
updated: "2026-11-19"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/42"
    - "https://github.com/scarecrow173/synvelis/issues/121"
    - "https://github.com/scarecrow173/synvelis/issues/122"
    - "https://github.com/scarecrow173/synvelis/issues/128"
    - "https://github.com/scarecrow173/synvelis/issues/278"
    - "https://github.com/scarecrow173/synvelis/issues/279"
    - "https://github.com/scarecrow173/synvelis/issues/280"
    - "https://github.com/scarecrow173/synvelis/issues/281"
    - "https://github.com/scarecrow173/synvelis/issues/282"
    - "https://github.com/scarecrow173/synvelis/issues/283"
    - "https://github.com/scarecrow173/synvelis/issues/284"
    - "https://github.com/scarecrow173/synvelis/issues/285"
    - "https://github.com/scarecrow173/synvelis/issues/286"
    - "https://github.com/scarecrow173/synvelis/issues/287"
    - "https://github.com/scarecrow173/synvelis/issues/288"
    - "https://github.com/scarecrow173/synvelis/issues/457"
    - "https://github.com/scarecrow173/synvelis/issues/458"
    - "https://github.com/scarecrow173/synvelis/issues/511"
    - "https://github.com/scarecrow173/synvelis/issues/513"
    - "https://github.com/scarecrow173/synvelis/issues/539"
    - "https://github.com/scarecrow173/synvelis/issues/548"
    - "https://github.com/scarecrow173/synvelis/issues/550"
    - "https://github.com/scarecrow173/synvelis/issues/552"
    - "https://github.com/scarecrow173/synvelis/issues/553"
    - "https://github.com/scarecrow173/synvelis/issues/557"
    - "https://github.com/scarecrow173/synvelis/issues/559"
    - "https://github.com/scarecrow173/synvelis/issues/561"
    - "https://github.com/scarecrow173/synvelis/issues/565"
    - "https://github.com/scarecrow173/synvelis/issues/567"
  informs: []
  explains: []
  supersedes: []
---

# ミキサー・Strip ドメインとバリアント

## 概要

Synvelis のミキサー Strip とバリアント編集は、Creative Branch（CB-01〜CB-11、synvelis#278〜#288）を基盤に、TakeGroup/Comp の effective comp 投影（MD-A）、ControlOwner・AutomationTimeScope・SendLevel の制御モデル（CTL-01/02）、strip ドメインの wire lowering と routing entity（synvelis#548/#550）、および variant の edit/apply/adopt 経路の fail-closed facts（synvelis#513/#553/#557/#559/#561）から構成される。VCS の semantic diff/apply/restore（synvelis#457/#458）まで含めて、variant state の保存・比較・復元・採用まで一貫した契約が敷かれている。設計の軸は、観測不能な効果を typed error で拒否する fail-closed、capability/facts 宣言と実 apply の一致、永続化された candidate payload の provenance 保持である。

## 詳細

### Creative Branch の基盤設計

CB-01 では、branch ID を mint 時点で live の ID 空間に予約し、adopt 時に remap しない設計とした。これにより lineage 衝突を構造的に防ぎ、`BranchLineage` と `ReservingIdGenerator` が一貫して機能する。controller の raw-ID 予約経路もこの設計に従う。

candidate payload は project snapshot に埋め込まず、外部の content-addressed な `BranchContentStore` に置く。payload は digest/schema/scope fingerprint で pin され、`BranchContentRef::Captured`（durable・bytes を store に保持）と `BranchContentRef::Live`（参照のみ）を区別する。adopt の書き込みは既存の apply machinery（`plan_adopt_writes`、`PlannedWrite`、`entity::write_all`、preflight、CAS、Undo、save/reload）を再利用し、新規の書き込み経路を作らない。これにより Undo・CAS・永続化の一貫性を保つ。schema migration は明示的な ladder として管理し、branch 追加に伴う snapshot 形式の変更を段階的に扱い、旧 schema からの読み込みは fail-closed に検証される。

fail-closed の error は typed に網羅的に定義する。`Unsupported`（操作非対応）、`ScopeViolation`（scope 外）、`StaleScope`（scope digest 不一致）、`RecordingActive`、`MissingPlugin`、`NotFound`、`Project(Validation)`、`Unapproved`、`UnknownCapability` を区別し、「操作非対応」と「効果を観測・認可できない」を判別可能にする。この方針は後続 slice の `VariantError::UnverifiableRemoval`（synvelis#559）にも引き継がれた。

CB-02〜CB-07 では、AudioClip・MIDI Clip・InstrumentSlot・ProcessingRange・Track/Bus Strip・Arrangement の各ドメインに adapter を実装し、capability report・edit_candidate・boundary inventory を adapter ごとに定義した。Strip ports は boundary inventory には所属するが scope-owned ではない点に注意が必要である。adopt の adapter-aware 経路は新規の `adopt_branch_adapted` として追加し、旧 API を残した。ドメインごとの capability/edit 判定を adapter に集約し、engine 側は generic な plan/write に留める設計である。

CB-03 では、MIDI の `SmfDocument` は scope 所有の外側にある shared provenance であり、SMF-backed note edit は adapter が op 自身の validate を再実行して拒否する。confinement 検査は adapter ごとに複製せず、中央の scope 検査を拡張する方針とした。CB-05 の FX ProcessingRange は初期実装で chain-tail の `end` のみ受理し、構造的に `InsertProcessor` のみを対象とし、外部 routing op を拒否する。playback mapping が unsupported な場合は render 時まで遅延させず、adapter の capability/edit 境界で拒否する。

Arrangement candidate の削除は契約変更を経た。CB-07 当初は「editable だが adopt/render は defer」とされていたが、synvelis#511 で Clip/Note の `Removed` materialization が実装され、挙動が変わった。源文書間の明確な契約変更であり、後続文書は新契約を前提とする。

synvelis#511 では、PPQN 変更を、いずれかの variant branch が `Captured` payload を保持する場合は常に typed 拒否とした。candidate payload を安全に再投影できないための fail-closed 境界であり、`SetProjectPpqn` の validate は `reproject_ppqn` の dry-run 経由でこの契約を継承する。

CB-08 の comparison orchestrator は Semantic・Signal・Perceptual の 3 層で差分を分類し、shared-context のドリフトを別次元で扱う。真に unnamed な shared drift のみ remainder digest で扱い、既知の次元は個別に報告する。provider evidence は orchestrator が gate し、mock evidence は label 付けし、unavailable/unknown を zero difference として表示しない。CB-09 の Difference Explorer の `VariantDiffView` は driver-neutral な `ViewSpec` を使い、action は authority 側で再検証する。CB-10 の gateway の variant 操作は `PreparedMutation::Variant` として独立した prepared mutation 型とし、`VariantPorts`・scratch simulation・`ScratchStore`・消費済み approval receipt を使う。approval ticket は caller 供給の wire 引数ではなく、消費された gateway receipt から注入する。CB-11 の cross-gate は、`ArrangementAdapter` が `VariantPorts::standard` に未登録だったため fail-closed unsupported を pin していたが、synvelis#539 で登録され、pin fixture は positive coverage に flip した。

### TakeGroup/Comp の effective comp 投影（MD-A）

TakeGroup/Comp は normalized placement（`ClipPlacement`/`ClipPlayback`/`PlaybackMapping`/`StretchRatio`）と effective comp 投影（`project_comp` 純粋導出）で表現する。旧 `TimeAnchor`（区間 enum）は `ProjectRange` に rename され、新 `TimeAnchor` は点 anchor となった。

時間契約は半開区間 `[start,end)` とし、PPQN ベースの musical anchor と media frame の absolute anchor を区別し、tempo mapping を介して再導出する。native source frame は保存し、境界の一意帰属と overlap 限定重複を検証する。handle 導出は take coverage・existing-source window・silence の 3 種である。`EditorDisplayTarget`/`EditorEditingTarget`/`SourceVersionRef` は型契約のみで、Raw version 0 束縛は拒否する。effective comp は同一 Project からの投影が完全一致し、Source は不変であることを要件とし、位置問合せは高々 1 件を返す。

Comp 系は `CompRegistry` + `CompOp`（project-domain operation）として schema v2 に導入され、`validate_references` が comp 参照整合（owner・adjacency・overlap・handle・非重複）を検査する。undo/redo で ID/状態が復元されることを MD-A-05 で pin した。`SCHEMA_VERSION = 2` への bump は PoC 段階の明示的拒否であり、旧 snapshot 互換の migration は対象外とした。consumer（recording の clip 生成・player の lowering 解決）は新スキーマに機械的適合させ、`SkippedClip::UnsupportedMapping` で unsupported mapping を typed に扱う。

### ControlOwner・AutomationTimeScope・SendLevel（CTL-01/02）

CTL-01 では `ControlScope` を廃止し、`ControlOwner`（Strip/Send/Processor 等）と `AutomationTimeScope`、`ParameterRole` に分離した。owner×time_scope の matrix、`(owner, role)` singleton 一意性、`gain_target`→`ControlOwner::Send` の owner mapping を `validate_references` で oracle 検査する。

schema v7 への migration は `migrate_v6_control` で実施し、legacy wire は手書き Deserialize で tag 二形態を厳密受理する。`deny_unknown_fields` との競合をこの手書き実装で解決した。`ProjectAbsolute` time scope は beats として評価せず、明示的に `ScopeUnavailable` を返す。`PreparedTarget` は owner+time_scope を freeze し、`is_stale` の recompile 比較に両 field を含める。routing entity 削除時の cascade を監査し、Strip/Send/Processor 所有 target の dangling を除去する。

CTL-02 では `SendLevel` の canonical internal spec を制御モデル層に単一 source（`send_level_spec()`/`has_canonical_send_level_spec()`）として定義し、`AddParameterTarget` 検査と `AddSend`/`SetSendGainTarget` の adoption validate で共有する。canonical spec は `Continuous`/`Linear`/`ProjectMusical`/`capability: None` である。非 canonical spec を持つ `SendLevel` target の adoption は `Error::Validation` で reject し、`ProjectAbsolute` target の adoption も新たに reject される意図的な契約狭めである。plugin binding 転用を防ぐためで、`SendGainSpec::Create` 受理や他 crate・manifest は変更しない。

synvelis#552 では、`SetSendGainTarget` の rebind（Some→Some）や unconfigure（Some→None）が、旧 `SendLevel` seal の release（旧 target + lane + assignment 閉包の削除）と新 seal を 1 apply で不可分に適用する。release helper は旧 target 単位で、send-owned source や他 target は残す。`Some→None→Some` の経路でも孤立 seal が残らないことを pin した。rebind を `Unsupported` で拒否していた typed fail-closed を、seal release 実装とともに positive 受理へ転換し、adapter の `check_strip_routing_op_targets` から typed `Unsupported` 分岐を除去した。scope 検査は維持する。

### Strip ドメインの lowering・routing entity（synvelis#548/#550）

synvelis#548 では strip-domain の wire capability を 5 件追加し、mask/confinement の所有は project adapter に残した。`op_table` 登録と `lower_for`/`op_name`/`op_facts` arms は同一 commit で追加しないと `op_specs()` が panic する構造的制約がある。facts 未宣言で ordinary prepare が fail-closed するため、`op_facts` arms を必ず追加する。positive mask-gated edit 試験では、狭い mask（`inserts` 未宣言）で insert/rebind を `variant-unsupported` に pin し、mask 負側証拠を維持した。`ProcessorBinding` wire decode は `arg_typed` の serde 形を cb06 fixture と同一 JSON で試験する。

synvelis#550 では routing mask（`Sends`/`MainRoute`/`Sidechain`/`Bus`）と strip-owned routing entity（SendConfig、Send edge、SendLevel target、modulation source/assignment、main/sidechain connection）を追加した。routing entity の `scope_objects` 導出は source-side 所有規則に従う。send wire lowering は send-only に限定し、connection/bus capability は対応する `ResourceKind` が存在しないため登録しない。`UnknownCapability` を維持することで deferred を偽装しない。

gain-target の scope 検査は、foreign `SendLevel` ownership の takeover を防ぐために追加した。別 target への rebind は、apply 経路が旧 `SendLevel` seal を解放しないため `Unsupported` で拒否されていたが、synvelis#552 で解消された。`RemoveSend` の unobservable modulation cascade は top-level prepare で deny される。`ResourceKind`/`snapshot::ENUMERATED_KINDS` は `ModulationSource`/`ModulationAssignment`/`RoutingConnection`/`SemanticPort` を含まない SEC-14 境界であり、これらの削除は観測モデル外である。

entity removal の materialization は `removal_supported_kind` と `remove_entity` で管理し、`Removed` write の payload は削除 entity の serialization とする。preflight は `Removed` を自然スキップし、`Null` payload で安全に扱う。`StripField` 追加は serde 後方互換であり、旧 mask JSON は有効のまま。parent-contract を Strip から Project へ修正した記録があり、send creation の scope 帰属が是正された。

### variant の edit/apply・adopt facts（synvelis#513/#553/#557/#559/#561）

synvelis#513 では `VariantOp::AddBranch`（literal 3 candidate lane）、`AddBranchReceipt`、comping/variant apply surface、`ProjectOp::SetProcessorBinding` を追加した。apply surface は comping-kind を variant adopt 経路で別 gate 受理しつつ、VCS `apply_selected` からは除外する。apply surface の拡張は `supported_kind`/`entity_value`/write ordering の変更として実装され、`Removed` は不変のまま `Replaced` を許可する形で comping replay を可能にした。

synvelis#553 では `declared_facts` が `VariantIntent::Edit` の inner `ProjectOp` に対して `op_facts` を呼びながら `preconditions`/`unsupported` を破棄していた gap を修正した。inner `OpFacts` を外側へ verbatim 伝播し、`provisional_gate` の "unverifiable effects" fail-closed deny が variant-edit 経路にも効くようにした。`reads`/`impacted` の既存伝播は不変である。`BranchEditOp::Comp` は wire lowering が Comp inner を受理しないため到達不能であり、inner facts を持たない。将来 Comp 経路が wire に載る場合は別 gap になると記録されている。

synvelis#557 では `VariantIntent::Adopt` 経路の `declared_facts` 伝播を監査し、synvelis#553 同型の gap は構造上存在しないことを確認した。`AdoptIntent` は set/branch/CAS/approval フィールドのみで inner op を持たず、wire `lower_adopt` も CAS 4 項目のみ受理する。adopt の write 認可は post-simulation の `derive_effects`/`authorize_effects` が実 diff で行う。ただし `DirectoryBranchContentStore` は durable であり、synvelis#550〜#553 の窓に永続化された candidate payload は pre-553 wire `send.remove` cascade が生成した send-owned `ModulationSource`/`ModulationAssignment` 削除を含み得る。この残存経路を synvelis#559 として起票した。

synvelis#559 では、durable stored candidate payload に含まれる観測不能 kind（`ModulationSource`/`ModulationAssignment`）の `Removed` write を `plan_adopt_writes` が `VariantError::UnverifiableRemoval` で fail-closed 拒否するようにした。`RoutingConnection` は send teardown の internal structure として意図的に対象外とし、観測可能な send teardown の adopt 物質化は維持した。fail-closed の設置境界は gateway prepare/effect 層ではなく domain `plan_adopt_writes` とした。`Removed` write 化直前に正確な `ObjectKind` を知る唯一の層であり、SEC-14 fail-closed の既存所有者である。gateway 側 filter は owner 境界下流での policy 重複になるため不採用とした。reject 集合は `ModulationSource`/`ModulationAssignment` のみで、`removal_supported_kind` ∧ 観測モデル外はこの 2 つと `RoutingConnection` の 3 つだが、後者は観測可能な owner（Send/Processor）削除に不可分な internal structure であり、reject すると synvelis#550/#557 の受理済み send teardown 契約を破壊する。

synvelis#561 では `op_facts` の `SetSendGainTarget`/`RemoveTrack`/`RemoveMidiSource` arm に unverifiable 検査を追加し、`RemoveSend` arm の inline ロジックを `ControlOwner` 版 helper `control_closure_unverifiable` に semantics-preserving に抽出した。domain（`synvelis-project`）は非接触で、cascade の発火条件は既存仕様のまま mirror するのみである。`monitoring.tracks` decl は optional（0-or-1）で独立 content を持つ keyed companion であり、strip chain/ports のような必須内部構造ではないため、`contains_key` 検査で unverifiable とした。`smf_documents` 削除も同様に unverifiable である。

`Composite` arm は全 member を pinned base に対して評価するため、batch 内で seal→release される中間状態を見逃す under-report があった。`lower_bound_batch` と同型の逐次 scratch 評価（validate → clone → apply → 成功時のみ swap）に修正し、member の reads/impacted/preconditions も中間状態を反映するようにした。`SetSendGainTarget` の unsupported 条件は apply の発火条件を厳密にミラーし、条件を満たさない rebind/clear では立てない（過剰拒否 = 既存受理の回帰を防ぐ）。positive control test で反証した。`SetProcessorBinding` の `sync_processor` が `plugin_instances` の旧 `PluginInstanceId` を退役させる cascade は本 slice 範囲外（main 既存・別クラスの persisted identity）として記録された。

synvelis#565 では `SetProcessorBinding` → `plugin_instances.sync_processor` の `PluginInstanceId` 退役経路を監査し、不変条件 3 軸（再読み込み・復元・外部参照）の照合結果「不変条件健全・外部参照 dangling なし」を確定した。production 動作変更はゼロで、契約明記コメント 3 箇所と test pin のみである。`PluginStateProbe` staleness は `PluginStateEvidence` が plugin/instance identity を持たず、rebind 後に旧 plugin 用の evidence が同一 `ProcessorId` の新 plugin に attach し得る問題だが、in-tree 実装は fixture のみで実害実測なし。TIM-04（synvelis#128 plugin state snapshot 一貫性）の設計に属するため defer と判定した。隣接発見 ADJ-1/ADJ-2（review-protocol で CONFIRMED・別 Issue 要）として、`SetProcessorBinding` が persisted `SemanticPort.binding`（`PortBinding` stamp）を更新しないため rebind 後も旧 plugin の port binding が残り、`resolved_endpoints` の `UnresolvedPort` 拒否迂回・declared-port 報告の事実矛盾が起き得る（P2・main 既存）。また `port_capability_generation` が `capability_revision` のみを合算するため同一 revision の rebind を検出しない（P3・main 既存）。

synvelis#567 では `async_pol06_concurrency_quota_pends_then_resumes` の Windows CI flake は scheduler/orchestrator の欠陥ではなく test-design race が根因と特定した。helper thread の `try_send` 実行タイミングがテスト内のどの同期点からも制御されておらず、A の terminal 遷移後の drain に滑り込むと `pending_gate` が active=0 を見て B を即受理し pend receipt が記録されない。`JobScheduler` に nonblocking `enqueue_intent` を追加し、test は helper thread を廃して `enqueue ≺ release` の happens-before を program order で構造保証する形に再構成した。assertion の意味（B に `Pending{QuotaExceeded}` と `Resumed`、両 job `Published`、executor calls==2）は一切変更していない。この slice は variant/strip ドメインではないが、同じ証拠群に含まれる。

### VCS semantic diff/apply・restore（synvelis#457/#458）

synvelis#457 では `variant_sets`（schema v14）を VCS-02 semantic diff の比較対象に追加し、`ObjectKind::VariantSet`/`VariantBranch`、`ComparisonDomain::Variants`、`VariantsAdapter`（set `diff_map` + branch の `MidiNote` 型 nested-global diff）を整備した。`DIFF_POLICY_VERSION` は 3→4 に bump した。apply surface は `Unsupported` fail-closed のまま変更なしである。field class は `origin`/`scope`/`created_from`/`content` → `Replaced`、`parent` → `Identity`、bookkeeping → `Modified`、`id`/`branches` → `Skip` とした。`scope` を `Replaced` としたのは governed subject の identity-defining 参照として `origin` と同格だからである。`render_input` canonical value 解決は追加したが、`all_semantic_objects` inventory には非搭載とした。variant state は render input ではなく、`shared_context_digest` の過剰 invalidation を回避するためである。

synvelis#458 では `restore_as_copy` について、`fork_project` が `project_id` のみ再採番し nested `VariantSet::origin`/`Branch::created_from` の `SnapshotIdentity` を verbatim 保持するのが CB-01 の authored provenance 意味論どおり正しいと確定した。`SnapshotIdentity` は `{project_id, source, locator, digest, ...}` で digest/locator が source snapshot bytes を指す authored 参照であり、`project_id` だけを書き換えると存在しない snapshot への参照を捏造して provenance を毀損する。`validate_set_shape` を `ProvenanceRule`（`SameProject`/`Persisted`）で境界分割し、live op 境界（`VariantOp::Fork::validate`）は `SameProject` で strict を維持し、persisted/load oracle（`validate_persisted` → `validate_references`）は `Persisted` で source-project provenance を受理する。設計 (a) nested identity 再採番は棄却した。`stable_id_counts` に `variant-set`/`branch` を追加し、restore の verbatim 検証を可能にした。`BranchContentRef::Captured` の payload bytes は `BranchContentStore`（project bundle 外）にあり、restore は fork 先へ bytes を複製しないため、復元 project で candidate 操作を行うと `CandidateUnavailable` になりうる既知の capability 限界がある。

### 検証・運用・プロセスの知見

検証は `mise run` タスクを canonical とし、`mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/`build`/`test-msrv`/`check-headless-deps`/`check-ruviform-rev`/`lint-actions`/`test-ui`/`prek run --all-files` を使う。cargo 直接実行は補助証拠に留める。mise.toml がタスク名・引数の正本である。

headless PASS は実 plugin/PCM/RT/GUI/OS/device/consumer acceptance を意味しない。実機・外部 gate（#103/#109 Domain Oracle、AudioLens、Device Oracle、実 Plugin verified binding、3-OS 横断、Full CI）は NotRun/Deferred として正直に記録する。GUI-L2・EXT-L3・MSRV・Full CI は検証ポリシー §5 により日常必須検証に含めず Daily/手動 Full CI に委譲する。

review-protocol は 1 回以上実施し、blocker 0・verdict ACCEPT（または ACCEPT WITH NON-BLOCKING NOTES）を確認してから commit/PR する。review で発見された gap は別 Issue として起票し、lease 範囲外の修正は行わない。docs 配下の管理 index（各種 README）は Worker が編集せず、Coordinator が統合時に再生成する。doc chain（discovery/specs/designs/plans/tasks/test-specs/impl-ir/impl-exp + exp JSONL ログ）は slice ごとに新規作成し、generated index の未同期は PR 本文の「未対応の作業」に明記する。commit/PR 本文は `Refs #N` のみ使用し、`Closes`/`Fixes`/`Resolves` は禁止である。Issue は open のまま維持し、完了判定は Issue 側のクローズに委ねる。

fail-closed の境界では typed error を使い分ける。`Unsupported`（操作非対応）、`ScopeViolation`（scope 外）、`StaleScope`（scope digest 不一致）、`RecordingActive`、`MissingPlugin`、`NotFound`、`Project(Validation)`、`Unapproved`、`UnknownCapability`、`UnverifiableRemoval`（観測不能削除）、`PreparationRejected`（gateway prepare 拒否）などである。gateway の capability 登録は `op_table` descriptor と `lower_for`/`op_name`/`op_facts` arms を同一 commit で追加する。

test fixture は共有のテスト rig を原則非接触とし、test-local helper（`seed_*`/`boot_*`/file-local fixture）で構成し、並行 slice 間の干渉を避ける。red→green の確認は修正前に新規 test を実行して失敗を実測し、修正後に全件 Pass を確認する手順を踏む。特に fail-closed deny test は「正しい deny 経路」を pin していることを assert で確認する（例: "unverifiable" は `provisional_gate` の unsupported check のみが生成することを確認）。merge 後 head での再実測が必要な場合がある（synvelis#559 の gateway pin は synvelis#561 着地後に domain seed 方式へ作り替えて再実測した）。upstream merge による conflict 解消では upstream（main）の managed index を採用する方針が記録されている。

Ruviform 依存は既定ブランチ追従の git 依存とし、解決 revision は `Cargo.lock` に記録する。`check-ruviform-rev` の REV 不一致は stale vendor を検出する fail-closed であり、復旧は `sync-ui-deps` の再実行である。CTL-01 検証では ruviform revision `8fb665f1` が記録されている。workspace の時間演算は `ruviform-media-core` の unchecked `Add`/`Sub` を避け、checked 演算（`checked_add`/`checked_sub`）を使用する。これは lint 対象である。

### 矛盾・既知の制約

Arrangement candidate 削除の契約変更：CB-07 当初は「editable だが adopt/render は defer」だったが、synvelis#511 で Clip/Note の `Removed` materialization が実装され、`cb_arr_11`/`cb_arr_08`/`cb_arr_14` の pin が新契約へ書き換えられた。後続文書は新契約を前提とするが、CB-07 の記述と矛盾する点は明示的に残す。

CB-06/CB-07 の impl 記録は in-progress ステータスを含むが、後続の synvelis#548/#550/#553 等で追補・完了記述が追加されている。ステータスの齟齬は文書間で残る可能性がある。

synvelis#559 の gateway pin は当初 wire 合法 op（`send.set-gain-target` 解除）で payload を生成する設計だったが、synvelis#561 着地により当該 wire 経路が `op_facts` unverifiable cascade として prepare で拒否されるようになったため、domain 層 seed 方式へ作り替えられた。「wire で生成不能な durable payload への防御層」という pin の本来意図を維持するための変更である。

synvelis#557 は「gap 無し」の監査結論を出したが、synvelis#559 で残存経路（pre-553 stored payload）が hardening 対象として起票された。#557 の「gap 無し」は現行 wire での新規生成不能性に基づくものであり、durable store の既存 payload には適用されない点に注意が必要である。

synvelis#561 の `SetProcessorBinding` 退役 cascade は synvelis#565 で監査され「不変条件健全・外部参照 dangling なし」と結論したが、隣接発見 ADJ-1/ADJ-2（`SemanticPort.binding` の `PortBinding` stamp 未更新・`port_capability_generation` が同一 revision rebind を検出しない）は別 Issue 要として記録された。これらは main 既存の P2/P3 であり、本 slice 範囲外である。

synvelis#567 の `async_pol06_cancel_queued_job_is_typed` にも同型の理論的 race が残存するが、観測 flake はなく残件記録に留められた。実 plugin/PCM/RT/GUI/OS/device/consumer acceptance はほぼ全 slice で NotRun/Deferred であり、headless 検証の PASS を実機受入と混同しないこと。docs 配下の管理 index は複数 slice で未同期のまま Coordinator 統合時再生成と記録されており、文書間で index 状態の記述が異なる可能性がある。synvelis#559・#567 は Issue が open 維持であり、実装記録の status completed は slice の作業完了を示すが、Issue のクローズを意味しない。

## 適用対象

この知識は Synvelis のミキサー Strip・variant・comping・制御モデル・VCS diff/restore に関わる全サブシステムに影響する。synvelis-project は永続モデル・schema migration・`plan_adopt_writes`・`removal_supported_kind`/`remove_entity`・`validate_references` oracle・`CompRegistry`/`CompOp` を所有し、gateway は `op_table`/`lower_for`/`op_name`/`op_facts`・`PreparedMutation::Variant`・`VariantPorts` を担う。Comparison/Explorer（CB-08/09）と VCS（#457/#458）は variant state の diff・apply・restore を扱う。UI 層は `VariantDiffView` の `ViewSpec` と authority 側の action 再検証に従う。

検証・受入工程では、mise run lane・evidence 記録・外部 gate の NotRun/Deferred 管理・review-protocol・`Refs #N` のみの PR 運用という規約が同じ知識体系に属する。

## 出処

Creative Branch（CB-01〜CB-11）、TakeGroup/Comp（MD-A）、ControlOwner/SendLevel（CTL-01/02）、strip ドメイン lowering・routing、variant edit/apply・adopt facts、VCS semantic diff/apply・restore、および検証・運用プロセスに関する作業群からの抽出。出所 Issue は synvelis#42・#121・#122・#128・#278〜#288・#457・#458・#511・#513・#539・#548・#550・#552・#553・#557・#559・#561・#565・#567 である。
