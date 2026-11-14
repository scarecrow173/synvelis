# Media & Render Lifecycle — MR Case Ledger (synvelis#21)

Issue #21 の MR-RCP-01・MR-001〜028 の 29 受入ケースについて、
fixture・oracle・Owner・status・証拠を台帳化する。**本 slice は
docs/ 設計契約 + evidence のみ**であり、実装・GUI・実
device/plugin/OS 受入は対象外。

Fixture 表記は `path::test_fn`（Rust integration test の
関数名）とする。`l1_*`/`l2_*`/`x_*`/`sf03_render_*`/`sf04_*`/
`tim03_*`/`tim04_*`/`strict_*` は共有 test file 内の test
関数名であり、独立 file ではない。

## Status 語彙

| Status | 意味 |
|---|---|
| `Pass` | 実機/対象環境で検証が成功した（本 slice では未使用） |
| `Pass(部分)` | headless fixture で関連契約が確認できるが、当該 MR 全体の独立 oracle・実機/GUI/Plugin/OS 受入を満たさない |
| `Fail` | 検証が失敗した |
| `Unsupported` | 当該 capability が未対応として明示されている |
| `NotRun` | 未実装または未検証 |

headless Pass を実機/GUI/Plugin/OS 受入に繰り上げない。
設計レビュー Pass と実測 Pass を区別する。

## Fixture 凡例（実在 file）

- `crates/synvelis-library/tests/`: `local.rs`・`git.rs`・
  `webdav.rs`・`cache.rs`・`types.rs`・`import.rs`・
  `import_recovery.rs`・`project.rs`・`lfs.rs`・`contract.rs`・
  `containment_swap.rs`・`rt_cache.rs`・`vcs03_git_provider.rs`・
  `vcs06_git_matrix.rs`
- `crates/synvelis-player/tests/`: `l1_e2e.rs`・`verified_
  import.rs`・`md_b.rs`・`sf03_l1..l9.rs`・`sf04_probe_
  acceptance.rs`・`sf04_probe_adapter.rs`・`sf04_s2_probe_
  trace_bridge.rs`・`sf04_s3_probe_discontinuity.rs`・
  `sf04_s7_probe_compare.rs`・`sf04_s8_probe_faults.rs`・
  `sf04_s9_probe_discontinuity_declare.rs`・`tim02_*.rs`・
  `tim03_*.rs`・`tim04_plugin_state.rs`・`trace_c3_05*.rs`・
  `pc_c4_*.rs`・`pc_c6_oracle_*.rs`・`rcd_04_midi_phrase_
  boundary.rs`・`sf_xr_01.rs`・`monitor*.rs`・`ctl_02.rs`・
  `pc_b.rs`
- `crates/synvelis-render/tests/`: `l2_e2e.rs`・`sf03_render.rs`・
  `tim04_plugin_state.rs`・`x_e2e.rs`・`rcd_04_rt_comp_pcm.rs`
- `crates/synvelis-gateway/tests/`: `job.rs`・`gw10_job.rs`
- `crates/synvelis-workspace/tests/`: `ws_a.rs`・`workspace_
  model.rs`
- `crates/synvelis-project/tests/`: `head_cas.rs`・`store_
  publish.rs`・`rcd_04_*.rs`
- `crates/synvelis-recording/tests/`: `vs_c.rs`・`staged_
  publish.rs`・`recovery_review.rs`・`rcd_04_*.rs`
- `crates/synvelis-app/tests/`: `sv113_dock.rs`・`rcd_04_
  product_e2e.rs`・`logging.rs`・`service_ops.rs`

## Case Ledger

### MR-RCP-01 Render Checkpoint（NotRun）

| 項目 | 内容 |
|---|---|
| 要求 | Snapshot pin 付き Offline Render output を RCP-02 independent readback で検証し、lossy probe/partial artifact/unknown source や Plugin state を Exact Cache に流用しない |
| Owner | Synvelis #290〜#297（RCP-01〜F01）+ Ruviform #289（RCP-00） |
| Fixture/Oracle | （未実装）RenderSpec pin・independent readback は設計で固定。先行実体として `crates/synvelis-render/tests/l2_e2e.rs::l2_01_master_export_reopen_pcm_match`（独立 Reader reopen 検証）が存在するが RCP の Artifact Index/GC・Trigger Job ではない |
| Status | **NotRun** — RCP 系 Issue が未実装 |
| 証拠 | `docs/designs/media-render-design.md` §RCP Boundary、`docs/specs/media-render.md` MR-R-18 |
| 後続 | #290〜#297（RCP-01〜F01） |

### MR-001 Local Audio Import → Verified Blob（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Local Audio Import→Verified Blob→Clip Playback→実 Master PCM WAV Export→Reopen が入力 PCM・Sample Frame・Provenance・独立 Decode で検証される |
| Owner | #24（L1）・#17・#86 |
| Fixture | `crates/synvelis-player/tests/l1_e2e.rs::l1_01_import_verifies_and_materializes`・`l1_02_id_separation_shared_source_undo_save`・`l1_03_headless_clip_playback_exact`・`crates/synvelis-player/tests/verified_import.rs::sha256_import_resolves_and_plays`・`crates/synvelis-library/tests/import.rs::import_streams_verifies_publishes_and_commits`・`replay_same_import_is_idempotent` |
| Oracle | SHA-256 hash・container/codec/frame/channel verify・CAS materialize・independent readback |
| Status | **Pass(部分)** — headless・WAV PCM のみ・実 codec/OS NotRun |
| 証拠 | `docs/specs/l1-local-wav-verified-media-graph-playback.md`、`docs/specs/issue-86-verified-import-wiring.md` |

### MR-002 Native PCM 変換履歴（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Native PCM differing rate/layout → explicit transform history、bytes unchanged |
| Owner | #24 |
| Fixture | `crates/synvelis-player/tests/l1_e2e.rs::l1_05_time_conversion_is_exact_and_documented`・`l1_04_failure_classes_are_typed_and_atomic`（rate mismatch typed reject） |
| Oracle | 変換履歴（SRC/channel map/scale/clipping/dither）の記録・byte 不変・typed reject |
| Status | **Pass(部分)** — identity/provenance 保持・typed reject 確認・実 SRC/resample は Deferred（#20 系） |
| 証拠 | `docs/specs/l1-local-wav-verified-media-graph-playback.md` |

### MR-003 Unsupported codec/container（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Unsupported codec/container → capability error・誤判定なし |
| Owner | #24・#86 |
| Fixture | `crates/synvelis-player/tests/l1_e2e.rs::l1_04_failure_classes_are_typed_and_atomic`・`crates/synvelis-player/tests/verified_import.rs::sha256_missing_and_mismatch_fail_typed`・`opaque_digest_fails_closed` |
| Oracle | typed error（Unsupported/Missing/Mismatch）・atomic reject・fail-closed |
| Status | **Pass(部分)** — headless・実 codec matrix NotRun |
| 証拠 | `docs/specs/l1-local-wav-verified-media-graph-playback.md`、`docs/specs/issue-86-verified-import-wiring.md` |

### MR-004 Same content 2 origins → dedup + 別 ID（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Same content 2 origins → dedup + separate IDs/provenance |
| Owner | #24・#17 |
| Fixture | `crates/synvelis-library/tests/import.rs::same_bytes_across_providers_share_one_verified_blob`・`same_import_id_different_content_is_conflict`・`crates/synvelis-library/tests/project.rs::imported_via_provenance_is_projected`・`foreign_provider_ref_is_rejected` |
| Oracle | ContentRef dedup・論理 ID/provenance 分離・conflict typed |
| Status | **Pass(部分)** — headless・実 provider dedup NotRun |
| 証拠 | `docs/specs/project-persistence.md` |

### MR-005 Remote metadata-first（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Remote Asset を metadata-first で検索・選択取得、未選択 Asset の全量同期を回避 |
| Owner | #9/#85〜#91 |
| Fixture | `crates/synvelis-library/tests/local.rs::paging_walks_all_entries_once_without_dupes`・`filters_by_name_kind_and_extension`・`search_matches_name_substring`・`crates/synvelis-library/tests/git.rs::paging_walks_the_whole_tree_deterministically`・`partial_clone_fetches_only_selected_objects`・`browse_*`・`crates/synvelis-library/tests/webdav.rs::browse_cursor_pages_listing`・`crates/synvelis-library/tests/project.rs::snapshot_catalog_supports_paging` |
| Oracle | Metadata-first page/filter/sort/search・IndexCoverage/IndexedAt/ProviderRevision/ResultCompleteness・未検索≠不存在 |
| Status | **Pass(部分)** — headless mock・実 remote server NotRun |
| 証拠 | `docs/specs/alib-01-asset-provider-contract.md`・`docs/specs/alib-03-catalog-preview-cache.md`（#9 系・参照のみ）、`docs/designs/alib-01-asset-provider-contract-design.md`・`alib-02-streaming-verified-import-design.md` |

### MR-006 Provider offline/stale（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Provider offline/stale → Stale explicit、imported project plays |
| Owner | #9/#85〜#91 |
| Fixture | `crates/synvelis-library/tests/types.rs::observations_separate_observed_inferred_unknown_stale`・`crates/synvelis-library/tests/cache.rs`（cache hit/miss・principal isolation）・`crates/synvelis-library/tests/git.rs::refresh_makes_remote_updates_visible`・`crates/synvelis-library/tests/webdav.rs::offline_server_fails_typed_on_every_op`・`crates/synvelis-library/tests/local.rs::changed_content_breaks_the_pin_not_silently_serves` |
| Oracle | Observation Level（Observed/Inferred/Unknown/Stale/Unavailable）・Stale 明示・cached metadata 代替 |
| Status | **Pass(部分)** — headless・実 provider offline NotRun |
| 証拠 | `docs/designs/alib-01-asset-provider-contract-design.md` |

### MR-007 Preview unsupported/partial fetch（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Preview unsupported/partial fetch → capability/error vs cache 分離 |
| Owner | #9/#87（ALIB-03） |
| Fixture | `crates/synvelis-library/tests/cache.rs::preview_range_fetch_cached_and_principal_isolated`・preview fault injection・`crates/synvelis-library/tests/local.rs::preview_reads_a_range_unverified`・`byte_budget_bounds_pin_and_open`・`crates/synvelis-library/tests/git.rs::preview_returns_unverified_bounded_bytes`・`byte_budget_is_enforced_at_pin_and_preview`・`crates/synvelis-library/tests/webdav.rs::preview_range_honored_206`・`preview_range_ignored_falls_back_to_client_skip`・`preview_416_yields_empty_read` |
| Oracle | Preview capability 宣言・range fetch・cache 分離・principal isolation・fault typed・preview は unverified |
| Status | **Pass(部分)** — headless・実 preview byte budget/eviction/実 server NotRun |
| 証拠 | `docs/specs/alib-03-catalog-preview-cache.md`・`docs/designs/alib-03-catalog-preview-cache-design.md` |

### MR-008 Pinned revision changed upstream（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Pinned revision changed upstream → no silent replace |
| Owner | #9/#85〜#91 |
| Fixture | `crates/synvelis-library/tests/local.rs::changed_content_breaks_the_pin_not_silently_serves`・`renamed_or_deleted_file_cannot_open_pinned`・`crates/synvelis-library/tests/git.rs::force_push_keeps_pinned_objects_fetchable`・`pinned_commit_survives_later_remote_updates`・`deleted_paths_and_branches_fail_typed_after_refresh`・`remote_rename_keeps_pin_and_fixed_commit_identity`・`crates/synvelis-library/tests/webdav.rs::open_pinned_412_is_precondition_failed`・`open_pinned_sends_if_match_and_verifies`・`open_pinned_response_etag_mismatch_is_source_changed` |
| Oracle | strong pin・content/etag 変更で pin 破壊（silent serve なし）・typed fail・If-Match precondition |
| Status | **Pass(部分)** — headless・実 provider revision race NotRun |
| 証拠 | `docs/designs/alib-01-asset-provider-contract-design.md` |

### MR-009 Import cancel/hash mismatch（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Import cancel/hash mismatch → no half commit・recoverable staging |
| Owner | #24・#86 |
| Fixture | `crates/synvelis-library/tests/import.rs::cancel_mid_stream_is_typed_and_same_import_resumes`・`cancel_during_verify_is_typed_and_same_import_resumes`・`cancel_during_publish_is_typed_and_same_import_resumes`・`cancel_at_commit_boundary_parks_at_published_and_resumes`・`subprocess_crash_during_publish_or_commit_resumes_across_processes`・`crates/synvelis-library/tests/import_recovery.rs`（recovery/resume 系）・`crates/synvelis-player/tests/verified_import.rs::sha256_missing_and_mismatch_fail_typed` |
| Oracle | 段階別 Cancel/Retry・staging 保持・Verified Orphan=Recoverable・Cancel≠即時削除・half commit なし・process crash resume |
| Status | **Pass(部分)** — headless・強 fixture・実 disk/OS NotRun |
| 証拠 | `docs/specs/issue-86-verified-import-wiring.md`・`docs/designs/media-render-design.md` §ImportJob |

### MR-010 Import完了時 target changed（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Import完了時 target changed → expected revision/target re-verify |
| Owner | #24・#17 |
| Fixture | `crates/synvelis-library/tests/import.rs::stale_revision_rejected_then_explicit_rebase_converges`・`missing_lane_target_is_rejected_without_clip`・`wrong_track_lane_target_is_rejected`・`crates/synvelis-library/tests/import_recovery.rs::stale_then_target_gone_rejects_and_orphan_protected`・`multi_track_placement_lands_on_intended_lane_only`・`undo_after_commit_reimport_converges`・`crates/synvelis-player/tests/verified_import.rs::undo_then_reimport_converges_reserved_ids` |
| Oracle | ExpectedRevision Commit・target 再検証・orphan protected・Undo≠物理削除・reserved id 収束 |
| Status | **Pass(部分)** — headless・実 concurrent edit NotRun |
| 証拠 | `docs/specs/project-persistence.md` |

### MR-011 Prefetch underrun/seek（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Prefetch underrun/seek → no RT file I/O・loss diagnostics |
| Owner | #24 |
| Fixture | `crates/synvelis-player/tests/l1_e2e.rs`（underrun/EOF 系 test・`l1_03_clip_end_past_source_eof`・`l1_03_headless_clip_playback_exact`） |
| Oracle | nonRT Prefetch→RT Frame・RT file I/O なし・loss/EOF typed |
| Status | **Pass(部分)** — headless・実 audio device NotRun |
| 証拠 | `docs/specs/l1-local-wav-verified-media-graph-playback.md`・`docs/designs/playback-graph-design.md` |

### MR-012 Local mix→PCM WAV（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Local mix→PCM WAV → real backend valid file |
| Owner | #25 |
| Fixture | `crates/synvelis-render/tests/l2_e2e.rs::l2_01_master_export_reopen_pcm_match`・`crates/synvelis-render/tests/x_e2e.rs::x_01_recorded_playback_export_bit_exact`・Ruviform io-wav（rf#191 closed）・`crates/synvelis-project/tests/store_publish.rs`（staged publish） |
| Oracle | 実 PCM WAV 出力・独立 Reader（symphonia 系）で reopen・PCM/sample frame/provenance 検証・Writer 自己申告で合格にしない |
| Status | **Pass(部分)** — headless・実 device/OS NotRun |
| 証拠 | `docs/specs/l2-master-sequential-offline-wav-export.md` |

### MR-013 Output codec unsupported（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Output codec unsupported → preflight reject |
| Owner | #25 |
| Fixture | `crates/synvelis-render/tests/l2_e2e.rs::l2_04_unknown_tail_and_support_rejected`・`l2_07_capability_evidence_on_receipt`・`crates/synvelis-render/tests/tim04_plugin_state.rs::strict_export_fails_at_preflight_before_destination`・`crates/synvelis-library/tests/webdav.rs::capabilities_declare_contract` |
| Oracle | Format negotiation・unsupported format preflight reject・destination 未作成・capability evidence on receipt |
| Status | **Pass(部分)** — headless・WAV PCM のみ supported・他 format NotRun |
| 証拠 | `docs/specs/l2-master-sequential-offline-wav-export.md` |

### MR-014 Export range/loop/tempo（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Export range/loop/tempo → half-open no dup/missing |
| Owner | #25・#26 |
| Fixture | `crates/synvelis-render/tests/l2_e2e.rs::l2_02_clip_boundaries_gaps_muted`・`l2_02_range_edges_and_preroll_clamp`・`l2_02_unresolvable_anchor_refused`・`l2_02_all_muted_refused`・`crates/synvelis-render/tests/x_e2e.rs::x_02_punch_in_loop_per_segment_correspondence` |
| Oracle | 半開 `[start,end)`・Frame Event 二重評価なし・anchor 明示・loop 区別・per-segment 対応 |
| Status | **Pass(部分)** — headless・実 tempo map/plugin NotRun |
| 証拠 | `docs/specs/l2-master-sequential-offline-wav-export.md`・`docs/specs/x-recorded-verified-source-playback-export.md` |

### MR-015 Preroll/Tail/PDC（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Preroll/Tail/PDC → head/tail/stem alignment |
| Owner | #25 |
| Fixture | `crates/synvelis-render/tests/l2_e2e.rs::l2_04_pdc_realigns_not_inflates`・`l2_04_preroll_is_state_semantics`・`l2_04_known_tail_captures_delay`・`l2_04_unknown_tail_and_support_rejected` |
| Oracle | Preroll/Warm-up・PDC realign（inflate ではない）・tail policy（Fixed/UntilSilence/User/Unsupported/Unknown）・master alignment |
| Status | **Pass(部分)** — master のみ・stem alignment 未実装（MR-017 と連動） |
| 証拠 | `docs/specs/l2-master-sequential-offline-wav-export.md` |

### MR-016 Stateful offline（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Stateful offline → no unconditional chunk parallel・state continuity |
| Owner | #25・Ruviform #198 |
| Fixture | `crates/synvelis-render/tests/l2_e2e.rs::l2_03_stateful_inserts_block_invariant`・`l2_03_delay_zero_is_identity`・`crates/synvelis-render/tests/sf03_render.rs::sf03_render_02_fail_closed_same_fault`・Ruviform rf#198 sequential offline driver |
| Oracle | 時間方向 Sequential・stateful DSP へ任意時間分割並列を無条件適用しない・state continuity・fail-closed |
| Status | **Pass(部分)** — headless・実 stateful plugin NotRun |
| 証拠 | `docs/specs/l2-master-sequential-offline-wav-export.md` |

### MR-017 Stem tap/master fx/sidechain（NotRun）

| 項目 | 内容 |
|---|---|
| 要求 | Stem tap/master fx/sidechain → explicit recipe・no false sum |
| Owner | #23 配下（未割当） |
| Fixture | （未実装 — stem/bounce/freeze/render-in-place の export mode・stem tap・sidechain closure を検証する fixture は存在しない） |
| Oracle | stem isolation・sidechain closure・master fx 分離・false sum なし |
| Status | **NotRun** — stem/bounce/freeze 未実装 |
| 証拠 | `docs/designs/media-render-design.md` §Export vs project-derived |
| 後続 | #23 配下の stem/bounce/freeze Issue |

### MR-018 Final int PCM/dither/SRC（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Final int PCM/dither/SRC → conversion verification |
| Owner | #25・#20 |
| Fixture | `crates/synvelis-render/tests/l2_e2e.rs::l2_01_master_export_reopen_pcm_match`（PCM 変換 path）・`l2_07_capability_evidence_on_receipt`・`crates/synvelis-render/tests/tim04_plugin_state.rs::standard_export_completes_and_records_evidence_verbatim`・`strict_accepts_exact_and_standard_passes_degraded` |
| Oracle | PCM 変換規則（clipping/rounding/dither/SRC/channel map/endianness）・dither 単一段階・conversion verify・evidence verbatim |
| Status | **Pass(部分)** — PCM path のみ・dither/SRC は Deferred（#20 系）・実 plugin SRC NotRun |
| 証拠 | `docs/specs/l2-master-sequential-offline-wav-export.md`・`docs/specs/tim-02-player-midi-rt-offline.md` |

### MR-019 Encoder drain/mux finalize failure（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Encoder drain/mux finalize failure → Failed/Recoverable・old file protected |
| Owner | #25・#82 |
| Fixture | `crates/synvelis-render/tests/l2_e2e.rs::l2_05_fault_injection_partial_never_completed`・`l2_05_destination_policy`・`leftover_temps`・`crates/synvelis-project/tests/store_publish.rs`（PublishFailure mapping） |
| Oracle | Staging/Temp 保護・partial 非公開・destination 保護・Failed write は Completed receipt を出さない・leftover temp 回収 |
| Status | **Pass(部分)** — headless fault injection・実 disk-full/durability/remote FS NotRun |
| 証拠 | `docs/designs/media-render-design.md` §OutputPipeline・§Failure taxonomy |

### MR-020 Export中 project edit（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Export中 project edit → fixed snapshot completes・live preserved |
| Owner | #25 |
| Fixture | `crates/synvelis-render/tests/l2_e2e.rs::l2_06_snapshot_freeze_and_immutability`・`crates/synvelis-render/tests/sf03_render.rs::sf03_render_01_same_revision_across_consumers`・`crates/synvelis-render/tests/x_e2e.rs::x_03_identity_revision_undo_save_reload` |
| Oracle | RenderSpec immutable・Project Live≠Saved≠Active Plan≠Render Source・live preserved・revision identity across consumers |
| Status | **Pass(部分)** — headless・実 concurrent edit/plugin state change NotRun |
| 証拠 | `docs/specs/l2-master-sequential-offline-wav-export.md` |

### MR-021 Plugin missing/best-effort state（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Plugin missing/best-effort state → preflight/degraded explicit |
| Owner | #127・#128 |
| Fixture | `crates/synvelis-render/tests/tim04_plugin_state.rs::strict_rejects_every_non_exact_class`・`strict_export_fails_at_preflight_before_destination`・`strict_accepts_exact_and_standard_passes_degraded`・`standard_export_completes_and_records_evidence_verbatim`・`crates/synvelis-player/tests/tim03_process_boundary.rs::tim03_pb_precision_gates`・`tim03_pb_invalid_requests`・`OfflineExact`/`OfflineUncertified` capability gate |
| Oracle | Plugin State quality（Coordinated/Quiescent/BestEffort/Unsupported）・preflight Reject/Degraded・fail-closed・consent 救済なし・evidence verbatim |
| Status | **Pass(部分)** — mock plugin・実 plugin×OS×format matrix NotRun |
| 証拠 | `docs/specs/tim03-process-boundary.md`・`docs/specs/tim-04-plugin-state-snapshot-consistency.md` |

### MR-022 Bounce/Freeze undo/unfreeze（NotRun）

| 項目 | 内容 |
|---|---|
| 要求 | Bounce/Freeze undo/unfreeze → originals preserved idempotent |
| Owner | #23 配下（未割当） |
| Fixture | （未実装 — bounce/freeze/render-in-place command・非破壊 Lineage・unfreeze/re-freeze を検証する fixture は存在しない。tim04 は plugin state recipe のみ） |
| Oracle | 非破壊 Lineage・Original Source/Plugin/Automation 保全・Undo/Redo・idempotent・凍結状態 UI 明示 |
| Status | **NotRun** — bounce/freeze/render-in-place 未実装 |
| 証拠 | `docs/designs/media-render-design.md` §Export vs project-derived |
| 後続 | #23 配下の bounce/freeze Issue |

### MR-023 Cancel/window close/app exit（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Cancel/window close/app exit → job owner-defined |
| Owner | #114・#23 |
| Fixture | `crates/synvelis-gateway/tests/job.rs::ids_stay_distinct_and_jobs_survive_disconnect`・`cancel_verdicts_are_honest`・`jobs_stack`・`input_snapshot_pins_committed_revision`・`job_cancel`・`pending_cancel_upgrades_to_confirmed_on_poll`・`crates/synvelis-workspace/tests/ws_a.rs`（`close_veto` 系）・`workspace_model.rs`・`crates/synvelis-render/tests/l2_e2e.rs`（export cancel phase） |
| Oracle | Window Close≠Job Cancel・job owner は Application/Project Session・View/Selection State と Project Command Target 分離・Cancel≠即時削除・cancel verdict honest |
| Status | **Pass(部分)** — gateway job registry（Generation/Analysis）+ workspace close_veto headless・unified orchestrator（Import/Render job 統合）未実装・実 GUI/OS NotRun |
| 証拠 | `docs/designs/media-render-design.md` §JobOrchestrator・`docs/specs/workspace-model.md`・`docs/test-specs/sv-gui-04-slice2-dock-transfer-workspace-adoption.md` |

### MR-024 Two docked inspector/probe（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Two docked inspector/probe → no double probe・view state independent |
| Owner | #107 系・#114 |
| Fixture | `crates/synvelis-player/tests/sf04_probe_acceptance.rs::sf04_probe_same_key_request_is_idempotent_single_install`・`sf04_probe_removed_entity_unbinds_and_reattach_binds_fresh`・`crates/synvelis-workspace/tests/ws_a.rs`（dock/workspace）・`crates/synvelis-app/tests/sv113_dock.rs` |
| Oracle | Probe Session/Controller は Inspector View 寿命と独立・2 Inspector で Capture 重複なし・view state independent・dock transfer |
| Status | **Pass(部分)** — headless・実 dock GUI/OS NotRun |
| 証拠 | `docs/designs/playback-graph-design.md`・`docs/specs/workspace-model.md`・`docs/test-specs/sv-gui-04-slice2-dock-transfer-workspace-adoption.md` |

### MR-025 Probe Off/Monitor/Capture + bypass（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Probe Off/Monitor/Capture + bypass → DSP unchanged・loss/unavailable identified |
| Owner | #107 系 |
| Fixture | `crates/synvelis-player/tests/sf04_s8_probe_faults.rs::sf04_s8_observation_states_stay_distinct`・`sf04_s8_unreachable_fault_statuses_are_unfaked`・`sf04_s8_chunk_pool_exhaustion_records_lost_blocks`・`sf04_s8_telemetry_overflow_downgrades_coverage_fail_closed`・`sf04_s8_capture_variants_close_with_distinct_reasons`・`crates/synvelis-player/tests/sf04_s3_probe_discontinuity.rs::sf04_s3_probe_modes_leave_master_output_identical` |
| Oracle | Probe Off（既定）/Monitor/Capture・bypass で DSP unchanged・loss/unavailable typed・budget 超過は Capture Loss/Unavailable・fail-closed |
| Status | **Pass(部分)** — headless・実 probe device NotRun |
| 証拠 | `docs/designs/playback-graph-design.md`・`docs/specs/playback-graph.md` |

### MR-026 2 probes PDC/clock correlation（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | 2 probes PDC/clock correlation → same frame/gen compare・missing=unknown |
| Owner | #107 系 |
| Fixture | `crates/synvelis-player/tests/sf04_s7_probe_compare.rs::sf04_s7_monitored_probes_align_on_shared_cursor`・`sf04_s7_partial_coverage_is_conditional_never_aligned`・`sf04_s7_sides_on_different_epochs_are_epoch_mismatch`・`sf04_s7_window_spanning_republish_is_generation_span`・`sf04_s7_window_spanning_seek_is_discontinuity_in_window`・`sf04_s7_evidence_budget_is_bounded`・`sf04_s7_empty_observed_range_is_coverage_empty`・`sf04_s7_unsubscribed_point_is_point_unresolved` |
| Oracle | 同 Frame/Clock/GraphGeneration + PDC/経路 Latency 考慮・時間の揃わない測定値を引き算しない・missing=unknown・coverage conditional |
| Status | **Pass(部分)** — headless・実 PDC/clock device NotRun |
| 証拠 | `docs/designs/playback-graph-design.md`・`docs/specs/playback-graph.md` |

### MR-027 Source→Track→Node→Parameter→Observation trace（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Source→Track→Node→Parameter→Observation trace → Configured/Applied/Observed/Inferred |
| Owner | #107 系・#20 |
| Fixture | `crates/synvelis-player/tests/sf04_s2_probe_trace_bridge.rs::sf04_s2_metric_events_become_measuring_updates_with_derived_stamp`・`sf04_s2_mode_off_maps_to_disabled_and_unrouted_record_returns_false`・`sf04_s2_stale_generation_records_are_rejected`・`crates/synvelis-player/tests/sf04_probe_adapter.rs::sf04_probe_request_queues_then_rt_confirms_bound_active`・`sf04_probe_unresolvable_point_is_recorded_not_faked`・`sf04_probe_stale_generation_command_is_rejected_and_rebound`・`sf04_probe_capture_segment_cuts_at_generation_boundary`・`crates/synvelis-player/tests/pc_c6_oracle_pl.rs`・`trace_c3_05.rs`・`pc_c4_03_observation.rs`・`pc_c4_07_transport_observe.rs`・`docs/adr/syn-obs-01-crate-ruviform-observability-logging-feature-host-global-subscriber.md` |
| Oracle | Configured/Prepared/Applied/Observed/Inferred/Unavailable + Clock/Frame/Revision/Provenance/Loss・原因候補は Confidence 付き・異常観測≠原因特定・stale generation reject |
| Status | **Pass(部分)** — headless・実 plugin/parameter trace NotRun |
| 証拠 | `docs/specs/playback-graph.md`・`docs/specs/tim03-process-boundary.md`・`docs/designs/syn-obs-01-design-host-logging-owner-source-catalog.md` |

### MR-028 Invalid metadata/credentials/path traversal（Pass(部分)）

| 項目 | 内容 |
|---|---|
| 要求 | Invalid metadata/credentials/path traversal → safe reject・no secret leak・blob protection |
| Owner | #9/#85〜#91 |
| Fixture | `crates/synvelis-library/tests/local.rs::traversal_locators_are_rejected_before_any_io`・`symlink_escaping_root_is_listed_but_never_opened`・`ancestor_symlink_cannot_leak_out_of_locator_scope`・`symlink_loop_does_not_recurse_forever`・`crates/synvelis-library/tests/project.rs::hostile_blob_identity_cannot_traverse`・`symlinked_blob_cannot_escape_the_store`・`device_and_nonregular_blob_names_are_rejected`・`crates/synvelis-library/tests/git.rs::hostile_locator_and_url_inputs_fail_closed`・`credential_failures_are_typed_and_secrets_never_leak`・`crates/synvelis-library/tests/webdav.rs::url_parse_rejects_unsafe_forms`・`cross_origin_redirect_refused`・`cross_origin_redirect_never_reaches_other_origin`・`credentials_sent_only_as_authorization_header`・`locator_outside_root_rejected`・`crates/synvelis-library/tests/types.rs::opaque_credential_debug_redacts_context`・`crates/synvelis-library/tests/local.rs::foreign_proof_is_not_confused_with_local_pin`・`cursor_is_rejected_after_query_scope_or_principal_changes` |
| Oracle | traversal/symlink/credential/secret reject・blob store 保護・safe reject・no leak・credential は Authorization header のみ・opaque debug redaction |
| Status | **Pass(部分)** — headless・実 credential store/SSRF/remote FS NotRun |
| 証拠 | `docs/designs/alib-01-asset-provider-contract-design.md`・`docs/designs/media-render-design.md` §AssetLibrary |

## 横断契約写像

### #127 TIM-03 Plugin Process precision（MR-021 関連）

| 項目 | 内容 |
|---|---|
| 契約 | `docs/specs/tim03-process-boundary.md` |
| Fixture | `crates/synvelis-player/tests/tim03_process_boundary.rs::tim03_pb_precision_gates`・`tim03_pb_exact_windows_and_boundary_positions`・`tim03_pb_invalid_requests`・`tim03_pb_degraded_coalescing_and_wrap_bounds`・`tim03_pb_min_max_illegal_segment_policy`・`tim03_pb_offsets_zero_one_last`・`tim03_pb_simultaneous_kinds_dedupe`・`tim03_node_degraded_consented_boundary`・`tim03_node_exact_rejects_illegal_boundary`・`tim03_node_param_event_per_loop_occurrence`・`tim03_node_simultaneous_loop_seek_plan_param`・`tim03_node_stale_transport_generation_zero_delivery`・`tim03_sched_*`・`tim03_process_boundary_rt.rs`・`tim03_attach_wiring.rs`・`tim03_request_plan.rs`・`tim03_session_wiring.rs` |
| Status | Pass(部分) — headless mock・実 plugin×OS×format matrix NotRun |
| MR 写像 | MR-021・MR-R-15 |

### #128 TIM-04 Plugin State snapshot（MR-021 関連）

| 項目 | 内容 |
|---|---|
| 契約 | `docs/specs/tim-04-plugin-state-snapshot-consistency.md` |
| Fixture | `crates/synvelis-player/tests/tim04_plugin_state.rs::capture_after_stop_processing_is_coordinated`・`capture_while_processing_is_best_effort`・`capture_with_quiescent_mode_records_quiescent`・`destroyed_instance_fails_closed`・`unsupported_plugin_reports_unsupported`・`unknown_processor_is_unavailable`・`restore_state`・`crates/synvelis-render/tests/tim04_plugin_state.rs::strict_rejects_every_non_exact_class`・`strict_export_fails_at_preflight_before_destination`・`standard_export_completes_and_records_evidence_verbatim` |
| Status | Pass(部分) — headless mock・実 plugin/OS NotRun |
| MR 写像 | MR-021・MR-R-16 |

### #116/#86 Recording durability vs Verified Import（MR-009/017 関連）

| 項目 | 内容 |
|---|---|
| 契約 | `docs/specs/recording-take-comping.md`・`docs/specs/issue-86-verified-import-wiring.md`・`docs/specs/x-recorded-verified-source-playback-export.md` |
| Fixture | Recording Verified Prefix/Journal: `crates/synvelis-recording/tests/vs_c.rs`・`staged_publish.rs`・`recovery_review.rs`・`rcd_04_recovery_partial.rs`・`rcd_04_take_comp_pcm.rs`・`crates/synvelis-render/tests/rcd_04_rt_comp_pcm.rs`・`crates/synvelis-player/tests/rcd_04_midi_phrase_boundary.rs`。Asset SHA-256 CAS/Project Save: `crates/synvelis-library/tests/import.rs`・`verified_import.rs`（player）・`crates/synvelis-project/tests/head_cas.rs`・`store_publish.rs`・`rcd_04_*.rs`。Probe lossy≠lossless: `crates/synvelis-player/tests/sf04_s8_probe_faults.rs` |
| Status | Pass(部分) — headless・3 系統（recording raw verified prefix / asset CAS / project save）の区別は fixture で確認・統合 receipt 未統一 |
| MR 写像 | MR-R-17・MR-009 |

### RCP-00〜F01 Render Checkpoint（MR-RCP-01）

| 項目 | 内容 |
|---|---|
| 契約 | synvelis #289（RCP-00 設計正本）〜#297（RCP-F01）・`PROJ-RCP-01`（`docs/specs/project-persistence.md`） |
| 状態 | **NotRun** — RCP 系 Issue 未実装 |
| MR 写像 | MR-RCP-01・MR-R-18 |

## 集計

| Status | 件数 |
|---|---|
| Pass(部分) | 26 |
| NotRun | 3（MR-RCP-01・MR-017・MR-022） |
| Pass | 0（本 slice は docs のみ・実機未検証） |
| Fail | 0 |
| Unsupported | 0 |

**注意**: `Pass(部分)` は headless fixture で関連契約が確認
できるが、当該 MR 全体の独立 oracle・実機/GUI/Plugin/OS 受入
を満たさないことを示す。実機/GUI/Plugin/OS 受入は別 lane で
実施されるまで `Pass` としない。
