# ミキサー・Strip ドメインとバリアント — 実行証拠

Creative Branch（CB-01〜CB-11、synvelis#278〜#288）、TakeGroup/Comp（MD-A、synvelis#42 系）、ControlOwner/SendLevel（CTL-01/02、synvelis#121/#122）、strip ドメイン lowering・routing（synvelis#548/#550）、variant edit/apply・adopt facts（synvelis#511/#513/#539/#553/#557/#559/#561/#565/#567）、VCS semantic diff/apply・restore（synvelis#457/#458）の実装・検証記録を統合した証拠文書である。検証は主に mise run lane と各 crate の cargo test fixture で構成され、headless PASS は実機・plugin・device・consumer acceptance を意味しない。実機・外部 gate は NotRun/Deferred として分離記録されている。

## Creative Branch（synvelis#278〜#288）

CB-01 実装記録では、branch-local ID の live 予約・content-addressed store・`plan_adopt_writes` 再利用・schema migration の実装を記録した。`BranchContentRef::Captured`/`Live` の区別と digest pin を確認した。

CB-02〜CB-07 実装記録では、AudioClip/MIDI Clip/InstrumentSlot/ProcessingRange/Track-Bus Strip/Arrangement の各 adapter 実装を記録した。capability report・edit_candidate・boundary inventory・confinement 検査の定義を確認した。CB-06/CB-07 は in-progress ステータスの記録もあり、後続 issue slice で追補されている。

CB-08 実装記録では comparison orchestrator の Semantic/Signal/Perceptual 3 層分類と remainder digest の実装を記録した。CB-09 実装記録では Difference Explorer の `VariantDiffView`/`ViewSpec`/authority-validated action の実装を記録した。CB-10 実装記録では gateway typed contract の `PreparedMutation::Variant`/`VariantPorts`/scratch simulation/approval receipt 消費の実装を記録した。CB-11 実装記録では cross-gate の cross-domain 検証を記録し、`ArrangementAdapter` 未登録による fail-closed gap を pin した。synvelis#539 で解消された。

## MD-A TakeGroup/Comp（synvelis#42 系）

MD-A 実装記録では、TakeGroup/Comp schema・`CompOp`・effective comp 投影・normalized placement/playback・`SCHEMA_VERSION = 2` の実装を記録した。MD-A-01..06 の受入 test（takegroup→comp→projection、非重複/隣接/overlap/handle 拒否、tempo 跨ぎ再導出、境界一意帰属、undo/redo、Raw version 0 拒否）を確認した。

MD-A task1〜task4 は normalized placement schema・Comp schema/ops・effective comp projection/editor targets・acceptance tests/verification の task 分解記録であり、全て status done である。

## CTL-01/02（synvelis#121/#122）

CTL-01 実装記録では、`ControlOwner`/`AutomationTimeScope`/`ParameterRole` 型・`validate_references` oracle・schema v7 migration・`resolve_scope_pos`・`PreparedTarget` freeze の実装を記録した。legacy wire 受理と `deny_unknown_fields` の競合を手書き Deserialize で解決した。task は status done で、CTL-01 統合テスト全件 PASS・既存の PC 系テスト群・routing・controller store テストが新 API で PASS・v6→v7 read-only migration が座標厳密保持・deterministic であることを acceptance とした。

CTL-02 実装記録では、`send_level_spec()`/`has_canonical_send_level_spec()` 単一 source・`AddSend`/`SetSendGainTarget` validate の canonical spec 検査・非 canonical adopt reject の実装を記録した。task は status done で、非 canonical spec を持つ target の `AddSend`/`SetSendGainTarget`/`RoutingEdit` adopt が `Error::Validation` で reject され persisted されないこと、canonical spec を持つ target の adopt が引き続き受理され `Send`+`SendLevel` に seal されることを Done When とした。canonical 9 lane（fmt-check/check/lint/test/test-rt/check-app-native/lint-app-native/test-msrv/build）全 exit 0 を記録した。

## strip-domain lowering・routing（synvelis#548/#550）

synvelis#548 実装記録では、`op_table` への 5 capability descriptor 登録・lower fn 5 件・`lower_for`/`op_name`/`op_facts` arms・positive mask-gated edit 試験 2 件・ordinary prepare 試験 3 件の実装を記録した。task は status done で、gateway の op 表・lowering/facts 層・strip gateway 統合テスト・ordinary prepare 統合テスト・doc chain・sv548・sv283 の evidence 記録を含む。

synvelis#550 実装記録では、`StripField` 4 variant 追加・routing entity の `scope_objects` 導出・adapter 受理/confinement/removal check・`removal_supported_kind`/`remove_entity` arms・send 4 op の wire descriptor/lowering/facts・send fixture/wire test の実装を記録した。parent-contract を Strip から Project へ修正した記録もある。task は status done で、variant の mask 型・scope 導出・adapter・capability・apply entity 削除・strip fields 統合テスト・gateway op 表/lowering/facts・strip gateway 統合テストと共有 rig・doc chain・sv550・sv283 の evidence 記録を含む。

## variant edit/apply・adopt facts（synvelis#511/#513/#539/#553/#557/#559/#561/#565/#567）

synvelis#511 実装記録では、`plan_adopt_writes` の removal 判定を 3 分岐化（move-out `Unsupported`/非対応 kind `Unsupported`/`Removed` write 計画）・`removal_supported_kind`/`remove_entity` 追加・`reproject_ppqn` の `Captured` payload 拒否・`cb_arr_11`/`cb_arr_08`/`cb_arr_14`/`cb_arr_16` test 更新の実装を記録した。task は status done で、T1〜T7 全て完了。`Removed` write が preflight/introduced_digests で誤検知されるリスクは `Null` payload + Added/kind 限定で構造的に安全と評価した。

synvelis#513 実装記録では、`VariantOp::AddBranch`・`AddBranchReceipt`・apply surface 拡張（comping + variant kinds の `supported_kind`/`entity_value`/write ordering）・`SetProcessorBinding`・adapter confinement/capability 更新・vcs06/vcs07/rcd_04/cb03/cb04 test 更新の実装を記録した。task は status completed で T1〜T11 全て完了である。

synvelis#539 実装記録では、`ArrangementAdapter` import・adapters map 登録・陳腐化 doc comment 4 箇所更新・cross-gate 統合テストの pin fixture positive flip・gap 記録解消（sv288 の coverage 記録・cb-11 doc 3 件）の実装を記録した。task は status done で T1〜T7 全て完了である。

synvelis#553 実装記録では、`declared_facts` の inner `OpFacts` verbatim 伝播・`preconditions`/`unsupported` accumulator 追加・variant-edit `send.remove` unobservable cascade deny test の実装を記録した。red 確認（修正前に `Completed` 受理を実測）と green 確認（`cb06_strip_gateway` 全 8 件 Pass）を記録した。review-protocol 1 回・blocker 0・verdict ACCEPT である。task は status done で T1〜T5 全て完了である。

synvelis#557 実装記録では、`VariantIntent::Adopt` の `declared_facts` 監査・gap 無しの構造的根拠（`AdoptIntent` inner op 不在・`lower_adopt` CAS のみ）・`cb06_strip_557_adopt_send_remove_candidate_materializes_teardown` pin test・synvelis#559 起票の実装を記録した。`cb06_strip_gateway` 9 件全 Pass。production code 非変更である。task は status done で T1〜T6 全て完了である。

synvelis#559 実装記録では、`unverifiable_removal_kind` predicate（`ModulationSource`/`ModulationAssignment` のみ真）・`VariantError::UnverifiableRemoval`・`plan_adopt_writes` 検査・`variant_reason` `unverifiable-removal` arm・domain/gateway pin test の実装を記録した。`cb06_strip_fields` 25 件・`cb06_strip_gateway` 10 件全 Pass を記録した。Issue は open 維持（`Refs #559` のみ）である。task は status done で T1〜T7 全て完了。gateway pin の payload 生成方式が synvelis#561 着地後に domain seed 方式へ作り替えられた経緯を記録した。

synvelis#561 実装記録では、`control_closure_unverifiable` helper・`SetSendGainTarget`/`RemoveTrack`/`RemoveMidiSource` unverifiable 検査・`Composite` arm 逐次 scratch 評価・wire pin 10 件・domain pin 7 件の実装を記録した。`monitoring.tracks` decl 検査追加と batch seal→release under-report 修正を review 対応として記録した。task は status done でタスク 1〜9 全て完了である。

synvelis#565 実装記録では、`SetProcessorBinding` 退役 cascade 監査・不変条件 3 軸（再読み込み・復元・外部参照）照合・契約明記コメント 3 箇所・test pin 4 件の実装を記録した。production 動作変更ゼロである。`PluginStateProbe` staleness は synvelis#128 へ defer した。隣接発見 ADJ-1/ADJ-2（`SemanticPort.binding` stamp 未更新・`port_capability_generation` 同一 revision rebind 未検出）を別 Issue 要として記録した。task は status done でタスク 1〜8 全て完了である。

synvelis#567 実装記録では、`async_pol06_concurrency_quota_pends_then_resumes` の Windows CI flake 修正を記録した。`JobScheduler::enqueue_intent` nonblocking API 追加・test 再構成・assertion 意味不変である。flake loop 42 iteration 全 Pass を記録した。Issue は open 維持である。なおこの slice は render job scheduler の flake 修正であり variant/strip ドメインではないが、リストに含まれるため証拠として記録する。

## VCS semantic diff/apply・restore（synvelis#457/#458）

synvelis#457 実装記録では、`ComparisonDomain::Variants`・`ObjectKind::VariantSet`/`VariantBranch`・`VariantsAdapter`・`classify_variants`・`DIFF_POLICY_VERSION` 3→4・variant diff 統合テスト 7 件・ledger `VCS-DIFF-11` Blocked→Pass flip の実装を記録した。variant matrix 統合テスト 10 件・diff 統合テスト 36 件 Pass を記録した。task は status completed で、apply surface transplant・`restore_as_copy` nested origin defect（#458）は out of scope とした。

synvelis#458 実装記録では、`ProvenanceRule` enum・`validate_set_shape` 規則引数・`stable_id_counts` 拡張・`vcs06_variant_matrix` restore pin flip（Fail→Published + envelope verbatim readback）・ledger `VCS-RESTORE-10` flip の実装を記録した。`vcs06_variant_matrix` 3 tests・`mise run test synvelis-project` 全 Pass を記録した。task は status done で、`restore_as_copy` on variant-set project が `Published` で成功し、envelope readback で `variant_sets` verbatim + `project_id` のみ再採番 + provenance source 指し継続が assert されることを Done When とした。

## 検証 lane と review の実績

各 slice の検証は `mise run` タスクを canonical とし、`fmt-check`/`check`/`lint`/`test`/`test-rt`/`build`/`test-msrv`/`check-headless-deps`/`check-ruviform-rev`/`lint-actions`/`test-ui`/`prek run --all-files` を使う。cargo 直接実行は補助証拠に留める運用であった。CTL-02 は canonical 9 lane（fmt-check/check/lint/test/test-rt/check-app-native/lint-app-native/test-msrv/build）全 exit 0 を記録した。synvelis#553 は review-protocol 1 回・blocker 0・verdict ACCEPT を記録した。red→green の手順として、synvelis#553 では修正前に新規 test を実行して `Completed` 受理を実測し、修正後に `cb06_strip_gateway` 全 8 件 Pass を確認した。fail-closed deny test は「unverifiable」が `provisional_gate` の unsupported check のみが生成することを assert で pin する方針を取った。

Ruviform 依存は既定ブランチ追従の git 依存で、解決 revision は `Cargo.lock` に記録する。CTL-01 検証では ruviform revision `8fb665f1` が記録されている。`check-ruviform-rev` の REV 不一致は stale vendor を検出する fail-closed であり、復旧は `sync-ui-deps` の再実行である。workspace の時間演算は `ruviform-media-core` の unchecked `Add`/`Sub` を避け、checked 演算を使用する lint 対象である。

## JSONL 実験ログ

各 issue slice の exp JSONL ログは実験イベントログとして存在し、採否判断（adopted/rejected）の根拠を記録している。JSONL はスキム対象であり、詳細な実験設計・却下理由は対応する exp 文書を参照すること。特に synvelis#559 のログには gateway prepare/effect 層での kind filter 不採用（EXP-2）・`ENUMERATED_KINDS` 外の全 removal-supported kind reject 不採用（EXP-3）の記録がある。synvelis#567 のログには helper thread 残留+scheduler defect 仮説・`GateExecutor` 内 enqueue・他テスト同時書き換えの却下記録（EXP-1/2/4/5）がある。

## 矛盾・曖昧性・未解決事項

Arrangement candidate 削除の契約変更：CB-07 当初は「editable だが adopt/render は defer」だったが、synvelis#511 で Clip/Note の `Removed` materialization が実装され、`cb_arr_11`/`cb_arr_08`/`cb_arr_14` の pin が新契約へ書き換えられた。後続文書は新契約を前提とするが、CB-07 の記述と矛盾する点は明示的に残る。

CB-06/CB-07 の impl 記録は in-progress ステータスを含むが、後続の synvelis#548/#550/#553 等で追補・完了記述が追加されている。ステータスの齟齬は文書間で残る可能性がある。

synvelis#559 の gateway pin は当初 wire 合法 op（`send.set-gain-target` 解除）で payload を生成する設計だったが、synvelis#561 着地により当該 wire 経路が `op_facts` unverifiable cascade として prepare で拒否されるようになったため、domain 層 seed 方式へ作り替えられた。「wire で生成不能な durable payload への防御層」という pin の本来意図を維持するための変更である。

synvelis#557 は「gap 無し」の監査結論を出したが、synvelis#559 で残存経路（pre-553 stored payload）が hardening 対象として起票された。#557 の「gap 無し」は現行 wire での新規生成不能性に基づくものであり、durable store の既存 payload には適用されない点に注意が必要である。

synvelis#561 の `SetProcessorBinding` 退役 cascade は synvelis#565 で監査され「不変条件健全・外部参照 dangling なし」と結論したが、隣接発見 ADJ-1/ADJ-2（`SemanticPort.binding` の `PortBinding` stamp 未更新・`port_capability_generation` が同一 revision rebind を検出しない）は別 Issue 要として記録された。これらは main 既存の P2/P3 であり、本 slice 範囲外である。

synvelis#567 の `async_pol06_cancel_queued_job_is_typed` にも同型の理論的 race が残存するが、観測 flake はなく残件記録に留められた。実 plugin/PCM/RT/GUI/OS/device/consumer acceptance はほぼ全 slice で NotRun/Deferred であり、headless 検証の PASS を実機受入と混同しないこと。docs 配下の管理 index は複数 slice で未同期のまま Coordinator 統合時再生成と記録されており、文書間で index 状態の記述が異なる可能性がある。synvelis#559・#567 は Issue が open 維持であり、完了判定は Issue 側のクローズに委ねられている。実装記録の status completed は slice の作業完了を示すが、Issue のクローズを意味しない。

## 出処

synvelis#42・#121・#122・#128・#278〜#288・#457・#458・#511・#513・#539・#548・#550・#552・#553・#557・#559・#561・#565・#567。Creative Branch（CB-01〜CB-11）、TakeGroup/Comp（MD-A）、ControlOwner/SendLevel（CTL-01/02）、strip ドメイン lowering・routing、variant edit/apply・adopt facts、VCS semantic diff/apply・restore、および検証・運用プロセスに関する作業群からの抽出。
