# Architecture Decision Records

<!-- doc-suite:generated-index -->

Directory: `docs/designs`

| ID | Title | Status | File |
| --- | --- | --- | --- |
| DESIGN-AjvrOoTZDaCcUxKLXeoxA3 | AI-Native Hybrid DAW Baseline — Grant/Principal/Scope/Approval/ChangeSet・AI Off 既定・3 Mode 責務設計 (synvelis#44) | approved | [ai-native-baseline-design.md](./ai-native-baseline-design.md) |
| DESIGN-pzR2GiO8Eru6Jw2zRK6l2s | ALIB-01 Design — Asset Provider 契約・Local/Project 基準 Provider | approved | [alib-01-asset-provider-contract-design.md](./alib-01-asset-provider-contract-design.md) |
| DESIGN-034XCHS23zpAY0YcXEkGAP | ALIB-02 fault-injection followup — 残存 integrity/dirsync 拒否経路の専用 test 設計 | approved | [alib-02-fault-injection-followup.md](./alib-02-fault-injection-followup.md) |
| DESIGN-034VMQIe8i9hOn2Tb317Ue | ALIB-02 residual closeout — 証拠照合と gate 分類の設計 | approved | [alib-02-residual-closeout-design.md](./alib-02-residual-closeout-design.md) |
| DSN-alib02-streaming-import | ALIB-02 — Streaming Verified Import 設計（synvelis-library） | approved | [alib-02-streaming-verified-import-design.md](./alib-02-streaming-verified-import-design.md) |
| DSN-alib02-v5snap | ALIB-02 — v5 snapshot asset-digest 互換移行 設計 | approved | [alib-02-v5-snapshot-compat-design.md](./alib-02-v5-snapshot-compat-design.md) |
| DSN-alib03-catalog-preview-cache | ALIB-03 — Catalog/Preview Cache 設計（synvelis-library） | approved | [alib-03-catalog-preview-cache-design.md](./alib-03-catalog-preview-cache-design.md) |
| DSN-alib04-git-remote-provider | ALIB-04 — Git Remote Provider 設計（synvelis-library） | approved | [alib-04-git-remote-provider-design.md](./alib-04-git-remote-provider-design.md) |
| DESIGN-alib04-lfs-remote-fetch | ALIB-04 LFS Remote Fetch — 設計（endpoint 解決・batch API・型付きエラー・credential 境界） | approved | [alib-04-lfs-remote-fetch-design.md](./alib-04-lfs-remote-fetch-design.md) |
| DSN-alib06-webdav-provider | ALIB-06 — WebDAV File-Server Provider 設計（synvelis-library） | approved | [alib-06-webdav-provider-design.md](./alib-06-webdav-provider-design.md) |
| DSN-alib07-fault-matrix | ALIB-07 — Asset Library 全経路 Fault Matrix 設計（synvelis-library headless slice） | approved | [alib-07-fault-matrix-design.md](./alib-07-fault-matrix-design.md) |
| DSN-alib07-import-cancel | ALIB-07 slice 2 設計 — LIB-IMPORT-04 cancel checkpoint・subprocess crash fixture | approved | [alib-07-import-cancel-design.md](./alib-07-import-cancel-design.md) |
| DESIGN-034X9CpHP2HhkIZ6GzyZI6 | Remote-aware Asset Library Browser/Import UX 設計 (synvelis#9) | approved | [asset-library-design.md](./asset-library-design.md) |
| DESIGN-034AUT01Kp3mWv9zR2sQfL6h | AUT-01 Touch Return・Writer State 境界設計 — session clock + 明示閉鎖 | approved | [aut-01-touch-return-writer-state-design.md](./aut-01-touch-return-writer-state-design.md) |
| DESIGN-034AUT02Qm8wXs5vLn3jKp9r | AUT-02 Region Replace 境界曲線保存 設計 — merged series 境界 anchor・host 選択・stitch | approved | [aut-02-region-replace-boundary-design.md](./aut-02-region-replace-boundary-design.md) |
| DESIGN-034AUT03B1wG9hW1zS0Yf4 | AUT-03 Loop 録音 1 Pass — Write Range 半開区間閉鎖・境界 flush・明示 Rearm 設計 | approved | [aut-03-loop-write-one-pass-design.md](./aut-03-loop-write-one-pass-design.md) |
| DESIGN-034AUT04CvbbR3Qzx1aKwY | AUT-04 Automation Curve 非破壊保持・Viewport LOD・有界記録 Event 予算 設計 | approved | [aut-04-automation-curve-lod-record-budget-design.md](./aut-04-automation-curve-lod-record-budget-design.md) |
| DESIGN-2PaoXTMFb4NbwT7TNBcUTc | Automation / Modulation 統合 Parameter Control — 設計（synvelis#6） | approved | [automation-modulation-design.md](./automation-modulation-design.md) |
| DESIGN-034XzqUYtBrYIsgDfTvyEz | AUTX-01-F01 has_executor accessor 設計（No-Handler 区別の解禁） | approved | [autx-01-f01-has-executor.md](./autx-01-f01-has-executor.md) |
| DESIGN-034YqseBG2a0IpRjmMQrwF | AUTX-08 Leg C live-adapter leg — 設計 | approved | [autx-08-live-adapter-leg.md](./autx-08-live-adapter-leg.md) |
| DESIGN-034VmgS1sGu9EW9NfKKRPY | Issue #245 verify-macos rt_cache alloc 計測 flake — 計測対象インスタンス warm-up 設計 | approved | [ci-245-macos-alloc-flake-design.md](./ci-245-macos-alloc-flake-design.md) |
| DESIGN-TiKdIX8rj4YqmP3Pmb58JR | Issue #377 synvelis-native / gpui-test 3OS CI OS-L1 — 既存 verify leg 拡張設計 | approved | [ci-os-l1-native-gpui.md](./ci-os-l1-native-gpui.md) |
| DESIGN-034RoWWK4Xx4NaZSoBYZSX | Synvelis 独立 CI — Lane 構成・証跡・依存アクセス設計 | approved | [ci-synvelis-pr-push-regression-gate.md](./ci-synvelis-pr-push-regression-gate.md) |
| DESIGN-034X8sAionwOcsXpKgR0ND | Capability / Command Gateway — GW-01〜10 型付き契約・State Transition・Failure Matrix 設計契約 (synvelis#45) | approved | [command-gateway-design.md](./command-gateway-design.md) |
| — | Synvelis — Configurable PPQN / Resolution-independent Musical Time | — | [configurable-ppqn-musical-time-architecture.md](./configurable-ppqn-musical-time-architecture.md) |
| DESIGN-6ClrSXGAyry8iwAqt4t32c | Context Inspector 設計 — Follow/Explore/Pinned 状態機械・TargetKind Provider・情報階層・Probe/編集契約（synvelis#5） | approved | [context-inspector-design.md](./context-inspector-design.md) |
| DESIGN-034XecqdTFl5brsp93RKYN | Creative Branch 統合設計 — VariantSet/Scope/Lineage・Compare/Adopt・Owner/DAG (synvelis#277 CB-00) | draft | [creative-branch-design.md](./creative-branch-design.md) |
| DESIGN-034CTL01Qy1wZt7aC3dE4fGh | CTL-01 ControlOwner/AutomationTimeScope 設計 — Schema v7 Migration | approved | [ctl-01-control-owner-time-scope-design.md](./ctl-01-control-owner-time-scope-design.md) |
| DESIGN-taIsR3SbHTaGKnhYkfth7f | CTL-02 adoption SendLevel canonical spec 強制 設計 | approved | [ctl-02-adopt-sendlevel-design.md](./ctl-02-adopt-sendlevel-design.md) |
| DESIGN-034X9D6y9KpvdrfOuRveV6 | 標準 DAW 制作ワークフロー UX 正本 — Gate A 代表 Task・Keyboard/Gesture/Tool 操作体系 (Issue #8) | approved | [daw-workflow-design.md](./daw-workflow-design.md) |
| DESIGN-034QAlPiE7atUoGKXNG7Lr | ED-A Editing Model 設計 — Selection/Tool/Snap/Gesture/LinkedContext headless PoC | approved | [ed-a-editing-model-design.md](./ed-a-editing-model-design.md) |
| DESIGN-034QN0IIBPRdrHiOv3uTvq | ED-B media-domain editing design | approved | [ed-b-media-domain-editing-design.md](./ed-b-media-domain-editing-design.md) |
| DESIGN-034X83GOEQ84LyUmDRBepu | Execution Plan Design — 横断 DAG・Owner/Gate/Acceptance Matrix・OS 検証レベル・Evidence 台帳構造 | approved | [execution-plan-design.md](./execution-plan-design.md) |
| DESIGN-0Gw01GateWayContract01 | GW-01 Gateway Contract — Capability Registry・Typed Invocation・Versioned Result/Error | draft | [gw-01-gateway-contract-design.md](./gw-01-gateway-contract-design.md) |
| DESIGN-0Gw02AuthorizationPo01 | GW-02 Principal / Execution Grant / Authorization Policy — Fixed/Dynamic Scope | draft | [gw-02-authorization-policy-design.md](./gw-02-authorization-policy-design.md) |
| DESIGN-2mTMmzcADhXvcwIO5uSsUn | Issue #115 rf#628 consumer oracle Design | approved | [issue-115-rf628-consumer-oracle.md](./issue-115-rf628-consumer-oracle.md) |
| DESIGN-6b7yO4AkTMb19oy5Wz3k1L | Issue #115 rf#628 record-after-recovery Design | approved | [issue-115-rf628-record-after-recovery.md](./issue-115-rf628-record-after-recovery.md) |
| DESIGN-sv115VirtualEndpointRcd01 | Issue #115 RCD-01 Consumer Virtual-WASAPI EXT-L3 Harness 設計 | approved | [issue-115-virtual-endpoint.md](./issue-115-virtual-endpoint.md) |
| DESIGN-7Y20yWdQNliSve36HXn2Hy | Issue #145 ci.yml manual-only 設計 — trigger 撤去の最小差分と不変契約の境界 | approved | [issue-145-ci-manual-only.md](./issue-145-ci-manual-only.md) |
| DESIGN-034RIJlv2BuYKx5KG2VyDg | Issue #147 sccache キャッシュ分離・段階展開設計 | approved | [issue-147-sccache-compile-cache.md](./issue-147-sccache-compile-cache.md) |
| DESIGN-034SVCIOPT02kX3pLwQs | Issue #148 変更影響分類・Fail-Closed evidence Gate 設計 | approved | [issue-148-impact-classification.md](./issue-148-impact-classification.md) |
| DESIGN-034RGXfK1v0mkpD0U2xAaN | Issue #151 CI ui-verify lane 設計 | approved | [issue-151-ui-ci-lane.md](./issue-151-ui-ci-lane.md) |
| DESIGN-034RUkhVNYe0G4XOqwwqOa | Issue #157 rev.3 matrix lane skip placeholder — evidence gate 設計 | approved | [issue-157-matrix-placeholder-gate.md](./issue-157-matrix-placeholder-gate.md) |
| DESIGN-034SYN166CiSuMmRy | Issue #166 Full CI 化・ci-summary 統一 — 第一段階設計 | proposed | [issue-166-full-ci-ci-summary.md](./issue-166-full-ci-ci-summary.md) |
| DESIGN-RBfO0jdqOYWxb6xvtT5z0p | Issue #167 Tracker reconcile 設計 — Consumer 共有 checkout 適合・mode 分離・単一 pin 契約 | approved | [issue-167-tracker-reconcile-entry.md](./issue-167-tracker-reconcile-entry.md) |
| DESIGN-034YXXUP2FqWQZDrCaOwUz | SV177 design tokens — canonical vocabulary mapping, parity evidence, screen fixtures | approved | [issue-177-design-tokens.md](./issue-177-design-tokens.md) |
| DESIGN-034U2uVeVKqECLpCn8kbSa | Issue #184 CI verify/evidence 失敗 — canonical temp fixture 設計 | approved | [issue-184-ci-verify-evidence-canonical-temp-fixture.md](./issue-184-ci-verify-evidence-canonical-temp-fixture.md) |
| DES-034YBy2Ks8xNvTc4QwE | issue-262-run-file-real-vm — run --file 実実行の設計 | approved | [issue-262-run-file-real-vm.md](./issue-262-run-file-real-vm.md) |
| DESIGN-034YDh5yvBwPjPPKMUXsgY | issue-274-cli-script-analyze — script analyze --file の設計 | approved | [issue-274-cli-script-analyze.md](./issue-274-cli-script-analyze.md) |
| DESIGN-034ZjJTBCxSxqBr82J1mjO | Issue #283 slice — strip-fields gateway exercise design | approved | [issue-283-strip-gateway-exercise.md](./issue-283-strip-gateway-exercise.md) |
| DESIGN-286DiffExplorerAppWiring03 | Issue #286 design — CB-09 Difference Explorer app wiring | approved | [issue-286-diff-explorer-app-wiring.md](./issue-286-diff-explorer-app-wiring.md) |
| DESIGN-034WsvXEZuKq8V4PoYjVxZ | Issue #365 impl index 生成可能化 — front matter 是正と marker 追加の設計 | approved | [issue-365-impl-index-generation-design.md](./issue-365-impl-index-generation-design.md) |
| DESIGN-034WuZE8vEiOZJKbZMaRJQ | Issue #368 docs/impl/ir missing-section 本文補完 — 設計 | approved | [issue-368-ir-missing-sections-design.md](./issue-368-ir-missing-sections-design.md) |
| DESIGN-sv511cD4eF6gH8iJ0kL1mN3oP | Issue #511 設計 — CB-07 materialization 残件の方式決定 | approved | [issue-511-cb07-materialization.md](./issue-511-cb07-materialization.md) |
| DESIGN-513a1b2c3d4e5f6g7h8i9j0k3 | issue-513-variant-apply-ops — 設計 | approved | [issue-513-variant-apply-ops.md](./issue-513-variant-apply-ops.md) |
| DESIGN-0886aYIscVs3Pjh9Ep5J8a | Issue #514 design — selection 非消費の責務境界と将来の配線条件 | approved | [issue-514-session-engine-selection.md](./issue-514-session-engine-selection.md) |
| DESIGN-034YXStSteikAVmmfTjwB3 | issue-515-os-credential — durable session store + OS credential 経路の設計 | approved | [issue-515-os-credential.md](./issue-515-os-credential.md) |
| DESIGN-dHD8y1UOzR1s1jhUXZnanm | Issue #516 設計 — synvelis-script-luau を ruviform 製品 port への薄い binding へ置換 | approved | [issue-516-mlua-adapter.md](./issue-516-mlua-adapter.md) |
| DESIGN-sv517eF6gH8iJ0kL1mN3oP5 | Issue #517 設計 — render/branch pipeline 本番配線 | approved | [issue-517-render-branch-wiring.md](./issue-517-render-branch-wiring.md) |
| DESIGN-sv518cD4eF6gH8iJ0kL3mN5 | Issue #518 設計 — production job system 配線 | approved | [issue-518-job-system.md](./issue-518-job-system.md) |
| DESIGN-lc4Y6qkD6O151hJoRKMQ | Issue #530 design — stateless/capture-applicability の契約表現 | approved | [issue-530-plugin-state-decl.md](./issue-530-plugin-state-decl.md) |
| DESIGN-539aArrngmntAdptr03 | Issue #539 design — adapters map への ArrangementAdapter 登録と pin fixture positive flip | approved | [issue-539-arrangement-adapter.md](./issue-539-arrangement-adapter.md) |
| DESIGN-548StripDomLowr03 | Issue #548 design — strip-domain capability 登録と lowering | approved | [issue-548-strip-domain-lowering.md](./issue-548-strip-domain-lowering.md) |
| DESIGN-550StripRtOps03 | Issue #550 design — strip routing mask・confinement・wire 表面 | approved | [issue-550-strip-routing-ops.md](./issue-550-strip-routing-ops.md) |
| DESIGN-552SendGainSeal03 | Issue #552 design — SendGainTarget rebind の seal 解放（削除 vs unseal の確定） | approved | [issue-552-sendgain-seal.md](./issue-552-sendgain-seal.md) |
| DESIGN-553IntentFacts03 | Issue #553 design — declared_facts の inner facts 伝播 | approved | [issue-553-intent-facts.md](./issue-553-intent-facts.md) |
| DESIGN-557AdoptFacts03 | Issue #557 design — Adopt 経路 declared_facts 監査と受理 pin | approved | [issue-557-adopt-facts.md](./issue-557-adopt-facts.md) |
| DESIGN-559AdoptUnobservableRemoval03 | Issue #559 design — adopt 時の観測不能 entity 削除 fail-closed | approved | [issue-559-adopt-unobservable-removal.md](./issue-559-adopt-unobservable-removal.md) |
| DESIGN-561OpFactsUnverifiable03 | Issue #561 design — op_facts 3 arm への unverifiable 検査追加 | approved | [issue-561-opfacts-unverifiable-cascade.md](./issue-561-opfacts-unverifiable-cascade.md) |
| DESIGN-565ProcessorBindingCascade03 | Issue #565 design — 退役 cascade 監査の着地（契約明記 + test pin） | approved | [issue-565-processor-binding-cascade.md](./issue-565-processor-binding-cascade.md) |
| DESIGN-vaYFK7BnCVN715EASswzTe | Issue #86 DoD(2) — verified import consumer 配線 + digest-kind blob 解決 設計 | approved | [issue-86-verified-import-wiring-design.md](./issue-86-verified-import-wiring-design.md) |
| DESIGN-yiV0z7HH1jyKUmweFh9hCT | L1 Design — Safe Import・Verified Media・Lowering・Headless Driver | approved | [l1-local-wav-verified-playback-design.md](./l1-local-wav-verified-playback-design.md) |
| DESIGN-034QmfChvqgB632y8fAydk | L2 Design — Render Snapshot・Preflight・Sequential Render・WAV Output・Reopen Verify・Safe Publish | approved | [l2-master-sequential-offline-wav-export-design.md](./l2-master-sequential-offline-wav-export-design.md) |
| DESIGN-YFKePNnF4peFnskSfrnSc2 | MD-A Design — TakeGroup/Comp スキーマ・Effective Comp 投影・正規化 Placement/Playback 時間契約 | approved | [mda-comp-effective-projection-placement-design.md](./mda-comp-effective-projection-placement-design.md) |
| — | Synvelis × Ruviform — Media / Import / Export / Offline Render / UI / Diagnostics 統合アーキテクチャ | — | [media-import-export-offline-render-ui-diagnostics-architecture.md](./media-import-export-offline-render-ui-diagnostics-architecture.md) |
| DESIGN-034X7uvDLVml1hERo5iqNc | Media / Import / Export / Offline Render / UI / Diagnostics — Media & Render Lifecycle 設計契約 (synvelis#21) | approved | [media-render-design.md](./media-render-design.md) |
| DESIGN-034MSA52Mn8xRt6yQwZkPf | 最小 MIDI Source / Note 編集契約 設計 | draft | [midi-source-note-editing-design.md](./midi-source-note-editing-design.md) |
| DESIGN-v1cBPgJUA5gYtWIH40kczJ | Musical Time — 設計契約マップ（synvelis#19） | approved | [musical-time-design.md](./musical-time-design.md) |
| DESIGN-OVERVIEW | Synvelis 設計概要 | draft | [overview.md](./overview.md) |
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
| DESIGN-c4e8b1a2f5d30947e6c1a8b2d5 | PC-C4 observe engine wiring design — observe_block の engine-driver 配線 | approved | [pc-c4-observe-engine-wiring-design.md](./pc-c4-observe-engine-wiring-design.md) |
| DESIGN-4IoiIeHs7TpfdoOHf3nu9E | PC-C4 plugin binding design — persisted instance authority を bind 経路へ接続 | approved | [pc-c4-plugin-binding-design.md](./pc-c4-plugin-binding-design.md) |
| DESIGN-034Uw4SG2SiRKQuoKOZGzL | PC-C4 plugin parameter binding design — verified frame base・capability gate・generation consistency | approved | [pc-c4-plugin-param-binding-design.md](./pc-c4-plugin-param-binding-design.md) |
| DESIGN-034XHDrzEI83tAEfivcKq7 | PC-C4 webview composition design — synvelis.rs への routing engine attach | approved | [pc-c4-webview-composition.md](./pc-c4-webview-composition.md) |
| DESIGN-AkFUh4M60YuHo8COCvb0Um | PC-C5 slice1 Relation Inspector 設計 — RelationRow/RelationView/RelationNav + Inspector 第 2 Table | approved | [pc-c5-01-relation-inspector-design.md](./pc-c5-01-relation-inspector-design.md) |
| DESIGN-034UxXM5FBBTGn5JWkSBss | PC-C5 slice2 Gesture 固定・stale drop・Inspector mode 設計 | approved | [pc-c5-02-03-gesture-stale-inspector-design.md](./pc-c5-02-03-gesture-stale-inspector-design.md) |
| DESIGN-034WaOyEzrP97e8ooaGm2W | PC-C5 slice4 Recording status read model 設計 | approved | [pc-c5-04-recording-status-design.md](./pc-c5-04-recording-status-design.md) |
| DESIGN-mFknHHQIpULZsaBsqLEtPk | PC-C6 oracle 設計 — fixture 宣言モデル・独立参照実装・Case 対応表 | approved | [pc-c6-oracle-rel-tr-design.md](./pc-c6-oracle-rel-tr-design.md) |
| DESIGN-034VmVbkRvKggN17EiZnQ9 | PC-C6 oracle 設計 — Recording/Plugin/UI/RT fixture 宣言モデル・独立 oracle・Case 対応表 | approved | [pc-c6-oracle-rows-design.md](./pc-c6-oracle-rows-design.md) |
| DESIGN-034W1WOQzxAE3iMWUo0d49 | PC-C6 TR ledger oracle 設計 — 実経路 rig・独立 oracle・TR 行対応表 | approved | [pc-c6-tr-ledger-oracle-design.md](./pc-c6-tr-ledger-oracle-design.md) |
| DESIGN-034WxmyGWBzk3HppFdEDXE | PC-C6 TR02/TR06 realpath oracle 設計 — 既存 file への test 追加と行対応表 | approved | [pc-c6-tr02-tr06-realpath-oracle-design.md](./pc-c6-tr02-tr06-realpath-oracle-design.md) |
| DESIGN-034X6WwOONG7H4Rr2nHV2z | Playback / Mixer / Generic Execution Graph Design — typed port, PDC, plan activation, owner separation | approved | [playback-graph-design.md](./playback-graph-design.md) |
| — | Synvelis × Ruviform — Playback / Mixer / Generic Execution Graph Architecture | — | [playback-mixer-generic-execution-graph-architecture.md](./playback-mixer-generic-execution-graph-architecture.md) |
| DESIGN-034VG5a2ikhPyz9RCyjDDT | plugin_instances VCS semantic diff classification — keyed-synthetic PluginInstance | approved | [plugin-instances-vcs-diff-design.md](./plugin-instances-vcs-diff-design.md) |
| DESIGN-034X3jMjITd6SCoqrUyFnH | Synvelis 製品価値と中核体験 — Gate A/B/C・UX評価設計 | approved | [product-value-core-design.md](./product-value-core-design.md) |
| — | Synvelis — Project / Asset / Persistence Architecture | — | [project-asset-persistence-architecture.md](./project-asset-persistence-architecture.md) |
| DESIGN-034X6iC9O7gEAUDL5Sxqqv | Project Model Design — entity schema, single-writer/CAS/undo, time domains, persistence | approved | [project-model-design.md](./project-model-design.md) |
| DESIGN-CjYVivoek7ZbOPp7uvvYVo | Project / Asset / Persistence — 設計契約マップ（synvelis#17） | approved | [project-persistence-design.md](./project-persistence-design.md) |
| DESIGN-034V2hPBihovSNoj4xusOA | RCD-01 device-recording headless fixture — Mock Input→Raw Writer 統合設計 | approved | [rcd-01-device-recording-headless-fixture.md](./rcd-01-device-recording-headless-fixture.md) |
| DESIGN-Bz2IMMfywvmEjmdIeThNvq | RCD-01 Input Adapter — binding/Arm/Monitor/channel-map と fake port fail-stop | approved | [rcd-01-input-adapter-binding-arm-monitor-fake-port.md](./rcd-01-input-adapter-binding-arm-monitor-fake-port.md) |
| DESIGN-89lmf1mLK8flCuybXMmEqR | RCD-01 実Audio Device 到達準備 design — stamp placement fallback + real backend composition root | approved | [rcd-01-real-backend-readiness-design.md](./rcd-01-real-backend-readiness-design.md) |
| DESIGN-034W8Los2DTXD6ljySt4Rf | RCD-01 実機 capture coverage 拡張 design — real-device rig + staging lane | approved | [rcd-01-real-input-coverage-design.md](./rcd-01-real-input-coverage-design.md) |
| DESIGN-7xa4ndX9WpWkTxHtHv2HZs | RCD-01 Virtual L1 Oracle Design | approved | [rcd-01-virtual-l1-oracle.md](./rcd-01-virtual-l1-oracle.md) |
| DESIGN-4CsMcB1lWPYHOuIXmJdj4W | RCD-02 Blob SHA-256 Promotion Design | approved | [rcd-02-blob-sha256-design.md](./rcd-02-blob-sha256-design.md) |
| DESIGN-034Urj7kRY8rq2nngtTSuL | RCD-02 Durability Evidence / Recovery Review / Commit Receipt | approved | [rcd-02-durability-evidence-recovery-review.md](./rcd-02-durability-evidence-recovery-review.md) |
| DESIGN-034XHN8wd1qFbv3bkzg7bn | RCD-02 residual closeout — AC 照合と gate 分類・簿記整合の設計 | approved | [rcd-02-residual-closeout-design.md](./rcd-02-residual-closeout-design.md) |
| DESIGN-034VplpQjplSqKKaNy9r1G | RCD-02 StagedFile Publish Migration Design | approved | [rcd-02-stagedfile-publish-migration-design.md](./rcd-02-stagedfile-publish-migration-design.md) |
| DESIGN-VClnKX5mXMKj7CQFWgFYAE | RCD-02 Unsynced-Write Loss Fixture + Real-FS Probe Design | approved | [rcd-02-unsynced-loss-real-fs-probe-design.md](./rcd-02-unsynced-loss-real-fs-probe-design.md) |
| DESIGN-034VdWksbk4tfLCaf6HPoj | RCD-03 app Arm/Monitoring UX + monitor session projection | approved | [rcd-03-app-arm-monitoring-ux.md](./rcd-03-app-arm-monitoring-ux.md) |
| DESIGN-034WCrWfgXKQV2I7XmJGE0 | RCD-03 monitor engine — MonitorSession/MonitorEngine design | approved | [rcd-03-monitor-engine.md](./rcd-03-monitor-engine.md) |
| DESIGN-034VpQd8lN3xO0wS5uZ7vK | RCD-03 Arm/Monitor/Punch Project 永続投影 design | approved | [rcd-03-monitor-persistence-design.md](./rcd-03-monitor-persistence-design.md) |
| DESIGN-034X1y3Clx3u80e2MVL9IH | RCD-03 monitor/punch fake session fixture design | approved | [rcd-03-monitor-punch-design.md](./rcd-03-monitor-punch-design.md) |
| DESIGN-034XCSxsFES1gDT3HrKxUY | RCD-03 monitor session UX design — Loop/PunchInLoop + count-in/pre-roll 宣言 | approved | [rcd-03-monitor-session-ux.md](./rcd-03-monitor-session-ux.md) |
| DESIGN-034UtlI2uLUF1lr2g3CUM0 | RCD-03 Monitor Policy / Punch Gate Probe / Player Monitor-Latency Adapter | approved | [rcd-03-monitoring-punch-latency.md](./rcd-03-monitoring-punch-latency.md) |
| DESIGN-rprIAB9zoxNjZbw6rODTWf | RCD-03 VCS semantic diff monitor 宣言分類 design | approved | [rcd-03-vcs-diff-monitor-classification-design.md](./rcd-03-vcs-diff-monitor-classification-design.md) |
| DESIGN-RCD04Fx2nB8cY6dF5hJ9kM3q | RCD-04 Comp Transition / MIDI Phrase fixture — crate 内完結設計 | approved | [rcd-04-comp-transition-midi-phrase-fixture.md](./rcd-04-comp-transition-midi-phrase-fixture.md) |
| DESIGN-034WEV4lGiVlfIG0LsEBNr | RCD-04 MIDI phrase boundary / Undo-Redo / VCS stable-ID design | approved | [rcd-04-midi-undo-stableid-design.md](./rcd-04-midi-undo-stableid-design.md) |
| DESIGN-034VmNZvp2r11R0Phmdf0P | RCD-04 product E2E design | approved | [rcd-04-product-e2e-design.md](./rcd-04-product-e2e-design.md) |
| DESIGN-034YpTJ7LGHVn4vKqeBjhO | RCD-04 residual closeout — AC 照合と gate 分類・簿記整合の設計 | approved | [rcd-04-residual-closeout-design.md](./rcd-04-residual-closeout-design.md) |
| DESIGN-034VBU0PCNZXCZsyRneE3S | RCD-04 RT engine comp PCM design | approved | [rcd-04-rt-engine-comp-pcm-design.md](./rcd-04-rt-engine-comp-pcm-design.md) |
| DESIGN-034XTaBm7KJKf8COVxX4IFGk | RCD-06 Monitor production integration design | proposed | [rcd-06-monitor-production-integration.md](./rcd-06-monitor-production-integration.md) |
| DESIGN-sv659VerificationRedesign01 | Issue #659 Journal・Authorization 形式的検証再設計 — slice1 ledger/matrix | draft | [issue-659-verification-redesign.md](./issue-659-verification-redesign.md) |
| DESIGN-034XJyqSj5ERrc0PHD2tDm | RCD-F02 Advanced Monitoring / Overdub / Pre-Punch Handles design | approved | [rcd-f02-monitoring-overdub-design.md](./rcd-f02-monitoring-overdub-design.md) |
| DESIGN-HBZzINKHriX9qpGtNZ1ZFY | Render Checkpoint — Render Recipe・依存閉包・有効性・Branch/Execution/Policy 統合設計 (synvelis#289 RCP-00) | approved | [rcp-render-recipe-design.md](./rcp-render-recipe-design.md) |
| DESIGN-jrb8PxsJgSO7YaSuwiQUcL | Recording / Take / Comping — Session・Take・Comp・Raw Writer・Monitor・Recovery・Source Native Frame の基本設計（synvelis#12） | approved | [recording-take-comping-design.md](./recording-take-comping-design.md) |
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
| DESIGN-492syZg75ShHdZRAeG2DRE | SF-04 loop-discontinuity 設計 — 検出 accessor・per-block 宣言結線・OpName wire | approved | [sf-04-loop-discontinuity.md](./sf-04-loop-discontinuity.md) |
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
| DESIGN-uivmNbXmQvR2wTbLpNcJyH | SF-05 ui/ slice 設計 — Signal Flow・Probe headless view-model consumer path | approved | [sf-05-ui-viewmodel.md](./sf-05-ui-viewmodel.md) |
| DESIGN-034V2wuzXfqcjIvGyXFhhz | SF-06 cross-gate headless 設計 — oracle 独立性・fixture topology・ledger schema | approved | [sf-06-cross-gate-oracle-domain-evidence.md](./sf-06-cross-gate-oracle-domain-evidence.md) |
| DESIGN-OIDrCGhRjzwr0IzN1tzVkg | SF-XR-01 Design — CTL-01/CTL-02 cross-gate oracle fixture | approved | [sfxr01-ctl-crossgate-oracle-fixture.md](./sfxr01-ctl-crossgate-oracle-fixture.md) |
| DESIGN-034X9CQwZrVbNmLpKsTuVw | Signal Flow 設計正本 — 階層・Typed Port/Connection・Semantic Routing・Probe 表示/編集・世代証拠 | approved | [signal-flow-design-design.md](./signal-flow-design-design.md) |
| DESIGN-034X5yWfKUcmf5eqz7NUJR | Issue #84 StagedFile Migration 実装済み設計 | approved | [stagedfile-migration-design.md](./stagedfile-migration-design.md) |
| DESIGN-jmv4N50pKcC7sDGSX5TVEq | Issue #245 webdav MockServer TransientIo flake — accept 済みソケット blocking 正規化設計 | approved | [sv-ci-webdav-transient-io-flake-design.md](./sv-ci-webdav-transient-io-flake-design.md) |
| DESIGN-lW54zR76NmEm8cc3NlMskX | SV-GUI-01 Design — Desktop App Bootstrap・Composition Root・Frontend Asset Pipeline | approved | [sv-gui-01-desktop-app-bootstrap-design.md](./sv-gui-01-desktop-app-bootstrap-design.md) |
| DESIGN-034RAAXZ1XMIuk2zr2dr4w | SV-GUI-02 Frontend View Model・Binding IPC・Reference Workspace View | approved | [sv-gui-02-frontend-view-model-binding-ipc-reference-workspace-view.md](./sv-gui-02-frontend-view-model-binding-ipc-reference-workspace-view.md) |
| DESIGN-5n53fkGhYaUtJlMHOu5P1p | SV-GUI-03 slice1 Transport/Inspector 設計 — MainScreenState push + 既存 syn.control.* 経路 | approved | [sv-gui-03-01-transport-inspector-slice1-design.md](./sv-gui-03-01-transport-inspector-slice1-design.md) |
| DESIGN-8o9zUv4L5ae0FBB0zOM293 | SV-GUI-03 slice2 Arrangement/Undo 設計 — workspace read model + syn.project.* op 露出 | approved | [sv-gui-03-02-arrangement-undo-slice2-design.md](./sv-gui-03-02-arrangement-undo-slice2-design.md) |
| DESIGN-FQvwpPOrC7St8RoKqyYNMK | SV-GUI-03 slice3 Mixer strip Toggle 設計 — role+owner 走査・pump reconcile・atomic Set | approved | [sv-gui-03-03-mixer-strip-toggle-slice3-design.md](./sv-gui-03-03-mixer-strip-toggle-slice3-design.md) |
| DESIGN-Mx7pQr2wNz4kLs8tVb6yHd | SV-GUI-03 slice4 設計 — row selection→syn.view.select→Inspector rebinding + syn.viz.* surface reconcile | approved | [sv-gui-03-04-selection-inspector-viz-design.md](./sv-gui-03-04-selection-inspector-viz-design.md) |
| DESIGN-Ht8wEr4xQz2nBs6vYp9dMc | SV-GUI-03 slice5 設計 — driver-neutral scenario 抽出 + GpuiGuiTest lane | approved | [sv-gui-03-05-gpui-scenario-design.md](./sv-gui-03-05-gpui-scenario-design.md) |
| DESIGN-BZaKwo7Z0KBbDJHyNDjzNK | SV-GUI-03 slice6 設計 — Keyboard/Gesture key-assignment contract | approved | [sv-gui-03-06-keyboard-gesture-design.md](./sv-gui-03-06-keyboard-gesture-design.md) |
| DESIGN-034UogNAUQrEmBkgZnhrAk | SV-GUI-04 slice1 precursor 設計 — HeadlessWindowHost + dock fixture | approved | [sv-gui-04-01-headless-window-handle-dock-design.md](./sv-gui-04-01-headless-window-handle-dock-design.md) |
| DESIGN-Ywl9hP3wdlHWjoMPB7Hvjn | SV-GUI-04 slice1 設計 — 2nd view spec + env-gated 2窓 mount + FakeDriver fixture | approved | [sv-gui-04-01-two-window-slice1-design.md](./sv-gui-04-01-two-window-slice1-design.md) |
| DESIGN-034UqwNDJrureYsAqmckuw | SV-GUI-04 slice2 設計 — close_guard op + 末尾 window veto | approved | [sv-gui-04-02-close-veto-design.md](./sv-gui-04-02-close-veto-design.md) |
| DESIGN-034UqNokCXcUXgKk1tyv0p | SV-GUI-04 slice2 dock transfer: workspace adoption と dock/UI 権威分界の詳細設計 | approved | [sv-gui-04-slice2-dock-transfer-workspace-adoption-design.md](./sv-gui-04-slice2-dock-transfer-workspace-adoption-design.md) |
| DESIGN-034YAcuqWR9Ny3LPvqdMWR | SV-SCRIPT-03 Luau backend — 設計 | approved | [sv-script-03-luau-backend.md](./sv-script-03-luau-backend.md) |
| DESIGN-034Yr9OvufCMQtVOo9MClW | Issue #114 synvelis-app recorder session owner + syn.recorder.* headless wire | approved | [sv114-gui-option-b-recorder-wire.md](./sv114-gui-option-b-recorder-wire.md) |
| DESIGN-BWzJVIfJHZS8suU7aFFAGf | SV175 Native App Shell 設計 — NativeShell routing・ViewBinding adapter・native feature lane | approved | [sv175-native-shell-design.md](./sv175-native-shell-design.md) |
| DESIGN-034T728YcMrGoBwDtbkIqS | SV177 Product Theme / Workspace Preset — synvelis-theme crate・DTCG overrides・slot-based preset・semantic screen spec | approved | [sv177-product-theme-workspace-preset-design.md](./sv177-product-theme-workspace-preset-design.md) |
| DESIGN-Xk9mQ2vR7wTzLp4nHs8dFj | SV185-01 mixer.gain projection 設計 — ControlModel bootstrap・wire target・display_value | approved | [sv185-mixer-gain-projection-design.md](./sv185-mixer-gain-projection-design.md) |
| DESIGN-gAU8JKJSNsO9ZsGEgF0CDj | SV#28 Ruviform pin 更新 設計 — lockfile pin 更新と API 追従の境界（fd1d4d5f → ac10a2f7） | approved | [sv28-ruviform-pin-ac10a2f7.md](./sv28-ruviform-pin-ac10a2f7.md) |
| DESIGN-b6Vd3Nx9Qm2Lp8Rt5Yw7Ks | SV#28 Ruviform pin 更新 設計 — lockfile pin 更新と API 追従の境界（20dc41f7 → 87a7ba8f） | approved | [sv28-ruviform-pin-87a7ba8f.md](./sv28-ruviform-pin-87a7ba8f.md) |
| DESIGN-034VobNphX5isOqTl9Wg5i | SV#28 Ruviform pin 更新 設計 — lockfile pin 更新と API 追従の境界 | approved | [sv28-ruviform-pin-b02942c2.md](./sv28-ruviform-pin-b02942c2.md) |
| DESIGN-2z5KtdGhRdUvPQ0fJobGHe | SV#28 Ruviform pin 更新 設計 — lockfile pin 更新と API 追従の境界（abb9f372 → cbcc8343） | approved | [sv28-ruviform-pin-cbcc8343.md](./sv28-ruviform-pin-cbcc8343.md) |
| DESIGN-43KJVuVRlioN1JtUFVuqtW | SV#28 Ruviform pin 更新 設計 — lockfile pin 更新と API 追従の境界 | approved | [sv28-ruviform-pin-cc7d5296.md](./sv28-ruviform-pin-cc7d5296.md) |
| DESIGN-AyDQMCLnAv4MbV3s4qnT8D | SV#28 Ruviform pin 更新 設計 — lockfile pin 更新と API 追従の境界（9865cd57 → fd1d4d5f） | approved | [sv28-ruviform-pin-fd1d4d5f.md](./sv28-ruviform-pin-fd1d4d5f.md) |
| DESIGN-XIH1YjE7gCOlIHlrIPt8Rn | sv293 semantic→selection integration 設計 (synvelis#293 deferred residual slice) | approved | [sv293-semantic-selection-integration.md](./sv293-semantic-selection-integration.md) |
| DESIGN-034YnFdVxCjVKSDV4JZaWa | Issue #488 lint-app-native clippy::large_enum_variant 是正 — 設計 | approved | [sv488-lint-app-native.md](./sv488-lint-app-native.md) |
| DSGN-SvtE2e02aB2cD3eF4gH5iJ7 | SV-GUI-05: SVT-01〜09 Headless UI Scenario 設計 | approved | [svt-product-e2e-design.md](./svt-product-e2e-design.md) |
| DESIGN-034Uvylguz0kZUgy1zbseh | SYN-IO-01 Ruviform 共通 I/O 適合境界設計 | approved | [syn-io-01-ruviform-io-boundary-design.md](./syn-io-01-ruviform-io-boundary-design.md) |
| DESIGN-034T2aGDbNinkumWeftaiP | SYN-IO-02 DirectoryBundleStore StagedFile 移行設計 | approved | [syn-io-02-directory-bundle-store-stagedfile-design.md](./syn-io-02-directory-bundle-store-stagedfile-design.md) |
| DESIGN-034Tk3mXw7LpQ2vRnB8y | SYN-IO-02 HEAD CAS consumer-side mutex 設計 | approved | [syn-io-02-head-cas-design.md](./syn-io-02-head-cas-design.md) |
| DESIGN-034WVJwLjBMYR3fBrNJD2M | SYN-IO-02 HEAD CAS 実 2-process E2E 検証 — design | approved | [syn-io-02-head-cas-e2e-design.md](./syn-io-02-head-cas-e2e-design.md) |
| DESIGN-0948qgqpuG8j47FSJjhaHR | SYN-IO-02 Windows NTFS 実機 Case lane — design | approved | [syn-io-02-windows-real-fs.md](./syn-io-02-windows-real-fs.md) |
| DESIGN-034RBzdpo91wcbqBK7Gb9t | SYN-OBS-01 Design — Host所有 Logging Owner・設定解決・Source Catalog | approved | [syn-obs-01-design-host-logging-owner-source-catalog.md](./syn-obs-01-design-host-logging-owner-source-catalog.md) |
| DESIGN-034T2RF305WAVSTAGED00Y | SYN-RF305 WAV Export StagedFile 移行設計 | approved | [syn-rf305-wav-export-staged-file-design.md](./syn-rf305-wav-export-staged-file-design.md) |
| — | Synvelis-driven Ruviform Target Architecture v0.7.1 | — | [synvelis-driven-ruviform-target-architecture-v07.md](./synvelis-driven-ruviform-target-architecture-v07.md) |
| DESIGN-clIJhvdpK3FeleehsQ9lfb | Take Lane / Comp Editor — UI 詳細設計（synvelis#12 §14/§15 後続 slice — docs-only 設計 slice） | approved | [take-lane-comp-editor-design.md](./take-lane-comp-editor-design.md) |
| DESIGN-s3oIq2dqNieQuUHD6G89a7 | Target Architecture 設計レビュー — TA-001〜012 契約マップ・Owner 境界・DAG/型漏出/GUI 証拠評価 | approved | [target-architecture-design.md](./target-architecture-design.md) |
| DESIGN-034VtzPNCX6aVKTaxy71ke | TIM-01 app 層 PPQN 統合 — syn.project.set_ppqn 設計 | approved | [tim-01-app-ppqn-design.md](./tim-01-app-ppqn-design.md) |
| DESIGN-034UHVNvFQXFumTOowIt0g | TIM-01 Configurable PPQN — Rational Beat 再投影設計 | approved | [tim-01-configurable-ppqn-design.md](./tim-01-configurable-ppqn-design.md) |
| DESIGN-JxGLd6Q5TTckxCpvsulzYJ | TIM-01 PPQN config — punch marks 再投影と受入残件検証設計 | approved | [tim-01-ppqn-config-design.md](./tim-01-ppqn-config-design.md) |
| DESIGN-4kLmNpQrStUvWxYz0123 | TIM-01 PPQN Preview op + New Project PPQN — 設計 | approved | [tim-01-ppqn-preview-design.md](./tim-01-ppqn-preview-design.md) |
| DESIGN-TiM01uIxYzAbCdEfGh2 | TIM-01 PPQN 選択 UI + Snap/Grid ポリシー — 設計 | approved | [tim-01-ppqn-ui-design.md](./tim-01-ppqn-ui-design.md) |
| DESIGN-034XmM4rWsUgOxCaZeItMq | TIM-02 SMF app ops — op 配線 + MIDI clip compile 証拠設計 | approved | [tim-02-app-ops-smf.md](./tim-02-app-ops-smf.md) |
| DESIGN-623tim02midiNodeDelivery01 | TIM-02 MidiClipPlan→plugin node 配送 seam — single-plan merge + instrument node 解決 | draft | [tim-02-midi-node-delivery.md](./tim-02-midi-node-delivery.md) |
| DESIGN-126tim02playerMidiRt01 | TIM-02 player MIDI RT/Offline 結合 — crate 内完結設計 | approved | [tim-02-player-midi-rt-offline.md](./tim-02-player-midi-rt-offline.md) |
| DESIGN-TiM02PpqnSmfXg02 | TIM-02 × TIM-01 PPQN×SMF cross-golden fixture — 設計 | approved | [tim-02-ppqn-smf-cross-golden-design.md](./tim-02-ppqn-smf-cross-golden-design.md) |
| DESIGN-034UwVoRGI8Bp9zuzRFQhj | TIM-02 SMF Format0/1 core — crate 内完結設計 | approved | [tim-02-smf-format01-core.md](./tim-02-smf-format01-core.md) |
| DESIGN-034XI7yLnh4SLMxH8BU2lg | TIM-03 driver-side loop-wrap boundary derivation | approved | [tim-03-driver-loop-boundary.md](./tim-03-driver-loop-boundary.md) |
| DESIGN-034XzNPr4B2cwT30gBhGCN | TIM-03 Loop/Tempo boundary 自動導出 — 設計 | approved | [tim-03-loop-boundary.md](./tim-03-loop-boundary.md) |
| DESIGN-NhzhoY6wYQRrrjxtkUsnO | TIM-03 real ABI process oracle boundary slice | approved | [tim-03-real-abi-process-oracle.md](./tim-03-real-abi-process-oracle.md) |
| DESIGN-b6Kd2Wn8Qm4Xr7TyZuC5 | TIM-03 L1 out-of-process hosting production wiring | approved | [tim-03-l1-out-of-process-hosting.md](./tim-03-l1-out-of-process-hosting.md) |
| DESIGN-MsBu5XoYY4Ex3QB0qmDvC5 | TIM-03 session→driver/routing wiring design — session 寿命・provenance mint・transport mirror | approved | [tim-03-session-driver-wiring-design.md](./tim-03-session-driver-wiring-design.md) |
| DESIGN-034UC0ixTsN58rRlRctgAH | TIM-04 plugin state snapshot consistency design — quality receipt/store/session/offline recipe | approved | [tim-04-plugin-state-snapshot-consistency-design.md](./tim-04-plugin-state-snapshot-consistency-design.md) |
| DESIGN-034jU54Fg1eMsmUlQArX7T | TIM-04 Plugin State Snapshot — real ABI consumer-run design | approved | [tim-04-real-abi-state.md](./tim-04-real-abi-state.md) |
| DESIGN-sv128Tim04L1StateFault | TIM-04 L1 state/fault 設計 | draft | [tim-04-l1-state-fault.md](./tim-04-l1-state-fault.md) |
| DESIGN-034UyEjdALaapQRSk5hAdk | TIM-03 plugin process boundary design — capability/degraded policy oracle | approved | [tim03-process-boundary-design.md](./tim03-process-boundary-design.md) |
| — | Synvelis × Ruviform — Time / Transport / MIDI / Automation / Plugin Integration Architecture v0.5 | — | [time-transport-midi-automation-plugin-integration-architecture.md](./time-transport-midi-automation-plugin-integration-architecture.md) |
| DESIGN-T4E6PtTJ870Fpg4pFi3aM6 | Timeline Editing / Selection / Tool System — Snap・Gesture・Undo・Cross-View編集の共通設計（synvelis#11） | approved | [timeline-editing-design.md](./timeline-editing-design.md) |
| DESIGN-034X7wOrHbsoz1senpZA9w | Transport Integration Design — Engine Frame canonical, process-start-relative offsets, capability/degraded policy, TT-001〜030 mapping | approved | [transport-integration-design.md](./transport-integration-design.md) |
| DESIGN-102UiCfInt002BbCcDdEeF | UI-C5 製品 GUI 統合 設計 — syn.control.relation / syn.control.edge / syn.recording.* op surface | approved | [ui-sf-control-integration-design.md](./ui-sf-control-integration-design.md) |
| DESIGN-034XGA5y1ft0IHe1TEUxj5 | variant_sets VCS semantic diff 分類設計 — VariantSet/VariantBranch + Variants domain | approved | [variant-sets-vcs-diff.md](./variant-sets-vcs-diff.md) |
| DESIGN-034XI4wmwNPA3krWXzz5bA | variant_sets restore-as-copy origin identity 修復設計 — persisted provenance 規則の境界確定 (synvelis#458) | approved | [variant-sets-vcs-restore.md](./variant-sets-vcs-restore.md) |
| DESIGN-034UAwTwlzWbToxqWtFM2B | VCS-01 Immutable Snapshot Resolver — crate 内完結設計 | approved | [vcs-01-immutable-snapshot-resolver-design.md](./vcs-01-immutable-snapshot-resolver-design.md) |
| DESIGN-034UEtYMoQUWsPmOBrl819 | VCS-02 Semantic Diff Engine design | approved | [vcs-02-semantic-diff-engine-design.md](./vcs-02-semantic-diff-engine-design.md) |
| DESIGN-7jUOgXVjk3gpAYkbIzdUfe | VCS-03 Git Read Provider design | approved | [vcs-03-git-read-provider-design.md](./vcs-03-git-read-provider-design.md) |
| DESIGN-UpOaSmwX-5aEOp4rEEmXG | synvelis#335 unborn HEAD 誤分類修正 — rev-parse --quiet 除去と NotFound 経路回復設計 | approved | [vcs-03-unborn-head-classification-design.md](./vcs-03-unborn-head-classification-design.md) |
| DESIGN-VCS04BkndWiringDsn0001 | VCS-04 production backend wiring — module 設計 | approved | [vcs-04-backend-wiring-design.md](./vcs-04-backend-wiring-design.md) |
| DESIGN-VCS04Hist0ryV1ewDsn01 | VCS-04 History/Visual Diff UI design | approved | [vcs-04-history-visual-diff-ui-design.md](./vcs-04-history-visual-diff-ui-design.md) |
| DESIGN-VCS04NativeMount001 | VCS-04 History/Visual Diff — native mount/dock 配線 design | approved | [vcs-04-native-mount-design.md](./vcs-04-native-mount-design.md) |
| DSGN-vcs05-apply-restore-headless | VCS-05 Apply/Restore headless — module 設計 | approved | [vcs-05-apply-restore-headless-design.md](./vcs-05-apply-restore-headless-design.md) |
| DSGN-vcs05-monitoring-apply-surface | VCS-05 monitoring kinds apply surface 拡張 design | approved | [vcs-05-monitoring-apply-surface-design.md](./vcs-05-monitoring-apply-surface-design.md) |
| DSGN-vcs05-plugin-instance-apply | VCS-05 PluginInstance apply surface 拡張 design | approved | [vcs-05-plugin-instance-apply.md](./vcs-05-plugin-instance-apply.md) |
| DESIGN-TR6hK0f243aEk3CmFQlfp | VCS-06 evidence completion — RESTORE-08 / COMPAT-03/04 設計 | approved | [vcs-06-evidence-completion-design.md](./vcs-06-evidence-completion-design.md) |
| DESIGN-lzcGjnJjPNFKiNNUpE2gE3 | VCS-06 monitoring-domain matrix — fixture 設計 | approved | [vcs-06-monitoring-matrix-design.md](./vcs-06-monitoring-matrix-design.md) |
| DESIGN-034WTsUdd5SHHSF6EBJmfK | VCS-06 real-git/FS fault rows — design | approved | [vcs-06-real-git-fs-fault-design.md](./vcs-06-real-git-fs-fault-design.md) |
| DESIGN-034VpC0m5WnwXqJkI2nNXw | VCS-06 UI headless matrix — fixture 設計 | approved | [vcs-06-ui-headless-matrix-design.md](./vcs-06-ui-headless-matrix-design.md) |
| DESIGN-034XEZZdGRhhT1iSrpYPas | VCS-06 variant-domain matrix — fixture 設計 | approved | [vcs-06-variant-matrix-design.md](./vcs-06-variant-matrix-design.md) |
| DESIGN-034X9E4j59hipUI46lNAQC | VCS Integration — 統合契約・層構成・Case 被覆設計 (synvelis#7) | approved | [vcs-integration-design.md](./vcs-integration-design.md) |
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
| DESIGN-3kN7fR4hX1yZ7vD0oS8aLkQe | SV-GPUI-02 headless PluginDock panel 詳細設計 — 4-ID 所有・shared session registry・dock 2-phase 接続 | approved | [sv-gpui-02-plugin-dock-headless-panel-design.md](./sv-gpui-02-plugin-dock-headless-panel-design.md) |
| DSN-alib07-webdav-docker-provider | ALIB-07 — WebDAV 実 provider 検証環境のイメージ選定と構成設計（synvelis#633） | approved | [alib-07-webdav-docker-provider.md](./alib-07-webdav-docker-provider.md) |
| DESIGN-TiM01wIdGeTnPw02 | TIM-01 PPQN 選択 widget intent — params 付き Action → `syn.project.ppqn` 設計（synvelis#125） | approved | [tim-01-ppqn-widget.md](./tim-01-ppqn-widget.md) |
| DESIGN-JCeMt2CNWWebw7vrpDhnTJgD | SV-GPUI-02 PluginDock native slot 詳細設計 — shared surface authority・opaque lease redemption・canonical GPUI binding | approved | [sv-gpui-02-plugin-dock-native-slot-design.md](./sv-gpui-02-plugin-dock-native-slot-design.md) |
| DESIGN-pcC4L1RealAbi01 | PC-C4 L1 real ABI binding design | approved | [pc-c4-l1-real-abi-binding.md](./pc-c4-l1-real-abi-binding.md) |
| DESIGN-SV114SVOSCONSUMER0000001 | SV-OS consumer catalog — catalog/headless/real lane 設計 (synvelis#114) | approved | [sv114-svos-consumer-catalog.md](./sv114-svos-consumer-catalog.md) |
