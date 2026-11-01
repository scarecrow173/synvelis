# Synvelis — Project / Asset / Persistence Architecture

- **Document status:** Approved design baseline (2026-11-01); implementation and PoC pending
- **Decision tracker:** [Synvelis #17](https://github.com/scarecrow173/synvelis/issues/17)
- **Parent context:** Synvelis × Ruviform integrated architecture (v0.2, overall restructuring under review)
- **Related Synvelis:** [#2 ProjectSession](https://github.com/scarecrow173/synvelis/issues/2) · [#7 History / VCS](https://github.com/scarecrow173/synvelis/issues/7) · [#9 Asset Library](https://github.com/scarecrow173/synvelis/issues/9) · [#10 Project Model](https://github.com/scarecrow173/synvelis/issues/10) · [#12 Recording](https://github.com/scarecrow173/synvelis/issues/12) · [#13 VS-A](https://github.com/scarecrow173/synvelis/issues/13) · [#15 VS-C](https://github.com/scarecrow173/synvelis/issues/15) · [#16 VS-D](https://github.com/scarecrow173/synvelis/issues/16)
- **Ruviform reference:** [Architecture Baseline](https://github.com/scarecrow173/ruviform/blob/main/docs/designs/ruviform-architecture-design-baseline.md) · [Media I/O](https://github.com/scarecrow173/ruviform/blob/main/crates/ruviform-media-io/src/lib.rs)

> Scope: この文書は合意済みの **設計原則・データ所有権・整合性契約** を記録する。Rust確定API、Serializer/Hash方式、各OSのFilesystem保証、GCの実装、実測値はPoCで決める。実装や試験が完了したことを意味しない。

## 1. 決定の要約（10項目）

1. **Single Writer:** ProjectControllerがAuthoring正本を管理し、他のUI/Engine/Jobは検証付きProject Commandによって変更する。
2. **Revision separation:** Live / Saved / Recovery / VCS / Active Execution Planを区別。Undo/Redoも新ProjectRevisionへ進める。
3. **Directory Project Bundle:** 初期の物理構造としてディレクトリ型を採用する方針。論理Schemaと物理保存手段は分離する。
4. **Immutable Snapshot + Root Publish:** Immutable Snapshotを検証・耐久化後にHEAD/Rootを公開し、旧Saved Snapshotを上書き破壊しない。
5. **Identity separation:** AssetId/SourceId/ClipId/TakeId、Content Digest、Provider Revision、Provenance、Availability、Physical Locationを分離する。
6. **Remote Import Copy:** Metadata-first / fixed revision / non-RT staging / verify / Project Import。正式ImportはProjectへのCopyを既定にし、Preview Cacheは正本ではない。
7. **Media materialization ≠ Project commit:** Recording / Import / Derived Edit / Bounce / Exportを共通の状態遷移へ整理するが、固有のLoss/Recovery Policyは分離する。
8. **Autosave/Recoveryは別履歴:** Saved HEADを暗黙置換しない。Recording Media Recoveryと未保存Project Recoveryを混同しない。
9. **Safe retention:** Live/Saved/Recovery/VCS/Undo/Redo/Active Jobの参照を保護。初期自動破壊的GCは実施しない。
10. **Repo boundary:** SynvelisがDAW Project/Asset/Undo/Journal/GC Policyを所有。Ruviformには必要な汎用Media/Clock/Plugin能力のみ、既存crate配置に拘束されず設計する。

## 2. Authoritative StateとRevision

| State | Authority | Semantics |
| --- | --- | --- |
| Live ProjectRevision R | ProjectController | Command適用で単調増加。Undo/Redoで巻き戻さない |
| SavedRevision S | ProjectStore / HEAD | 正式Saveが完了したSnapshot。Liveより古くなり得る |
| RecoveryRevision A | RecoveryStore | Autosave/Crash Recovery専用Snapshot。Savedと別Root |
| VCS Revision V | External Provider | Provider+Repository+固定Revisionで識別。Branch名だけでは固定しない |
| ActivePlanRevision E | Runtime integration | 実行中Graph/Planの由来。Project保存成功と独立 |
| Materialized Media | Media Store | 実Bytesと検証/耐久化状態。Project参照登録とは独立 |

ProjectController以外がAuthoring正本を直接Mutationしてはならない。Workspace、Browser、Playback、Inspector、Recording Writer、Render Jobは読み取りProjectionか検証付きCommandを介する。

**Monotonic revision invariant:** Saved R120を非RT保存中、LiveがR123へ進むことは許す。Save成功はSaved=R120を意味し、Live Dirtyは継続する。非同期Saveが逆順に完了した場合、旧Jobが新Saved HEADを覆さないようRoot GenerationとProject Revisionによる排他/比較検証を行う。

## 3. Canonical Project Schemaの論理レイヤー

Project Snapshotは以下の**意味的構造**を所有する。これは特定SerializerやRust structを確定しない。

- Project identity / schema compatibility / revision / authoring settings
- TempoMap / musical timeline / transport defaults / markers
- Track Registry / Lane Registry / Clip Registry / Routing / Automation / Modulation
- Source Registry（Raw/Derived ContentVersion、Native Format、Lineage、Time mapping）
- Asset Manifest（AssetId、ContentRef、Provenance、Import Mode、Availability、Integrity）
- Plugin Registry（Stable Instance ID、Plugin Identity、Opaque State ContentRef、Missing capability）
- Recording Session / Pass / Take / Compの確定済みメタデータ
- Applied Commit Identity registry（冪等Retryの照合用）

### 識別子とデータの意味

| Item | Ownership / invariant |
| --- | --- |
| Clip | Timeline上の配置、Trim/Fade/Gain、Playback Source / Comp参照 |
| Source | Audio/MIDIの内容VersionとNative Time/Region。Rawは非破壊 |
| Asset | Materialized Contentの論理登録、取得元、権限・出典・解決状態 |
| Blob | 完全なBytesから計算・検証された不変Content |
| Provenance | RecordingSession、ProviderのPinned Revision、Derived Operation、Render Snapshotなど |
| Physical location | Project-local Blob、外部参照、Staging、Cache、Missing。IDではない |

**同じContent DigestでもAssetId/SourceIdの同一性は成立しない。** 実Bytesを重複排除しても、来歴/アクセス元/Revisionが異なるAssetは独立に管理する。Provider ETagやGit LFS Pointerを暗号学的Content Digestと同義にしない。

Canonical Serializerは意味のないMap列順を安定化し、意味のあるFX ChainやTrack順を保持する。OSごとのキャッシュ/Runtime Handle/volatile I/O情報をCanonical Snapshot Hashへ混入させない。Opaque Plugin Stateは不透明Bytesとして保持し、読めない/比較できない差分を「変更なし」としない。

## 4. Asset State MachineとSource Resolution

Assetの概念状態は Discovered → Staging → Verified → Registered。中断したStagingはRecoverable/Invalid、実体が失われたRegisteredはMissingとしてProjectの論理参照を保護する。

- VerifiedはByte/Format/Integrityを確認した状態であって、電源断後のDurabilityやProject登録済みを意味しない。
- RegisteredはProjectControllerのCommandが成功した状態。録音、Import、RenderのMaterialization済みBlobとProject Referenceの全体Atomicityは仮定しない。
- External Referenceは明示的な高度モードとして検討。パス、権限、外部変更、OS/Mount依存を可視化し、元Bytesを再検証する。
- Project ImportはCopy既定。Remoteから固定Revisionの内容を選択取得して検証後に登録。取得後のネットワーク切断/Preview Cache evictionでも既存Project Audioを失わない。
- Providerの更新はUpdate Available。既存のPinned Contentを勝手に上書きしない。Asset MissingでもSource/Clipの編集状態とProvenanceを保持する。
- Peak/Waveform/Thumbnail/Proxyなど再生成可能なCacheはSource正本/正式Blobと独立。

## 5. Directory Project Bundleの候補

    MySong.synvelis/
      HEAD                         # saved immutable snapshot pointer
      snapshots/
        <snapshot-id>.snapshot     # canonical project
      objects/
        ab/<content-digest>        # verified immutable blob
      staging/
        <job-id>/                  # recording / import / render
      recovery/
        HEAD                      # separate autosave root (optional location)
      metadata/
        ...                       # optional local project metadata
      cache/
        ...                       # disposable / VCS-ignore candidate

これは**Logical/Physical境界の概念**である。拡張子、Hash Algorithm、Snapshot Index、Recovery/Cacheの実配置、Compression、Archive Save As等はPoCで決める。

同一Filesystem内の小さなHEAD参照を最後に公開する構成を優先。Stagingが別ボリュームにある場合やNetwork FS上ではatomic renameを当然視しない。Backup / Save Asでは依存Blobを検証・コピーし、不完全なBundleを正式出力としない。

## 6. 正式Saveプロトコル（non-RT）

1. ProjectControllerから対象Immutable Snapshot RとExpected Saved Generationを固定する。
2. Stable ID/Schema/参照の整合性と、**新規正式登録される**必須Media BlobのFinalizationを検査する。既存のMissing MediaはDegraded Snapshotとして保存可能にし、過去素材を失ったからといって未保存編集を保存不能にしない。
3. 必要なSnapshot/Blobを一時ファイルへ出力し、必要なFile Syncを行い、Immutableな名前で公開する。
4. Snapshot全体の検証後、次のHEADを一時作成してSyncする。
5. 同一FilesystemにおけるCapabilityとGenerationを検証し、HEADをAtomic ReplaceできるBackendでは置換・Directory Syncする。
6. 成功したRevisionをSavedRevisionとして公開。途中の失敗は旧Saved HEADを保持し、孤児Objectを回復/後日Cleanup候補にする。

**保証境界:** write成功 / sync成功 / HEAD切替 / directory durability / ハードウェア電源断耐性はそれぞれ異なる。Windows/macOS/Linux/NAS/Cloud sync等の実能力をPoCで検証する。UnsupportedなStorageでは保証を下げる・Read-only・Save Copy・Reject等のPolicyを定義する。

**ACK lost:** HEAD置換直後のアプリ終了・応答喪失は、再Open時のRoot/CommitIdentity/Content検証で結果を判定する。未確認なのに再書き込み成功と断定しない。失敗時も旧Snapshotを消さない。

## 7. 操作別の保存・変更境界

| Operation | Main Saved HEAD | Expected behavior |
| --- | --- | --- |
| Save | 変更する | Immutable Snapshot Rの検証済みRoot Publish |
| Autosave | 変更しない | Recovery専用Rootへ新Checkpoint。最新編集すべての保全を保証しない |
| Save As | 元を変更しない | 新Bundleを別に構築・検証して公開。Project ID Fork/Move方式はPoC |
| Backup | 変更しない | 元Bundleに依存せず復旧可能な対象データ集合を保存 |
| VCS Commit | 変更しない | VCS Stage内容を別に検証。Save/Autosaveと無条件連動しない |
| Undo / Redo | 変更しない | Liveに新Revisionとして適用。Mediaを即物理削除しない |
| Restore History | 明示操作のみ | 読み取り専用Snapshot閲覧とProject Command適用を分離 |

Autosaveは一定間隔でもCrash直前の編集をすべて守る保証をしない。起動時はSavedとRecoveryの差分・時刻・欠落範囲を提示し、勝手にSaved HEADを巻き戻さない。

Multi-windowは一つのProjectControllerを共有。複数プロセス/外部VCS操作/Cloud SyncがBundleを変更したときはProject Lock/Root Generation/External Modification検出で衝突を報告。Lockが古いときにOS Process生死だけで安全解除できると仮定しない。

## 8. Recording / Import / Derived / Render Lifecycle

共通の意味的シーケンスは **Acquire/Produce → Staging → Validate → Durable Materialization → Project Reference Commit → Job Finalize**。Project外ExportにはProject Commitが存在しない場合がある。

| Producer | Source and verification | Failure and commit policy |
| --- | --- | --- |
| Recording | Raw PCM、Session/Source Frame Map、Writer Journal、Verified Range | 欠落検出はFail-stop/Partial Recovery、CommitIdentityで冪等 |
| Remote Import | Pinned Revision、取得Byte/Format/Hash、Permissions | Fail/CancelでProject未変更、Retry可能 |
| MIDI Derived Edit | 新Event Version、RawとのLineage | Raw不変、Independent / Linkedは明示 |
| Freeze / Bounce | 入力Snapshot/FX State/Render SettingsからDerived PCM | Failure時は旧Project/Playbackを維持 |
| Export | Encode/Muxされた完成File | 一時出力→検証→公開。通常Project Assetは追加しない |

**Orphan保護:** Materialization後にProject Commitが失敗/Crashした場合、Verified Orphanを保持し、Job ManifestからReview/Retryできる状態にする。同一CommitIdentity+同一ContentはAlreadyApplied、同Identity+異ContentはIntegrity Conflict。Project Save前のLive CommitでCrashした場合、回復できるMediaと未保存Projectの状態は区別する。

## 9. Crash Consistency Matrix

| Crash location | Saved state | Recovery |
| --- | --- | --- |
| Recording/import staging中 | 旧HEAD | Verified部分のみ候補。未完成を正式登録しない |
| Blob検証後/Project登録前 | 旧HEAD | Verified Orphanを保護・冪等Retry |
| Live Project Commit後/Main Save前 | 旧HEAD | Media保護。Recovery Snapshotの有無により未保存Projectの復元可能範囲が異なる |
| Snapshot生成中 | 旧HEAD | 旧Snapshotを保持、未公開ObjectはOrphan |
| HEAD置換直後/ACK喪失 | 新HEADの可能性 | 再OpenでRoot/Contentを検証 |
| RecoveryのほうがSavedより新しい | Saved旧/Recovery新 | 異なるSnapshotとして比較・明示復旧 |
| VCS操作中 | Saved独立 | Providerの実Working/Stage/Committed状態を照会 |
| Backup/Save As中断 | 元Project維持 | 新BundleをIncompleteとして扱う |

## 10. Schema Evolution / Migration

- Schema CompatibilityはMajor/Minor/Required Feature/Writer Version等を持つ概念。最終形式はPoC決定。
- 不明な必須Featureを無言で削除して保存しない。明示Reject/Read-only Degraded/Upgradeを選ぶ。
- Migrationは旧Immutable Snapshotを書き換えず新Snapshotを作る。ID/Provenance/Plugin Stateを検証し、失敗時には旧Rootを保持。
- Source/Clip等のStable IDを維持。再採番が不可避な場合はMapping/Lineageと影響を記録する。
- 過去VCS Revisionの参照時はRead-only Migration Projectionとし、元Revisionを変更しない。
- Plugin State MigrationはProject Schemaと別能力。Plugin欠落でもOpaque Bytesを失わない。

## 11. Retention / Garbage Collection

保護Rootは Saved HEAD、保持対象のHistorical Snapshot、Recovery Checkpoint、Active Live Project、Undo/Redo到達可能性、進行中Job、User Pin、把握可能なVCS参照とする。

GCはReachability-based Mark-and-Sweepのような方式を候補とするが、外部VCSや共有Storageのすべての参照を探索できるとは限らない。初期P0では自動破壊的GCを行わず、容量集計、候補検出、保護理由表示、明示Cleanupへ限定する。Hash一致を根拠に削除許可を推定しない。

Cacheはいつでも再生成/再取得できるDisposable領域。正式Import済みBlobをCache Evictionで消してはならない。

## 12. Ruviform責務の境界と拡張方針

| Capability | Synvelis | Ruviform |
| --- | --- | --- |
| Project Schema/Save/Undo/Recovery Root | Own | DAW専用型を導入しない |
| Asset/Source/Clip/Take IDとRetention Policy | Own | Generic Hash/File helperは実需要がある場合だけ |
| Decode/Demux/Media Inspect | Import Policy | Generic Media I/O Port / Adapter |
| Encode/Mux/Media Sink | Export/Render Policy | Generic Media I/O Port / Concrete backend capabilities |
| Audio Frame/Native Format | Source/Clip意味論 | Generic Audio Core |
| Plugin State/Plugin Identity | Project内の参照・保存・Missing管理 | Plugin Hostの規格中立State Capture/Restore |
| Recording Timestamp/Clock | Session/Placement/Source | Generic Device/Clock APIs（#187/#188） |
| Non-RT Job | UI/Commit Policy | Generic Exec/Cancel/Progress |
| Generic Timeline Evaluation | Authoring正本 | Reusable pure evaluator; second DAW Project modelは作らない |

Ruviformの現行crate境界を所与にせず、各API/Adapterの実需要に応じ **reuse / extend / extract / replace / deprecate** を比較する。汎用性、既存Consumer（Video/CLI等）、DAG、Migrationを確認し、実装時に必要なIssue/ADRを登録。抽象Encoder/Muxer Portの存在と、具体Backendでの動作保証を混同しない。

## 13. Existing Recording Vertical Sliceとの接続

- [VS-A #13](https://github.com/scarecrow173/synvelis/issues/13)は最小Canonical Snapshot/Stable IDs/Revision/ProjectStore Portを用意し、将来のDirectory Bundle/Schema Migrationを阻害しない。初期PoCで全物理形式を実装する必要はない。
- [VS-B #14](https://github.com/scarecrow173/synvelis/issues/14)はSourceFrame / SessionFrame / ClockEpochの区別、RT Buffer Owner契約を維持。ProjectStoreのI/OをRTへ持ち込まない。
- [VS-C #15](https://github.com/scarecrow173/synvelis/issues/15)はRecording Staging/Writer/Verified Range/Journalを担当。Project Saveの原子性とWriter durabilityを混同しない。
- [VS-D #16](https://github.com/scarecrow173/synvelis/issues/16)はFinalized MediaとCommitIdentityの冪等登録、Undo/Redo、Save/Reload、Recovery Previewを検証。Projectの全未保存編集が復旧できると偽装しない。
- Playback/Render/Importも将来このProject Asset ResolverとAuthoring Snapshotを利用し、Recording専用Project Formatを作らない。

## 14. 設計受入ケース（PAP-01〜PAP-18）

| Case | Situation | Required observation |
| --- | --- | --- |
| PAP-01 | 1 Sourceを共有するClip移動/Trim | Blob不変、Clipの配置のみ変更 |
| PAP-02 | 同一Bytesを別ProviderからImport | AssetId/Provenanceは別、Blob共有可能 |
| PAP-03 | Remote固定Revision Copy→Offline | Project Blobで再生可能、Cache非依存 |
| PAP-04 | Save中Liveで編集継続 | Saved RとLive R+1を区別 |
| PAP-05 | 古いSaveが遅れて完了 | Saved HEADが巻き戻らない |
| PAP-06 | Blob staging / HEAD前Crash | 旧Saved Rootが有効 |
| PAP-07 | HEAD切替直後 ACK消失 | 再Openで実Save状態を判定 |
| PAP-08 | Recovery新、Saved旧 | 差分を示して明示Restore |
| PAP-09 | Recording Commit後Save前Crash | Verified Mediaを保護、Project Recoveryと分離 |
| PAP-10 | CommitIdentity Retry | 重複登録なし、異内容はConflict |
| PAP-11 | Undo/Redo/GC | 保護Mediaを自動削除しない |
| PAP-12 | 未知Schema/Plugin State | Read-only/Degraded/Reject、元Bytes保全 |
| PAP-13 | VCS History比較 | Live/Working Tree/Engineを変更しない |
| PAP-14 | 悪意あるPath/Archive/Symlink | Traversal/意図しない上書きを拒否 |
| PAP-15 | Atomic Replace/Sync非対応FS | Capability低下・Fallback/Rejectを明示 |
| PAP-16 | Project同時Open/External Modification | Conflict検出・安全なSave Copy等 |
| PAP-17 | Save As/Backup途中障害 | 元Project不変、新BundleをIncomplete扱い |
| PAP-18 | Blob Corruption/Hash mismatch | Integrity Degraded、元レコード/修復候補保持 |

## 15. 未確定事項・PoCへの委譲

- Physical Serializer（JSON/CBOR/SQLite/Hybrid）、Blob Hash Algorithm、物理配置、公開Project Format、Large MIDI/Automation Event分割。
- Windows/macOS/Linux/NAS/Cloud FSのAtomic Replace/Directory Sync、Project Lock/Recoveryの耐久保証。
- Save As時のProjectId Fork/Move、Autosave周期、Cross-restart Undo Log、Retention期間、GC実行UI。
- Plugin StateのSchema/Plugin Version不一致、未知Featureの保持とRead-only互換性。
- 多プロセス・外部変更・VCS/Cloud Sync、バックアップ・共有Media、暗号化や可搬性の詳細。
- Recording Journalと一般Project StoreのCheckpoint連携、実Device/RT/Writerの計測。

**設計の合意は実装・実機検証の完了ではない。** 次の横断テーマはPlayback / Mixer / Generic Execution Graphとし、Authoring Snapshot → Prepared Execution PlanのLoweringを詳細化する。
