# Architecture Decision Records

<!-- doc-suite:generated-index -->

Directory: `docs/designs`

| ID | Title | Status | File |
| --- | --- | --- | --- |
| DESIGN-OVERVIEW | Synvelis 設計概要 | draft | [overview.md](./overview.md) |
| DESIGN-pzR2GiO8Eru6Jw2zRK6l2s | ALIB-01 Design — Asset Provider 契約・Local/Project 基準 Provider | approved | [alib-01-asset-provider-contract-design.md](./alib-01-asset-provider-contract-design.md) |
| DESIGN-034VMQIe8i9hOn2Tb317Ue | ALIB-02 residual closeout — 証拠照合と gate 分類の設計 | approved | [alib-02-residual-closeout-design.md](./alib-02-residual-closeout-design.md) |
| DSN-alib02-streaming-import | ALIB-02 — Streaming Verified Import 設計（synvelis-library） | approved | [alib-02-streaming-verified-import-design.md](./alib-02-streaming-verified-import-design.md) |
| DSN-alib02-v5snap | ALIB-02 — v5 snapshot asset-digest 互換移行 設計 | approved | [alib-02-v5-snapshot-compat-design.md](./alib-02-v5-snapshot-compat-design.md) |
| DSN-alib03-catalog-preview-cache | ALIB-03 — Catalog/Preview Cache 設計（synvelis-library） | approved | [alib-03-catalog-preview-cache-design.md](./alib-03-catalog-preview-cache-design.md) |
| DSN-alib04-git-remote-provider | ALIB-04 — Git Remote Provider 設計（synvelis-library） | approved | [alib-04-git-remote-provider-design.md](./alib-04-git-remote-provider-design.md) |
| DESIGN-alib04-lfs-remote-fetch | ALIB-04 LFS Remote Fetch — 設計（endpoint 解決・batch API・型付きエラー・credential 境界） | approved | [alib-04-lfs-remote-fetch-design.md](./alib-04-lfs-remote-fetch-design.md) |
| DSN-alib06-webdav-provider | ALIB-06 — WebDAV File-Server Provider 設計（synvelis-library） | approved | [alib-06-webdav-provider-design.md](./alib-06-webdav-provider-design.md) |
| DSN-alib07-fault-matrix | ALIB-07 — Asset Library 全経路 Fault Matrix 設計（synvelis-library headless slice） | approved | [alib-07-fault-matrix-design.md](./alib-07-fault-matrix-design.md) |
| DSN-alib07-import-cancel | ALIB-07 slice 2 設計 — LIB-IMPORT-04 cancel checkpoint・subprocess crash fixture | approved | [alib-07-import-cancel-design.md](./alib-07-import-cancel-design.md) |
| DESIGN-034AUT01Kp3mWv9zR2sQfL6h | AUT-01 Touch Return・Writer State 境界設計 — session clock + 明示閉鎖 | approved | [aut-01-touch-return-writer-state-design.md](./aut-01-touch-return-writer-state-design.md) |
| DESIGN-034AUT02Qm8wXs5vLn3jKp9r | AUT-02 Region Replace 境界曲線保存 設計 — merged series 境界 anchor・host 選択・stitch | approved | [aut-02-region-replace-boundary-design.md](./aut-02-region-replace-boundary-design.md) |
| DESIGN-034AUT03B1wG9hW1zS0Yf4 | AUT-03 Loop 録音 1 Pass — Write Range 半開区間閉鎖・境界 flush・明示 Rearm 設計 | approved | [aut-03-loop-write-one-pass-design.md](./aut-03-loop-write-one-pass-design.md) |
| DESIGN-034AUT04CvbbR3Qzx1aKwY | AUT-04 Automation Curve 非破壊保持・Viewport LOD・有界記録 Event 予算 設計 | approved | [aut-04-automation-curve-lod-record-budget-design.md](./aut-04-automation-curve-lod-record-budget-design.md) |
| DESIGN-2PaoXTMFb4NbwT7TNBcUTc | Automation / Modulation 統合 Parameter Control — 設計（synvelis#6） | approved | [automation-modulation-design.md](./automation-modulation-design.md) |
| DESIGN-034VmgS1sGu9EW9NfKKRPY | Issue #245 verify-macos rt_cache alloc 計測 flake — 計測対象インスタンス warm-up 設計 | approved | [ci-245-macos-alloc-flake-design.md](./ci-245-macos-alloc-flake-design.md) |
| DESIGN-TiKdIX8rj4YqmP3Pmb58JR | Issue #377 synvelis-native / gpui-test 3OS CI OS-L1 — 既存 verify leg 拡張設計 | approved | [ci-os-l1-native-gpui.md](./ci-os-l1-native-gpui.md) |
| DESIGN-034RoWWK4Xx4NaZSoBYZSX | Synvelis 独立 CI — Lane 構成・証跡・依存アクセス設計 | approved | [ci-synvelis-pr-push-regression-gate.md](./ci-synvelis-pr-push-regression-gate.md) |
| — | Synvelis — Configurable PPQN / Resolution-independent Musical Time | — | [configurable-ppqn-musical-time-architecture.md](./configurable-ppqn-musical-time-architecture.md) |
| DESIGN-6ClrSXGAyry8iwAqt4t32c | Context Inspector 設計 — Follow/Explore/Pinned 状態機械・TargetKind Provider・情報階層・Probe/編集契約（synvelis#5） | approved | [context-inspector-design.md](./context-inspector-design.md) |
| DESIGN-034CTL01Qy1wZt7aC3dE4fGh | CTL-01 ControlOwner/AutomationTimeScope 設計 — Schema v7 Migration | approved | [ctl-01-control-owner-time-scope-design.md](./ctl-01-control-owner-time-scope-design.md) |
| DESIGN-taIsR3SbHTaGKnhYkfth7f | CTL-02 adoption SendLevel canonical spec 強制 設計 | approved | [ctl-02-adopt-sendlevel-design.md](./ctl-02-adopt-sendlevel-design.md) |
| DESIGN-034QAlPiE7atUoGKXNG7Lr | ED-A Editing Model 設計 — Selection/Tool/Snap/Gesture/LinkedContext headless PoC | approved | [ed-a-editing-model-design.md](./ed-a-editing-model-design.md) |
| DESIGN-034QN0IIBPRdrHiOv3uTvq | ED-B media-domain editing design | approved | [ed-b-media-domain-editing-design.md](./ed-b-media-domain-editing-design.md) |
| DESIGN-0Gw01GateWayContract01 | GW-01 Gateway Contract — Capability Registry・Typed Invocation・Versioned Result/Error | draft | [gw-01-gateway-contract-design.md](./gw-01-gateway-contract-design.md) |
| DESIGN-0Gw02AuthorizationPo01 | GW-02 Principal / Execution Grant / Authorization Policy — Fixed/Dynamic Scope | draft | [gw-02-authorization-policy-design.md](./gw-02-authorization-policy-design.md) |
| DESIGN-7Y20yWdQNliSve36HXn2Hy | Issue #145 ci.yml manual-only 設計 — trigger 撤去の最小差分と不変契約の境界 | approved | [issue-145-ci-manual-only.md](./issue-145-ci-manual-only.md) |
| DESIGN-034RIJlv2BuYKx5KG2VyDg | Issue #147 sccache キャッシュ分離・段階展開設計 | approved | [issue-147-sccache-compile-cache.md](./issue-147-sccache-compile-cache.md) |
| DESIGN-034SVCIOPT02kX3pLwQs | Issue #148 変更影響分類・Fail-Closed evidence Gate 設計 | approved | [issue-148-impact-classification.md](./issue-148-impact-classification.md) |
| DESIGN-034RGXfK1v0mkpD0U2xAaN | Issue #151 CI ui-verify lane 設計 | approved | [issue-151-ui-ci-lane.md](./issue-151-ui-ci-lane.md) |
| DESIGN-034RUkhVNYe0G4XOqwwqOa | Issue #157 rev.3 matrix lane skip placeholder — evidence gate 設計 | approved | [issue-157-matrix-placeholder-gate.md](./issue-157-matrix-placeholder-gate.md) |
| DESIGN-034SYN166CiSuMmRy | Issue #166 Full CI 化・ci-summary 統一 — 第一段階設計 | proposed | [issue-166-full-ci-ci-summary.md](./issue-166-full-ci-ci-summary.md) |
| DESIGN-RBfO0jdqOYWxb6xvtT5z0p | Issue #167 Tracker reconcile 設計 — Consumer 共有 checkout 適合・mode 分離・単一 pin 契約 | approved | [issue-167-tracker-reconcile-entry.md](./issue-167-tracker-reconcile-entry.md) |
| DESIGN-034U2uVeVKqECLpCn8kbSa | Issue #184 CI verify/evidence 失敗 — canonical temp fixture 設計 | approved | [issue-184-ci-verify-evidence-canonical-temp-fixture.md](./issue-184-ci-verify-evidence-canonical-temp-fixture.md) |
| DESIGN-034WsvXEZuKq8V4PoYjVxZ | Issue #365 impl index 生成可能化 — front matter 是正と marker 追加の設計 | approved | [issue-365-impl-index-generation-design.md](./issue-365-impl-index-generation-design.md) |
| DESIGN-034WuZE8vEiOZJKbZMaRJQ | Issue #368 docs/impl/ir missing-section 本文補完 — 設計 | approved | [issue-368-ir-missing-sections-design.md](./issue-368-ir-missing-sections-design.md) |
| DESIGN-vaYFK7BnCVN715EASswzTe | Issue #86 DoD(2) — verified import consumer 配線 + digest-kind blob 解決 設計 | approved | [issue-86-verified-import-wiring-design.md](./issue-86-verified-import-wiring-design.md) |
| DESIGN-yiV0z7HH1jyKUmweFh9hCT | L1 Design — Safe Import・Verified Media・Lowering・Headless Driver | approved | [l1-local-wav-verified-playback-design.md](./l1-local-wav-verified-playback-design.md) |
| DESIGN-034QmfChvqgB632y8fAydk | L2 Design — Render Snapshot・Preflight・Sequential Render・WAV Output・Reopen Verify・Safe Publish | approved | [l2-master-sequential-offline-wav-export-design.md](./l2-master-sequential-offline-wav-export-design.md) |
| DESIGN-YFKePNnF4peFnskSfrnSc2 | MD-A Design — TakeGroup/Comp スキーマ・Effective Comp 投影・正規化 Placement/Playback 時間契約 | approved | [mda-comp-effective-projection-placement-design.md](./mda-comp-effective-projection-placement-design.md) |
| — | Synvelis × Ruviform — Media / Import / Export / Offline Render / UI / Diagnostics 統合アーキテクチャ | — | [media-import-export-offline-render-ui-diagnostics-architecture.md](./media-import-export-offline-render-ui-diagnostics-architecture.md) |
| DESIGN-034MSA52Mn8xRt6yQwZkPf | 最小 MIDI Source / Note 編集契約 設計 | draft | [midi-source-note-editing-design.md](./midi-source-note-editing-design.md) |
| DESIGN-034PCA03Nj8xRs4yAbOcDe | PC-A Parameter Control Model 設計 — Scope/Breakdown/StateMachine headless PoC | approved | [pc-a-parameter-control-design.md](./pc-a-parameter-control-design.md) |
| DESIGN-034PCB03Mn8xRs4uZvWdEf | PC-B Parameter Control dispatch/note-scope/RT/offline 設計 | approved | [pc-b-parameter-dispatch-note-scope-design.md](./pc-b-parameter-dispatch-note-scope-design.md) |
| DESIGN-034RoTLCvNiYIgUG0JDqVa | PC-C1 Recording Transaction prepare/apply/finalize 設計 | approved | [pc-c1-recording-transaction-design.md](./pc-c1-recording-transaction-design.md) |
| DESIGN-034Up3Bm4bb2Rb9RZ1ien7 | PC-C2 Control Relation Index 設計 — 双方向辺モデル・状態証拠・有界 traversal | approved | [pc-c2-control-relation-index-design.md](./pc-c2-control-relation-index-design.md) |
| DESIGN-o5U9K2sOik9XcS81AkO1Qj | PC-C3-05 RT trace adapter 設計 — DispatchTraceAdapter・写像・bounded history・metrics | approved | [pc-c3-05-rt-trace-adapter-design.md](./pc-c3-05-rt-trace-adapter-design.md) |
| DESIGN-034UqtGwJKTXdIjOxuLVS2 | PC-C3 ControlTrace 設計 — stage evidence slot・freshness 判定・plan activation 投影 | approved | [pc-c3-control-trace-design.md](./pc-c3-control-trace-design.md) |
| DESIGN-PjOpAzd2cNOD9kshEbJeKl | PC-C4-02 plugin instance binding design — connect-time identity vend・stage-1 authority consumption | approved | [pc-c4-02-plugin-instance-binding-design.md](./pc-c4-02-plugin-instance-binding-design.md) |
| DSGN-PCc4030bs3vN1pQ8wXyZ2aC | PC-C4-03 param observation design — drain surface・binding 逆引き・gate | approved | [pc-c4-03-param-observation-design.md](./pc-c4-03-param-observation-design.md) |
| DSGN-PCc4040bs3vN1pQ8wXyZ2aC | PC-C4-04 app composition design — vend cell・所有 dispatch・pump 駆動・feed 橋渡し | approved | [pc-c4-04-app-composition-design.md](./pc-c4-04-app-composition-design.md) |
| DESIGN-034VvFkFH34loxhN1Sn2tn | PC-C4-05 persisted StablePluginInstanceId design — project identity store・schema v13・hydration | approved | [pc-c4-05-persisted-instance-id-design.md](./pc-c4-05-persisted-instance-id-design.md) |
| DESIGN-7c4e9a1b2d3f4e5a6b7c8e9f | PC-C4-06 control op surface design — syn.control.* typed-evidence ops | approved | [pc-c4-06-control-op-surface-design.md](./pc-c4-06-control-op-surface-design.md) |
| DESIGN-7c4e9a1b2d3f4e5a6b7c8d9e0f1a | PC-C4-07 transport mirror + observe_block design | approved | [pc-c4-07-transport-mirror-observe-design.md](./pc-c4-07-transport-mirror-observe-design.md) |
| DESIGN-a1d4f7c2e9b30586d1a4f7c2f | PC-C4 drain composition design — drain_output_reports 周期配線と production composition root | approved | [pc-c4-drain-composition-design.md](./pc-c4-drain-composition-design.md) |
| DESIGN-4IoiIeHs7TpfdoOHf3nu9E | PC-C4 plugin binding design — persisted instance authority を bind 経路へ接続 | approved | [pc-c4-plugin-binding-design.md](./pc-c4-plugin-binding-design.md) |
| DESIGN-034Uw4SG2SiRKQuoKOZGzL | PC-C4 plugin parameter binding design — verified frame base・capability gate・generation consistency | approved | [pc-c4-plugin-param-binding-design.md](./pc-c4-plugin-param-binding-design.md) |
| DESIGN-AkFUh4M60YuHo8COCvb0Um | PC-C5 slice1 Relation Inspector 設計 — RelationRow/RelationView/RelationNav + Inspector 第 2 Table | approved | [pc-c5-01-relation-inspector-design.md](./pc-c5-01-relation-inspector-design.md) |
| DESIGN-034UxXM5FBBTGn5JWkSBss | PC-C5 slice2 Gesture 固定・stale drop・Inspector mode 設計 | approved | [pc-c5-02-03-gesture-stale-inspector-design.md](./pc-c5-02-03-gesture-stale-inspector-design.md) |
| DESIGN-034WaOyEzrP97e8ooaGm2W | PC-C5 slice4 Recording status read model 設計 | approved | [pc-c5-04-recording-status-design.md](./pc-c5-04-recording-status-design.md) |
| DESIGN-mFknHHQIpULZsaBsqLEtPk | PC-C6 oracle 設計 — fixture 宣言モデル・独立参照実装・Case 対応表 | approved | [pc-c6-oracle-rel-tr-design.md](./pc-c6-oracle-rel-tr-design.md) |
| DESIGN-034VmVbkRvKggN17EiZnQ9 | PC-C6 oracle 設計 — Recording/Plugin/UI/RT fixture 宣言モデル・独立 oracle・Case 対応表 | approved | [pc-c6-oracle-rows-design.md](./pc-c6-oracle-rows-design.md) |
| DESIGN-034W1WOQzxAE3iMWUo0d49 | PC-C6 TR ledger oracle 設計 — 実経路 rig・独立 oracle・TR 行対応表 | approved | [pc-c6-tr-ledger-oracle-design.md](./pc-c6-tr-ledger-oracle-design.md) |
| DESIGN-034WxmyGWBzk3HppFdEDXE | PC-C6 TR02/TR06 realpath oracle 設計 — 既存 file への test 追加と行対応表 | approved | [pc-c6-tr02-tr06-realpath-oracle-design.md](./pc-c6-tr02-tr06-realpath-oracle-design.md) |
| — | Synvelis × Ruviform — Playback / Mixer / Generic Execution Graph Architecture | — | [playback-mixer-generic-execution-graph-architecture.md](./playback-mixer-generic-execution-graph-architecture.md) |
| DESIGN-034VG5a2ikhPyz9RCyjDDT | plugin_instances VCS semantic diff classification — keyed-synthetic PluginInstance | approved | [plugin-instances-vcs-diff-design.md](./plugin-instances-vcs-diff-design.md) |
| DESIGN-034X3jMjITd6SCoqrUyFnH | Synvelis 製品価値と中核体験 — Gate A/B/C・UX評価設計 | approved | [product-value-core-design.md](./product-value-core-design.md) |
| — | Synvelis — Project / Asset / Persistence Architecture | — | [project-asset-persistence-architecture.md](./project-asset-persistence-architecture.md) |
| DESIGN-034V2hPBihovSNoj4xusOA | RCD-01 device-recording headless fixture — Mock Input→Raw Writer 統合設計 | approved | [rcd-01-device-recording-headless-fixture.md](./rcd-01-device-recording-headless-fixture.md) |
| DESIGN-Bz2IMMfywvmEjmdIeThNvq | RCD-01 Input Adapter — binding/Arm/Monitor/channel-map と fake port fail-stop | approved | [rcd-01-input-adapter-binding-arm-monitor-fake-port.md](./rcd-01-input-adapter-binding-arm-monitor-fake-port.md) |
| DESIGN-89lmf1mLK8flCuybXMmEqR | RCD-01 実Audio Device 到達準備 design — stamp placement fallback + real backend composition root | approved | [rcd-01-real-backend-readiness-design.md](./rcd-01-real-backend-readiness-design.md) |
| DESIGN-034W8Los2DTXD6ljySt4Rf | RCD-01 実機 capture coverage 拡張 design — real-device rig + staging lane | approved | [rcd-01-real-input-coverage-design.md](./rcd-01-real-input-coverage-design.md) |
| DESIGN-034Urj7kRY8rq2nngtTSuL | RCD-02 Durability Evidence / Recovery Review / Commit Receipt | approved | [rcd-02-durability-evidence-recovery-review.md](./rcd-02-durability-evidence-recovery-review.md) |
| DESIGN-034VplpQjplSqKKaNy9r1G | RCD-02 StagedFile Publish Migration Design | approved | [rcd-02-stagedfile-publish-migration-design.md](./rcd-02-stagedfile-publish-migration-design.md) |
| DESIGN-VClnKX5mXMKj7CQFWgFYAE | RCD-02 Unsynced-Write Loss Fixture + Real-FS Probe Design | approved | [rcd-02-unsynced-loss-real-fs-probe-design.md](./rcd-02-unsynced-loss-real-fs-probe-design.md) |
| DESIGN-034VdWksbk4tfLCaf6HPoj | RCD-03 app Arm/Monitoring UX + monitor session projection | approved | [rcd-03-app-arm-monitoring-ux.md](./rcd-03-app-arm-monitoring-ux.md) |
| DESIGN-034WCrWfgXKQV2I7XmJGE0 | RCD-03 monitor engine — MonitorSession/MonitorEngine design | approved | [rcd-03-monitor-engine.md](./rcd-03-monitor-engine.md) |
| DESIGN-034VpQd8lN3xO0wS5uZ7vK | RCD-03 Arm/Monitor/Punch Project 永続投影 design | approved | [rcd-03-monitor-persistence-design.md](./rcd-03-monitor-persistence-design.md) |
| DESIGN-034X1y3Clx3u80e2MVL9IH | RCD-03 monitor/punch fake session fixture design | approved | [rcd-03-monitor-punch-design.md](./rcd-03-monitor-punch-design.md) |
| DESIGN-034UtlI2uLUF1lr2g3CUM0 | RCD-03 Monitor Policy / Punch Gate Probe / Player Monitor-Latency Adapter | approved | [rcd-03-monitoring-punch-latency.md](./rcd-03-monitoring-punch-latency.md) |
| DESIGN-rprIAB9zoxNjZbw6rODTWf | RCD-03 VCS semantic diff monitor 宣言分類 design | approved | [rcd-03-vcs-diff-monitor-classification-design.md](./rcd-03-vcs-diff-monitor-classification-design.md) |
| DESIGN-RCD04Fx2nB8cY6dF5hJ9kM3q | RCD-04 Comp Transition / MIDI Phrase fixture — crate 内完結設計 | approved | [rcd-04-comp-transition-midi-phrase-fixture.md](./rcd-04-comp-transition-midi-phrase-fixture.md) |
| DESIGN-034WEV4lGiVlfIG0LsEBNr | RCD-04 MIDI phrase boundary / Undo-Redo / VCS stable-ID design | approved | [rcd-04-midi-undo-stableid-design.md](./rcd-04-midi-undo-stableid-design.md) |
| DESIGN-034VmNZvp2r11R0Phmdf0P | RCD-04 product E2E design | approved | [rcd-04-product-e2e-design.md](./rcd-04-product-e2e-design.md) |
| DESIGN-034VBU0PCNZXCZsyRneE3S | RCD-04 RT engine comp PCM design | approved | [rcd-04-rt-engine-comp-pcm-design.md](./rcd-04-rt-engine-comp-pcm-design.md) |
| DESIGN-034SF01Mo9rSt4uV1wXyZaCdE | SF-01 Semantic Routing Model 設計 — 型・整合・Schema v6 Migration | approved | [sf-01-semantic-routing-model-design.md](./sf-01-semantic-routing-model-design.md) |
| DESIGN-034SF02Vy9qNo6pS9uA4bCeFg | SF-02 RoutingTransaction/TempRef Staging・二層検証・Mute/Solo Audibility 設計 | approved | [sf-02-routing-transaction-design.md](./sf-02-routing-transaction-design.md) |
| DESIGN-034Ux9PPNPAMU9m3oovJh8 | SF-03 semantic→Ruviform runtime 写像 — 責務境界・世代証拠・失敗不変条件・検証計画 | approved | [sf-03-map-semantic-runtime-mapping-design.md](./sf-03-map-semantic-runtime-mapping-design.md) |
| DESIGN-106SF03Ef3fGh4iJk5lMn6oPq | SF-03 slice1 — Lowering/SourceMap/Activation 実装設計 | approved | [sf-03-slice1-lowering-sourcemap-activation-design.md](./sf-03-slice1-lowering-sourcemap-activation-design.md) |
| DESIGN-106SF03Dz2aBc3dEf4gHi5jKl6 | SF-03 slice2 設計 — RoutingFault 18分類・Last-good/Retry・reconciliation | approved | [sf-03-slice2-fault-reconcile-design.md](./sf-03-slice2-fault-reconcile-design.md) |
| DESIGN-034UfwVzxaw6WwUYS3nMDN | SF-03 slice3 設計 — processor host 接続 boundary・世代連動 teardown | approved | [sf-03-slice3-plugin-host-connect-design.md](./sf-03-slice3-plugin-host-connect-design.md) |
| DESIGN-6tbKfWyun38H7ERAnefMsA | SF-03 slice4 設計 — NoRebuild Equivalence・世代連動 processor control 払い出し | approved | [sf-03-slice4-act-acceptance-design.md](./sf-03-slice4-act-acceptance-design.md) |
| DESIGN-034Uj7LfhkkACvyn53L7eN | SF-03 slice5 設計 — session EnginePort・SessionRoutingEngine・canonical probe | approved | [sf-03-slice5-session-engine-connect-design.md](./sf-03-slice5-session-engine-connect-design.md) |
| DESIGN-034UJqgcnnlXJWllu9yUGb | SF-03 slice6 design — RoutingControlDispatch control surface（世代 gate・有界 queue・binding 証拠・catalog refresh） | approved | [sf-03-slice6-act06-control-surface-design.md](./sf-03-slice6-act06-control-surface-design.md) |
| DESIGN-poso4hBPDap0Gs9cisA92Y | SF-03 slice7 — render semantic-lowering 消費 design | approved | [sf-03-slice7-render-consumption-design.md](./sf-03-slice7-render-consumption-design.md) |
| DESIGN-034VoSreRK4VhuFxIqvayx | SF-03 slice8 — app/workspace same-Revision fixture design | approved | [sf-03-slice8-revision-fixture-design.md](./sf-03-slice8-revision-fixture-design.md) |
| DESIGN-034VWsBTFsvYFbqeDihe3Z | SF-03 slice9 — host capability generation live firing design | approved | [sf-03-slice9-capability-firing-design.md](./sf-03-slice9-capability-firing-design.md) |
| DESIGN-CwatwmnW371rKsz3F2ajlh | SF-04 probe adapter — Issue #107 SF-PROBE-01..08 受入集約設計 | approved | [sf-04-probe-adapter-design.md](./sf-04-probe-adapter-design.md) |
| DESIGN-034Um5591oRMfPg7cVthyU | SF-04 slice1 設計 — Semantic Probe Adapter（project contract + player runtime adapter） | approved | [sf-04-slice1-semantic-probe-adapter-design.md](./sf-04-slice1-semantic-probe-adapter-design.md) |
| DESIGN-9kRtY3wQzX8cV6bNmLp2Sd | SF-04 slice10 設計 — syn.probe.compare app op（wire 契約・read-only 判定・JSON mapper） | approved | [sf-04-slice10-probe-compare-op-design.md](./sf-04-slice10-probe-compare-op-design.md) |
| DESIGN-034UunDPfVm34dBXKZHHG0 | SF-04 slice2 設計 — ControlSession probe observation → ControlTrace 配線 | approved | [sf-04-slice2-controlsession-probe-trace-design.md](./sf-04-slice2-controlsession-probe-trace-design.md) |
| DESIGN-034UL2WkinplQmMD9VPyeX | SF-04 slice3 設計 — probe clock epoch discontinuity・capture segment cut | approved | [sf-04-slice3-clock-epoch-discontinuity-design.md](./sf-04-slice3-clock-epoch-discontinuity-design.md) |
| DESIGN-4AUoTxfZvkN7cj3NjLptJ2 | SF-04 slice4 設計 — app/session wiring（probe port・syn.probe.*・drain hook） | approved | [sf-04-slice4-app-wiring-design.md](./sf-04-slice4-app-wiring-design.md) |
| DESIGN-034VpkqegnYxvgZbOSsE3K | SF-04 slice5 設計 — probe subscriber/window ownership（session 層 lease・capture owner） | approved | [sf-04-slice5-probe-subscriber-ownership-design.md](./sf-04-slice5-probe-subscriber-ownership-design.md) |
| DESIGN-3kM8vBnQ2wX7yT1zR5pL9d | SF-04 slice6 設計 — 複数 Probe 整列比較 consumer（ProbeComparisonRequest → AcousticObservation） | approved | [sf-04-slice6-probe-compare-design.md](./sf-04-slice6-probe-compare-design.md) |
| DESIGN-7qW2mX5cV9bN4tY8zK1jH6f | SF-04 slice8 設計 — SF-PROBE-05 fault fixtures + seek 以外の不連続宣言点 | approved | [sf-04-slice8-probe-faults-declare-design.md](./sf-04-slice8-probe-faults-declare-design.md) |
| DESIGN-034UudJz8OJsiidXryTNTU | SF-05 slice1 Signal Flow headless 設計 — graph 投影 + visual state + status 合成 | approved | [sf-05-slice1-signal-flow-headless-graph-visual-state-status.md](./sf-05-slice1-signal-flow-headless-graph-visual-state-status.md) |
| DESIGN-CJVcdyUweKy4i6p0xOHdJc | SF-05 slice2 設計 — routing gesture fixation/preview/commit + inspection focus + schema v3 persist | approved | [sf-05-slice2-signal-flow-ux0307-gesture-inspection-persist.md](./sf-05-slice2-signal-flow-ux0307-gesture-inspection-persist.md) |
| DESIGN-mB7tYwKsPqNvXcZaE5rGjH | SF-05 slice3 設計 — probe subscriber/capture dedup の app/UI 配線 | approved | [sf-05-slice3-probe-subscriber-wiring-design.md](./sf-05-slice3-probe-subscriber-wiring-design.md) |
| DESIGN-034V2wuzXfqcjIvGyXFhhz | SF-06 cross-gate headless 設計 — oracle 独立性・fixture topology・ledger schema | approved | [sf-06-cross-gate-oracle-domain-evidence.md](./sf-06-cross-gate-oracle-domain-evidence.md) |
| DESIGN-OIDrCGhRjzwr0IzN1tzVkg | SF-XR-01 Design — CTL-01/CTL-02 cross-gate oracle fixture | approved | [sfxr01-ctl-crossgate-oracle-fixture.md](./sfxr01-ctl-crossgate-oracle-fixture.md) |
| DESIGN-034X5yWfKUcmf5eqz7NUJR | Issue #84 StagedFile Migration 実装済み設計 | approved | [stagedfile-migration-design.md](./stagedfile-migration-design.md) |
| DESIGN-jmv4N50pKcC7sDGSX5TVEq | Issue #245 webdav MockServer TransientIo flake — accept 済みソケット blocking 正規化設計 | approved | [sv-ci-webdav-transient-io-flake-design.md](./sv-ci-webdav-transient-io-flake-design.md) |
| DESIGN-lW54zR76NmEm8cc3NlMskX | SV-GUI-01 Design — Desktop App Bootstrap・Composition Root・Frontend Asset Pipeline | approved | [sv-gui-01-desktop-app-bootstrap-design.md](./sv-gui-01-desktop-app-bootstrap-design.md) |
| DESIGN-034RAAXZ1XMIuk2zr2dr4w | SV-GUI-02 Frontend View Model・Binding IPC・Reference Workspace View | approved | [sv-gui-02-frontend-view-model-binding-ipc-reference-workspace-view.md](./sv-gui-02-frontend-view-model-binding-ipc-reference-workspace-view.md) |
| DESIGN-5n53fkGhYaUtJlMHOu5P1p | SV-GUI-03 slice1 Transport/Inspector 設計 — MainScreenState push + 既存 syn.control.* 経路 | approved | [sv-gui-03-01-transport-inspector-slice1-design.md](./sv-gui-03-01-transport-inspector-slice1-design.md) |
| DESIGN-8o9zUv4L5ae0FBB0zOM293 | SV-GUI-03 slice2 Arrangement/Undo 設計 — workspace read model + syn.project.* op 露出 | approved | [sv-gui-03-02-arrangement-undo-slice2-design.md](./sv-gui-03-02-arrangement-undo-slice2-design.md) |
| DESIGN-FQvwpPOrC7St8RoKqyYNMK | SV-GUI-03 slice3 Mixer strip Toggle 設計 — role+owner 走査・pump reconcile・atomic Set | approved | [sv-gui-03-03-mixer-strip-toggle-slice3-design.md](./sv-gui-03-03-mixer-strip-toggle-slice3-design.md) |
| DESIGN-Mx7pQr2wNz4kLs8tVb6yHd | SV-GUI-03 slice4 設計 — row selection→syn.view.select→Inspector rebinding + syn.viz.* surface reconcile | approved | [sv-gui-03-04-selection-inspector-viz-design.md](./sv-gui-03-04-selection-inspector-viz-design.md) |
| DESIGN-Ht8wEr4xQz2nBs6vYp9dMc | SV-GUI-03 slice5 設計 — driver-neutral scenario 抽出 + GpuiGuiTest lane | approved | [sv-gui-03-05-gpui-scenario-design.md](./sv-gui-03-05-gpui-scenario-design.md) |
| DESIGN-034UogNAUQrEmBkgZnhrAk | SV-GUI-04 slice1 precursor 設計 — HeadlessWindowHost + dock fixture | approved | [sv-gui-04-01-headless-window-handle-dock-design.md](./sv-gui-04-01-headless-window-handle-dock-design.md) |
| DESIGN-Ywl9hP3wdlHWjoMPB7Hvjn | SV-GUI-04 slice1 設計 — 2nd view spec + env-gated 2窓 mount + FakeDriver fixture | approved | [sv-gui-04-01-two-window-slice1-design.md](./sv-gui-04-01-two-window-slice1-design.md) |
| DESIGN-034UqwNDJrureYsAqmckuw | SV-GUI-04 slice2 設計 — close_guard op + 末尾 window veto | approved | [sv-gui-04-02-close-veto-design.md](./sv-gui-04-02-close-veto-design.md) |
| DESIGN-034UqNokCXcUXgKk1tyv0p | SV-GUI-04 slice2 dock transfer: workspace adoption と dock/UI 権威分界の詳細設計 | approved | [sv-gui-04-slice2-dock-transfer-workspace-adoption-design.md](./sv-gui-04-slice2-dock-transfer-workspace-adoption-design.md) |
| DESIGN-BWzJVIfJHZS8suU7aFFAGf | SV175 Native App Shell 設計 — NativeShell routing・ViewBinding adapter・native feature lane | approved | [sv175-native-shell-design.md](./sv175-native-shell-design.md) |
| DESIGN-034T728YcMrGoBwDtbkIqS | SV177 Product Theme / Workspace Preset — synvelis-theme crate・DTCG overrides・slot-based preset・semantic screen spec | approved | [sv177-product-theme-workspace-preset-design.md](./sv177-product-theme-workspace-preset-design.md) |
| DESIGN-Xk9mQ2vR7wTzLp4nHs8dFj | SV185-01 mixer.gain projection 設計 — ControlModel bootstrap・wire target・display_value | approved | [sv185-mixer-gain-projection-design.md](./sv185-mixer-gain-projection-design.md) |
| DESIGN-034VobNphX5isOqTl9Wg5i | SV#28 Ruviform pin 更新 設計 — lockfile pin 更新と API 追従の境界 | approved | [sv28-ruviform-pin-b02942c2.md](./sv28-ruviform-pin-b02942c2.md) |
| DESIGN-43KJVuVRlioN1JtUFVuqtW | SV#28 Ruviform pin 更新 設計 — lockfile pin 更新と API 追従の境界 | approved | [sv28-ruviform-pin-cc7d5296.md](./sv28-ruviform-pin-cc7d5296.md) |
| DESIGN-AyDQMCLnAv4MbV3s4qnT8D | SV#28 Ruviform pin 更新 設計 — lockfile pin 更新と API 追従の境界（9865cd57 → fd1d4d5f） | approved | [sv28-ruviform-pin-fd1d4d5f.md](./sv28-ruviform-pin-fd1d4d5f.md) |
| DSGN-SvtE2e02aB2cD3eF4gH5iJ7 | SV-GUI-05: SVT-01〜09 Headless UI Scenario 設計 | approved | [svt-product-e2e-design.md](./svt-product-e2e-design.md) |
| DESIGN-034Uvylguz0kZUgy1zbseh | SYN-IO-01 Ruviform 共通 I/O 適合境界設計 | approved | [syn-io-01-ruviform-io-boundary-design.md](./syn-io-01-ruviform-io-boundary-design.md) |
| DESIGN-034T2aGDbNinkumWeftaiP | SYN-IO-02 DirectoryBundleStore StagedFile 移行設計 | approved | [syn-io-02-directory-bundle-store-stagedfile-design.md](./syn-io-02-directory-bundle-store-stagedfile-design.md) |
| DESIGN-034Tk3mXw7LpQ2vRnB8y | SYN-IO-02 HEAD CAS consumer-side mutex 設計 | approved | [syn-io-02-head-cas-design.md](./syn-io-02-head-cas-design.md) |
| DESIGN-034WVJwLjBMYR3fBrNJD2M | SYN-IO-02 HEAD CAS 実 2-process E2E 検証 — design | approved | [syn-io-02-head-cas-e2e-design.md](./syn-io-02-head-cas-e2e-design.md) |
| DESIGN-034RBzdpo91wcbqBK7Gb9t | SYN-OBS-01 Design — Host所有 Logging Owner・設定解決・Source Catalog | approved | [syn-obs-01-design-host-logging-owner-source-catalog.md](./syn-obs-01-design-host-logging-owner-source-catalog.md) |
| DESIGN-034T2RF305WAVSTAGED00Y | SYN-RF305 WAV Export StagedFile 移行設計 | approved | [syn-rf305-wav-export-staged-file-design.md](./syn-rf305-wav-export-staged-file-design.md) |
| — | Synvelis-driven Ruviform Target Architecture v0.7.1 | — | [synvelis-driven-ruviform-target-architecture-v07.md](./synvelis-driven-ruviform-target-architecture-v07.md) |
| DESIGN-034VtzPNCX6aVKTaxy71ke | TIM-01 app 層 PPQN 統合 — syn.project.set_ppqn 設計 | approved | [tim-01-app-ppqn-design.md](./tim-01-app-ppqn-design.md) |
| DESIGN-034UHVNvFQXFumTOowIt0g | TIM-01 Configurable PPQN — Rational Beat 再投影設計 | approved | [tim-01-configurable-ppqn-design.md](./tim-01-configurable-ppqn-design.md) |
| DESIGN-JxGLd6Q5TTckxCpvsulzYJ | TIM-01 PPQN config — punch marks 再投影と受入残件検証設計 | approved | [tim-01-ppqn-config-design.md](./tim-01-ppqn-config-design.md) |
| DESIGN-4kLmNpQrStUvWxYz0123 | TIM-01 PPQN Preview op + New Project PPQN — 設計 | approved | [tim-01-ppqn-preview-design.md](./tim-01-ppqn-preview-design.md) |
| DESIGN-TiM01uIxYzAbCdEfGh2 | TIM-01 PPQN 選択 UI + Snap/Grid ポリシー — 設計 | approved | [tim-01-ppqn-ui-design.md](./tim-01-ppqn-ui-design.md) |
| DESIGN-126tim02playerMidiRt01 | TIM-02 player MIDI RT/Offline 結合 — crate 内完結設計 | approved | [tim-02-player-midi-rt-offline.md](./tim-02-player-midi-rt-offline.md) |
| DESIGN-TiM02PpqnSmfXg02 | TIM-02 × TIM-01 PPQN×SMF cross-golden fixture — 設計 | approved | [tim-02-ppqn-smf-cross-golden-design.md](./tim-02-ppqn-smf-cross-golden-design.md) |
| DESIGN-034UwVoRGI8Bp9zuzRFQhj | TIM-02 SMF Format0/1 core — crate 内完結設計 | approved | [tim-02-smf-format01-core.md](./tim-02-smf-format01-core.md) |
| DESIGN-MsBu5XoYY4Ex3QB0qmDvC5 | TIM-03 session→driver/routing wiring design — session 寿命・provenance mint・transport mirror | approved | [tim-03-session-driver-wiring-design.md](./tim-03-session-driver-wiring-design.md) |
| DESIGN-034UC0ixTsN58rRlRctgAH | TIM-04 plugin state snapshot consistency design — quality receipt/store/session/offline recipe | approved | [tim-04-plugin-state-snapshot-consistency-design.md](./tim-04-plugin-state-snapshot-consistency-design.md) |
| DESIGN-034UyEjdALaapQRSk5hAdk | TIM-03 plugin process boundary design — capability/degraded policy oracle | approved | [tim03-process-boundary-design.md](./tim03-process-boundary-design.md) |
| — | Synvelis × Ruviform — Time / Transport / MIDI / Automation / Plugin Integration Architecture v0.5 | — | [time-transport-midi-automation-plugin-integration-architecture.md](./time-transport-midi-automation-plugin-integration-architecture.md) |
| DESIGN-102UiCfInt002BbCcDdEeF | UI-C5 製品 GUI 統合 設計 — syn.control.relation / syn.control.edge / syn.recording.* op surface | approved | [ui-sf-control-integration-design.md](./ui-sf-control-integration-design.md) |
| DESIGN-034UAwTwlzWbToxqWtFM2B | VCS-01 Immutable Snapshot Resolver — crate 内完結設計 | approved | [vcs-01-immutable-snapshot-resolver-design.md](./vcs-01-immutable-snapshot-resolver-design.md) |
| DESIGN-034UEtYMoQUWsPmOBrl819 | VCS-02 Semantic Diff Engine design | approved | [vcs-02-semantic-diff-engine-design.md](./vcs-02-semantic-diff-engine-design.md) |
| DESIGN-7jUOgXVjk3gpAYkbIzdUfe | VCS-03 Git Read Provider design | approved | [vcs-03-git-read-provider-design.md](./vcs-03-git-read-provider-design.md) |
| DESIGN-UpOaSmwX-5aEOp4rEEmXG | synvelis#335 unborn HEAD 誤分類修正 — rev-parse --quiet 除去と NotFound 経路回復設計 | approved | [vcs-03-unborn-head-classification-design.md](./vcs-03-unborn-head-classification-design.md) |
| DESIGN-VCS04BkndWiringDsn0001 | VCS-04 production backend wiring — module 設計 | approved | [vcs-04-backend-wiring-design.md](./vcs-04-backend-wiring-design.md) |
| DESIGN-VCS04Hist0ryV1ewDsn01 | VCS-04 History/Visual Diff UI design | approved | [vcs-04-history-visual-diff-ui-design.md](./vcs-04-history-visual-diff-ui-design.md) |
| DESIGN-VCS04NativeMount001 | VCS-04 History/Visual Diff — native mount/dock 配線 design | approved | [vcs-04-native-mount-design.md](./vcs-04-native-mount-design.md) |
| DSGN-vcs05-apply-restore-headless | VCS-05 Apply/Restore headless — module 設計 | approved | [vcs-05-apply-restore-headless-design.md](./vcs-05-apply-restore-headless-design.md) |
| DSGN-vcs05-monitoring-apply-surface | VCS-05 monitoring kinds apply surface 拡張 design | approved | [vcs-05-monitoring-apply-surface-design.md](./vcs-05-monitoring-apply-surface-design.md) |
| DESIGN-TR6hK0f243aEk3CmFQlfp | VCS-06 evidence completion — RESTORE-08 / COMPAT-03/04 設計 | approved | [vcs-06-evidence-completion-design.md](./vcs-06-evidence-completion-design.md) |
| DESIGN-lzcGjnJjPNFKiNNUpE2gE3 | VCS-06 monitoring-domain matrix — fixture 設計 | approved | [vcs-06-monitoring-matrix-design.md](./vcs-06-monitoring-matrix-design.md) |
| DESIGN-034WTsUdd5SHHSF6EBJmfK | VCS-06 real-git/FS fault rows — design | approved | [vcs-06-real-git-fs-fault-design.md](./vcs-06-real-git-fs-fault-design.md) |
| DESIGN-034VpC0m5WnwXqJkI2nNXw | VCS-06 UI headless matrix — fixture 設計 | approved | [vcs-06-ui-headless-matrix-design.md](./vcs-06-ui-headless-matrix-design.md) |
| DESIGN-034QdO7AB6cp4DRgKeuuEn | VS-A Project/Recording Foundation — Stable ID・時間型・Command・永続化 PoC | approved | [vs-a-project-recording-foundation-stable-id-command-poc.md](./vs-a-project-recording-foundation-stable-id-command-poc.md) |
| DESIGN-8OW4JtYUYaLPOTZ9ZGxXuz | VS-B Clock / Gate Scheduler / RT Capture — Mock Input・Buffer Pool・SPSC | approved | [vs-b-clock-gate-rt-capture-mock-spsc.md](./vs-b-clock-gate-rt-capture-mock-spsc.md) |
| DESIGN-034QfL0bH9u3cVqK0pYwT7x | VS-C Staging Writer / Journal / Crash Recovery — Verified Media Range | approved | [vs-c-staging-writer-journal-crash-recovery-verified-media-range.md](./vs-c-staging-writer-journal-crash-recovery-verified-media-range.md) |
| DESIGN-sbd87tLPXRrvP2kyru1XD2 | VS-D Recording Commit / E2E — Idempotent Commit・Undo・Reload 設計 | approved | [vs-d-recording-commit-e2e-idempotency-undo-reload.md](./vs-d-recording-commit-e2e-idempotency-undo-reload.md) |
| — | Synvelis Workspace / Docking / Multi-Window 統合設計 | — | [workspace-docking-state-architecture.md](./workspace-docking-state-architecture.md) |
| DESIGN-034X4TrgrbyPHCdc8l0xPg | Workspace Model Design — GPUI-neutral View/Driver Port, lifetime independence, routing | approved | [workspace-model-design.md](./workspace-model-design.md) |
| DESIGN-034X3lVqc2s7Qx5w6YylGa | Workspace UX 操作仕様 — Docking/Window・View 閉鎖/Inspector/Link Group/入力境界/Layout 復帰 (Issue #3 正本) | approved | [workspace-ux-scenarios-design.md](./workspace-ux-scenarios-design.md) |
| DESIGN-034QwYMaHNxxpkZgLxPjx6 | WS-A Workspace 基盤設計 — View/Selection/LinkGroup/Command/Dock headless PoC | approved | [ws-a-workspace-foundation-design.md](./ws-a-workspace-foundation-design.md) |
| DESIGN-HEDK2Gt2m6FAc3GREIhJDl | X Design — Canonical publication receipt・commit digest 接続・同一経路 playback/export | approved | [x-recorded-verified-source-playback-export-design.md](./x-recorded-verified-source-playback-export-design.md) |

| DESIGN-mFknHHQIpULZsaBsqLEtPk | PC-C6 oracle 設計 — fixture 宣言モデル・独立参照実装・Case 対応表 | approved | [pc-c6-oracle-rel-tr-design.md](./pc-c6-oracle-rel-tr-design.md) |
| DESIGN-AkFUh4M60YuHo8COCvb0Um | PC-C5 slice1 Relation Inspector 設計 — RelationRow/RelationView/RelationNav + Inspector 第 2 Table | approved | [pc-c5-01-relation-inspector-design.md](./pc-c5-01-relation-inspector-design.md) |
| — | Synvelis × Ruviform — Playback / Mixer / Generic Execution Graph Architecture | — | [playback-mixer-generic-execution-graph-architecture.md](./playback-mixer-generic-execution-graph-architecture.md) |
| — | Synvelis — Project / Asset / Persistence Architecture | — | [project-asset-persistence-architecture.md](./project-asset-persistence-architecture.md) |
| DESIGN-Bz2IMMfywvmEjmdIeThNvq | RCD-01 Input Adapter — binding/Arm/Monitor/channel-map と fake port fail-stop | approved | [rcd-01-input-adapter-binding-arm-monitor-fake-port.md](./rcd-01-input-adapter-binding-arm-monitor-fake-port.md) |
| DESIGN-034Urj7kRY8rq2nngtTSuL | RCD-02 Durability Evidence / Recovery Review / Commit Receipt | approved | [rcd-02-durability-evidence-recovery-review.md](./rcd-02-durability-evidence-recovery-review.md) |
| DESIGN-034UtlI2uLUF1lr2g3CUM0 | RCD-03 Monitor Policy / Punch Gate Probe / Player Monitor-Latency Adapter | approved | [rcd-03-monitoring-punch-latency.md](./rcd-03-monitoring-punch-latency.md) |
| DESIGN-RCD04Fx2nB8cY6dF5hJ9kM3q | RCD-04 Comp Transition / MIDI Phrase fixture — crate 内完結設計 | approved | [rcd-04-comp-transition-midi-phrase-fixture.md](./rcd-04-comp-transition-midi-phrase-fixture.md) |
| DESIGN-034SF01Mo9rSt4uV1wXyZaCdE | SF-01 Semantic Routing Model 設計 — 型・整合・Schema v6 Migration | approved | [sf-01-semantic-routing-model-design.md](./sf-01-semantic-routing-model-design.md) |
| DESIGN-034SF02Vy9qNo6pS9uA4bCeFg | SF-02 RoutingTransaction/TempRef Staging・二層検証・Mute/Solo Audibility 設計 | approved | [sf-02-routing-transaction-design.md](./sf-02-routing-transaction-design.md) |
| DESIGN-106SF03Ef3fGh4iJk5lMn6oPq | SF-03 slice1 — Lowering/SourceMap/Activation 実装設計 | approved | [sf-03-slice1-lowering-sourcemap-activation-design.md](./sf-03-slice1-lowering-sourcemap-activation-design.md) |
| DESIGN-106SF03Dz2aBc3dEf4gHi5jKl6 | SF-03 slice2 設計 — RoutingFault 18分類・Last-good/Retry・reconciliation | approved | [sf-03-slice2-fault-reconcile-design.md](./sf-03-slice2-fault-reconcile-design.md) |
| DESIGN-034UfwVzxaw6WwUYS3nMDN | SF-03 slice3 設計 — processor host 接続 boundary・世代連動 teardown | approved | [sf-03-slice3-plugin-host-connect-design.md](./sf-03-slice3-plugin-host-connect-design.md) |
| DESIGN-6tbKfWyun38H7ERAnefMsA | SF-03 slice4 設計 — NoRebuild Equivalence・世代連動 processor control 払い出し | approved | [sf-03-slice4-act-acceptance-design.md](./sf-03-slice4-act-acceptance-design.md) |
| DESIGN-034Uj7LfhkkACvyn53L7eN | SF-03 slice5 設計 — session EnginePort・SessionRoutingEngine・canonical probe | approved | [sf-03-slice5-session-engine-connect-design.md](./sf-03-slice5-session-engine-connect-design.md) |
| DESIGN-034Um5591oRMfPg7cVthyU | SF-04 slice1 設計 — Semantic Probe Adapter（project contract + player runtime adapter） | approved | [sf-04-slice1-semantic-probe-adapter-design.md](./sf-04-slice1-semantic-probe-adapter-design.md) |
| DESIGN-lW54zR76NmEm8cc3NlMskX | SV-GUI-01 Design — Desktop App Bootstrap・Composition Root・Frontend Asset Pipeline | approved | [sv-gui-01-desktop-app-bootstrap-design.md](./sv-gui-01-desktop-app-bootstrap-design.md) |
| DESIGN-034RAAXZ1XMIuk2zr2dr4w | SV-GUI-02 Frontend View Model・Binding IPC・Reference Workspace View | approved | [sv-gui-02-frontend-view-model-binding-ipc-reference-workspace-view.md](./sv-gui-02-frontend-view-model-binding-ipc-reference-workspace-view.md) |
| DESIGN-5n53fkGhYaUtJlMHOu5P1p | SV-GUI-03 slice1 Transport/Inspector 設計 — MainScreenState push + 既存 syn.control.* 経路 | approved | [sv-gui-03-01-transport-inspector-slice1-design.md](./sv-gui-03-01-transport-inspector-slice1-design.md) |
| DESIGN-8o9zUv4L5ae0FBB0zOM293 | SV-GUI-03 slice2 Arrangement/Undo 設計 — workspace read model + syn.project.* op 露出 | approved | [sv-gui-03-02-arrangement-undo-slice2-design.md](./sv-gui-03-02-arrangement-undo-slice2-design.md) |
| DESIGN-034UogNAUQrEmBkgZnhrAk | SV-GUI-04 slice1 precursor 設計 — HeadlessWindowHost + dock fixture | approved | [sv-gui-04-01-headless-window-handle-dock-design.md](./sv-gui-04-01-headless-window-handle-dock-design.md) |
| DESIGN-Ywl9hP3wdlHWjoMPB7Hvjn | SV-GUI-04 slice1 設計 — 2nd view spec + env-gated 2窓 mount + FakeDriver fixture | approved | [sv-gui-04-01-two-window-slice1-design.md](./sv-gui-04-01-two-window-slice1-design.md) |
| DESIGN-034UqwNDJrureYsAqmckuw | SV-GUI-04 slice2 設計 — close_guard op + 末尾 window veto | approved | [sv-gui-04-02-close-veto-design.md](./sv-gui-04-02-close-veto-design.md) |
| DESIGN-034UqNokCXcUXgKk1tyv0p | SV-GUI-04 slice2 dock transfer: workspace adoption と dock/UI 権威分界の詳細設計 | approved | [sv-gui-04-slice2-dock-transfer-workspace-adoption-design.md](./sv-gui-04-slice2-dock-transfer-workspace-adoption-design.md) |
| DESIGN-BWzJVIfJHZS8suU7aFFAGf | SV175 Native App Shell 設計 — NativeShell routing・ViewBinding adapter・native feature lane | approved | [sv175-native-shell-design.md](./sv175-native-shell-design.md) |
| DESIGN-034T728YcMrGoBwDtbkIqS | SV177 Product Theme / Workspace Preset — synvelis-theme crate・DTCG overrides・slot-based preset・semantic screen spec | approved | [sv177-product-theme-workspace-preset-design.md](./sv177-product-theme-workspace-preset-design.md) |
| DESIGN-Xk9mQ2vR7wTzLp4nHs8dFj | SV185-01 mixer.gain projection 設計 — ControlModel bootstrap・wire target・display_value | approved | [sv185-mixer-gain-projection-design.md](./sv185-mixer-gain-projection-design.md) |
| DESIGN-43KJVuVRlioN1JtUFVuqtW | SV#28 Ruviform pin 更新 設計 — lockfile pin 更新と API 追従の境界 | approved | [sv28-ruviform-pin-cc7d5296.md](./sv28-ruviform-pin-cc7d5296.md) |
| DESIGN-034T2aGDbNinkumWeftaiP | SYN-IO-02 DirectoryBundleStore StagedFile 移行設計 | approved | [syn-io-02-directory-bundle-store-stagedfile-design.md](./syn-io-02-directory-bundle-store-stagedfile-design.md) |
| DESIGN-034RBzdpo91wcbqBK7Gb9t | SYN-OBS-01 Design — Host所有 Logging Owner・設定解決・Source Catalog | approved | [syn-obs-01-design-host-logging-owner-source-catalog.md](./syn-obs-01-design-host-logging-owner-source-catalog.md) |
| DESIGN-034T2RF305WAVSTAGED00Y | SYN-RF305 WAV Export StagedFile 移行設計 | approved | [syn-rf305-wav-export-staged-file-design.md](./syn-rf305-wav-export-staged-file-design.md) |
| — | Synvelis-driven Ruviform Target Architecture v0.7.1 | — | [synvelis-driven-ruviform-target-architecture-v07.md](./synvelis-driven-ruviform-target-architecture-v07.md) |
| — | Synvelis × Ruviform — Time / Transport / MIDI / Automation / Plugin Integration Architecture v0.5 | — | [time-transport-midi-automation-plugin-integration-architecture.md](./time-transport-midi-automation-plugin-integration-architecture.md) |
| DESIGN-034QdO7AB6cp4DRgKeuuEn | VS-A Project/Recording Foundation — Stable ID・時間型・Command・永続化 PoC | approved | [vs-a-project-recording-foundation-stable-id-command-poc.md](./vs-a-project-recording-foundation-stable-id-command-poc.md) |
| DESIGN-8OW4JtYUYaLPOTZ9ZGxXuz | VS-B Clock / Gate Scheduler / RT Capture — Mock Input・Buffer Pool・SPSC | approved | [vs-b-clock-gate-rt-capture-mock-spsc.md](./vs-b-clock-gate-rt-capture-mock-spsc.md) |
| DESIGN-034QfL0bH9u3cVqK0pYwT7x | VS-C Staging Writer / Journal / Crash Recovery — Verified Media Range | approved | [vs-c-staging-writer-journal-crash-recovery-verified-media-range.md](./vs-c-staging-writer-journal-crash-recovery-verified-media-range.md) |
| DESIGN-sbd87tLPXRrvP2kyru1XD2 | VS-D Recording Commit / E2E — Idempotent Commit・Undo・Reload 設計 | approved | [vs-d-recording-commit-e2e-idempotency-undo-reload.md](./vs-d-recording-commit-e2e-idempotency-undo-reload.md) |
| — | Synvelis Workspace / Docking / Multi-Window 統合設計 | — | [workspace-docking-state-architecture.md](./workspace-docking-state-architecture.md) |
| DESIGN-034QwYMaHNxxpkZgLxPjx6 | WS-A Workspace 基盤設計 — View/Selection/LinkGroup/Command/Dock headless PoC | approved | [ws-a-workspace-foundation-design.md](./ws-a-workspace-foundation-design.md) |
| DESIGN-HEDK2Gt2m6FAc3GREIhJDl | X Design — Canonical publication receipt・commit digest 接続・同一経路 playback/export | approved | [x-recorded-verified-source-playback-export-design.md](./x-recorded-verified-source-playback-export-design.md) |
| DSGN-vcs05-apply-restore-headless | VCS-05 Apply/Restore headless — module 設計 | approved | [vcs-05-apply-restore-headless-design.md](./vcs-05-apply-restore-headless-design.md) |
| DSGN-PCc4030bs3vN1pQ8wXyZ2aC | PC-C4-03 param observation design — drain surface・binding 逆引き・gate | approved | [pc-c4-03-param-observation-design.md](./pc-c4-03-param-observation-design.md) |
