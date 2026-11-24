# Acceptance Ledger — synvelis#89 ALIB-05 Browser/Import UX headless slice

Date: 2026-11-15. Slice: ALIB-05/ALIB-UX-H — Asset Library
Browser/Import UX の headless view-model + `syn.library.*`
op surface + driver-neutral `ViewSpec` projection
（`crates/synvelis-workspace/src/library_ui.rs` +
`synvelis-app` op 配線）。Issue #89 の UX 受入行を
`library_browser.rs` の 16 integration test・
`library_ui.rs` の 14 unit test・doc chain に写像した
監査台帳。

**実 GPUI mount・実 provider/network/credential・v5
real-data import・OS/IME/accessibility は一切実行して
いない。** headless の Pass を実 GUI/実 provider/実
データの Pass に昇格しない。それらは synvelis#114・
#91・#86（および provider 各 Issue）が Owner。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/89
（OPEN・`Refs` のみ・Close しない）。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis SHA | `7bedcbaa44cae7f496adf59452846324f5d94da0`（branch `scarecrow173/sv89-alib05-browser` の base = origin/main HEAD）+ 本 change set（未 commit） |
| Ruviform resolved SHA | `ac10a2f7761c08db73aebd861c0a5c1f53f269b7`（`Cargo.lock` git dep・本 change set で rev 変更なし。`synvelis-workspace` → `synvelis-library` の dep edge 追加のみ） |
| OS / CPU / FS | Windows（win32）/ NTFS |
| Toolchain | rustc 1.99.1（`58cf8f9c3`、mise 管理。CI pin は `mise.ci.toml` の 1.99.0） |
| Schema | `SCHEMA_VERSION = 14`（synvelis-project） |
| Device + Plugin + Provider | なし（headless — provider は scripted in-memory `FakeProvider`、import ports は temp-dir `ImportStore` + `LocalBlobDir` + `StagedBlobPublisher`。network/credential/実データ不使用） |

## 判定語彙

- **Pass-measured (headless)**: `mise run test
  synvelis-workspace` および `mise exec -- cargo test -p
  synvelis-app --test library_browser` が当該 change set
  で green だった test の存在を確認したことのみを意味する。
- **NotRun**: 実 OS/実 GUI/実 provider/実 network/実
  credential/実データ/実 device — 本 slice で実行して
  いない部分。
- **Unsupported / NotImplemented**: provider capability
  が持たない操作は typed `Unsupported`/`unsupported-
  capability` として表現される設計上の非対応。捏造して
  いない。

## 受入行 → 証拠写像

### Browse / coverage / capability（AL-R-03/04/05、AC-2/8/9/10）

| 受入行 | 要件 | 証拠（test / doc） | 判定 |
| --- | --- | --- | --- |
| AL-R-03 | metadata-first paging・opaque cursor・未選択本体の全量同期なし | `browse_pages_and_typed_states`（`page_limit=64` の page 投影・`next_cursor` 保持）、`browse_pages_and_cursor_invalid`（cursor 尽き後は `Malformed` fault・捏造 page なし） | Pass-measured (headless)。実 provider の cursor 意味論（Git/WebDAV/実 index）は NotRun（#88/#90/#91） |
| AL-R-04 | No Results/Partial/Offline/Stale/Unsupported/IndexIncomplete/権限不足の typed 区別・検索非対応は empty でなく Unsupported | `browse_complete_empty_is_not_failure`（真の 0 件は `coverage:"complete"` で fault ではない）、`unsupported_search_is_typed_not_empty`・`unsupported_capabilities_stay_typed`（`unsupported-capability`/`PinUiState::Unsupported`/`PreviewUiState::Failed` が typed で残る）、`stat_preserves_availability_distinctions`（`Offline`/`Missing{detail}` 区別）、`entry_wire_preserves_observation_and_availability_tags`（`Stale{reason}`/`Unknown{reason}`/`Unavailable{reason}` が wire で保持） | Pass-measured (headless)。実 index-incomplete/unauthorized/offline の provider 実挙動は NotRun（#87/#91） |
| AL-R-05 | metadata 不明は「不明」・preview は必要 byte 量と provider 制約を踏まえ取得 | `preview_plan_consent_and_fetch_ready`（size 不明 → `consent-required` gate・`over-budget` 拒否）、`search_stat_pin_and_preview_flow`（plan→fetch の consent 経路） | Pass-measured (headless)。実 preview byte decode・`LibraryCache` 経由 preview は NotRun（#87 領域） |
| AC-9 | provider capability を UI へ正しく返す | `open_view_projects_providers_state_and_spec`（`ProviderCapabilities` が `supported`/`unsupported{reason}`/`PinCapability` tag・`access` mode として serialize）、`unsupported_capabilities_stay_typed` | Pass-measured (headless)。Git ホスト API vs 汎用 Git・WebDAV の実 capability 差は provider gate（#88/#90） |

### Pin / preview / drag（AL-R-06/08 の UX 面、AL-R-16 要素）

| 受入行 | 要件 | 証拠 | 判定 |
| --- | --- | --- | --- |
| AL-R-06/08（UX 面） | pin は typed evidence であり project write ではない | `pin_unsupported_and_supported`（`Pinned{proof:"immutable-object"}`/`Unsupported`）、`search_stat_pin_and_preview_flow`（pin 後も project revision 不変） | Pass-measured (headless)。Git ref→固定 Commit/Tree/blob・LFS pointer≠実 Byte・WebDAV strong ETag/`If-Match`/412→`SourceChanged` の実 semantics は NotRun（#88/#90/#331） |
| preview ≠ import | preview `Ready` は unverified observation で import job/`CommittedLive`/revision を生まない | `preview_plan_consent_and_fetch_ready`・`search_stat_pin_and_preview_flow`（fetch `Ready` 後も `import.state=="idle"`・revision 不変） | Pass-measured (headless) |
| AL-R-16 drag 条項 | pending drag/drop ghost は verified import+commit 前に Project Asset/Source/Clip record を作らない | `drag_ghost_never_mutates`・`drag_ghost_never_mutates_project`（`Ghost{locator,display_name}` のみ・revision 不変・drop で `Idle`） | Pass-measured (headless)。実 window 上の ghost 描画・drop target 配線は NotRun（#114） |

### Import（AL-R-10/11、AC-4 の UX 面）

| 受入行 | 要件 | 証拠 | 判定 |
| --- | --- | --- | --- |
| AL-R-10 | `ImportIntent{import_id, expected_project_revision, source: PinnedAssetRef, target}`・submit 前の明示 pin・同一 import_id replay 冪等 | `import_commits_live_and_replays_idempotently`（`CommittedLive` receipt: import_id/asset_id/source_id/content_digest/byte_len/`reused:false` → 同一 replay で `reused:true`・二重 commit なし）、`import_committed_live_with_receipt`（unit — fake の `open_pinned` 非対応を typed failure で確認し `CommittedLive` を捏造しない） | Pass-measured (headless)。bounded streaming/SHA-256/staged readback/atomic publish/manifest-receipt recovery の内部正しさは `import_verified`（ALIB-02/#86）の責務であり本 slice はその呼出経路のみ検証 |
| AL-R-11 | `CommittedLive` ≠ saved HEAD・`StaleRevision`→rebase→retry・`Failed`/`Cancelled` typed | `CommittedLive` receipt が revision・digest を持ち save 状態を主張しないことは上記 test で固定。`import_stale_revision_rebase_retry_recovers`（`StaleRevision{import_id,expected,actual}` → `import.rebase` → `import.retry` で commit 回復・`import_id` 不一致 rebase は `malformed`）、`provider_switch_scopes_import_surface`（provider 切替で pending intent 破棄・`CommittedLive` receipt は project truth として残存）、`library_backend_survives_project_new`（`project.new` 後の backend 再 bind） | Pass-measured (headless) |
| import 境界 | import services 未 bind・provider fetch 非対応は typed で project 不変 | `import_without_backend_or_fetch_is_typed`（ports 無し → `malformed` fault・revision 不変、`open_pinned` 非対応 → typed fault で `committed-live` を名乗らない） | Pass-measured (headless) |

### AL-R-16 UX 統合要素の写像

| AL-R-16 要素 | 証拠 | 判定 |
| --- | --- | --- |
| source/provider selector | `open_view_projects_providers_state_and_spec`（catalog + capability 表示）、`provider_switch_invalidates_bound_state`（未知 provider は `unknown-provider` fault・現 provider 保持） | Pass-measured (headless) |
| catalog/list + coverage badge | `browse_pages_and_typed_states`（`coverage` tag + entries + `next_cursor`） | Pass-measured (headless) |
| search/filter（capability-aware） | `unsupported_capabilities_stay_typed`・`search_stat_pin_and_preview_flow`（provider-side search が listing を置換・`search.clear` で復帰） | Pass-measured (headless)。filter UI の実描画は NotRun（#114） |
| revision selector | pin の `proof`/`object`/`expected_size` が revision identity を運ぶ（`pin_unsupported_and_supported`）。独立した revision-history selector は provider 側 capability（#88/#90）依存 | 部分 — pin evidence は Pass-measured (headless)、selector 要素自体は NotRun |
| metadata inspector | `entry.select`→`entry.stat` で `StatWire{availability,facts,provenance}`（`search_stat_pin_and_preview_flow`・`stat_preserves_availability_distinctions`）。unknown metadata は `EntryFacts::all_unknown` のまま「不明」 | Pass-measured (headless) |
| preview affordance | `preview.plan`/`preview.fetch`/`preview.cancel` → `PlanReady{bounded|consent-required|over-budget}`/`Ready`/`Cancelled`/`Failed` | Pass-measured (headless) |
| selected/pinned intent + import progress | `state.pin`/`state.import`/`state.preview`/`state.drag` が typed surface state として `syn.library.view` に投影（`open_view_projects_providers_state_and_spec`） | Pass-measured (headless)。progress bar 等の実描画は NotRun（#114） |
| cancel/conflict/retry/recovery | `preview.cancel`→`Cancelled`（`preview_cancel_is_typed`・`preview_cancel_does_not_poison_later_plan` — cancel token は fetch 専用で後続 plan を汚染しない）。import の `Failed{reason}`/`Cancelled`/`StaleRevision`→rebase→retry は `import_stale_revision_rebase_retry_recovers` で計測。`import.cancel` action は本 slice に存在しない（同期 op のため mid-flight cancel は preview のみ） | Pass-measured (headless) |
| offline/permission message | `LibraryReason` の `offline`/`permission-denied`/`auth-expired`/`unsupported-capability` が `state.fault`/`PinUiState`/`PreviewUiState`/`ImportUiState` に typed 着地（`malformed_and_implicit_actions_are_typed`・`unsupported_capabilities_stay_typed`）。実 offline/credential revoke の provider 応答は NotRun（#91） | Pass-measured (headless)（typed 経路）・実挙動 NotRun |

### View/op surface・lifecycle

| 受入行 | 要件 | 証拠 | 判定 |
| --- | --- | --- | --- |
| op surface | `syn.library.providers`/`visual`/`view`/`action` が `sys.capabilities` と `UiRuntime::dispatch_op` 経路で応答 | `ops_registered_and_dispatch_through_runtime` | Pass-measured (headless) |
| view lifecycle | open が `ViewType::AssetLibrary` を adopt・close が共有 teardown で model ごと除去・wrong-kind/未登録 view は `invalid_params` | `open_view_projects_providers_state_and_spec`・`close_removes_view_and_model`・`view_rejects_unregistered_and_wrong_kind` | Pass-measured (headless) |
| unbound backend | provider catalog を捏造しない — `providers` は空 list・view/action op は `unavailable` | `unbound_backend_is_typed_not_fabricated` | Pass-measured (headless) |
| deterministic ViewSpec | 同一 `(model,state)` で同一 serialization・`library.*` stable key | `open_view_projects_providers_state_and_spec`（spec1==spec2・`library.providers`/`library.browse` 含有）・`providers_and_select_roundtrip`（unit） | Pass-measured (headless) |
| 明示 target | mutating action は locator を明示・ambient selection を暗黙 target にしない | `malformed_and_implicit_actions_are_typed`（ports 無し import.submit は `malformed`）。`preview.fetch` の `locator` は必須 field — selection fallback は存在しない | Pass-measured (headless) |

### AL-R-17 / AC-11/13/15（UX 消費側の残り）

| 受入行 | 要件 | 証拠 | 判定 |
| --- | --- | --- | --- |
| AL-R-17/AC-11 | 特定 revision import 後 network 切断でも Project 再生に必要 data が残る（copy into Project 既定） | `import_commits_live_and_replays_idempotently` は verified bytes が `LocalBlobDir` 経由で project commit されることを headless で確認。offline playback・実 blob 永続化は #86/#87 の責務 | 部分 Pass-measured (headless) — commit 経路のみ。実 offline playback は NotRun |
| AC-13 | source 更新後も既 Import Asset は勝手に差し替わらず来歴・更新候補を確認可能 | `StatWire.provenance`/`BrowseEntryWire` が `ProvenanceRecord`/`Availability` を typed 保持（`stat_preserves_availability_distinctions`）。update-availability diff UI は #97 領域 | 部分 Pass-measured (headless) — provenance typed 保持のみ。更新候補提示 UI は NotRun |
| AC-15 | VCS 未接続でも local 素材 import・通常 DAW 制作が可能 | provider registry は provider 非依存で、本 slice は登録済み provider なら何でも駆動できることを `FakeProvider`（`ProviderKind::LocalFolder`）で実証。実 local provider 配線は #85/#91 | 部分 Pass-measured (headless) — surface の provider 非依存性のみ。実 local provider は NotRun |

### 非 UX 行（本 slice の対象外 — 正直に NotRun）

| 行 | 内容 | 判定 |
| --- | --- | --- |
| 実 GPUI mount | `ViewSpec`/`LibraryViewState` の実 driver 消費・実 window/IME/accessibility | NotRun（synvelis#114） |
| 実 provider/network/credential | Git/WebDAV/実 server・LFS・ETag semantics・auth revoke・fault matrix 実環境 | NotRun（#88/#90/#331・#91） |
| v5 real-data import | 旧 snapshot 移行データでの import・`inspect_imports` manifest recovery op | NotRun（#86） |
| `syn.library.jobs` ledger | cross-view import job recovery ledger | NotRun（deferred — 別 slice） |
| `CallOutcome::Job` 化 | 長時間 fetch/import の mid-flight cancel・progress | NotRun（deferred — 同期 op は `syn.vcs.*` と同じ discipline） |
| OS-L1/GUI-L2/EXT-L3 | Windows 実機 lane・3OS・実 device | NotRun（POLICIES.md の利用条件成立まで Deferred） |

## 既知の未カバー（正直記録）

- `import.cancel` action は存在しない — 同期 op のため
  import 途中 cancel の UI 経路はなく、`ImportUiState::
  Cancelled` は provider/import 層が返す cancelled の写像
  のみ。job 化 slice で要検討。
- `import.submit` の `Io`/`Corrupt`/`PublishFailed`/
  `JobState` `ImportError` は `Failed{reason:"internal"}`
  に丸められる（`IntegrityMismatch`/`CommitRejected`/
  `PublishIndeterminate`/`Provider`/`StaleRevision`/
  `IntegrityConflict` は typed）— stage manifest の
  `Recoverable` resume 等の細かい typed 回復は
  `syn.library.jobs` slice の責務。

## fixture 補完判定

新規 fixture/test として `crates/synvelis-app/tests/
common/library.rs`（scripted `FakeProvider` +
`app()`/`app_with()`/`app_with_import()`/
`app_with_import_registry()`）と
`crates/synvelis-app/tests/library_browser.rs`（16 test）、
および `library_ui.rs` 内 `#[cfg(test)]`（14 unit test、
`search_mode_survives_refresh_and_more`/
`preview_cancel_does_not_poison_later_plan` regression
含む）を追加。provider は in-memory fake、
import ports は temp-dir の実 `ImportStore`/`LocalBlobDir`/
`StagedBlobPublisher` — headless 成功が real provider/実
データ受入を装わない境界を維持した。

## Doc chain

- spec: `docs/specs/asset-library.md`（既存・AL-R-01〜17）
- design: `docs/designs/asset-library-design.md`（既存）
- umbrella test-spec: `docs/test-specs/asset-library.md`
  （既存・AL-T-R-01〜08）
- test-spec: `docs/test-specs/alib-05-browser-import-ux.md`
  （新規）
- impl record: `docs/impl/ir/alib-05-browser-import-ux.md`
  （新規）
- experiment log: `docs/impl/exp/alib-05-browser-import-ux.md`
  （新規）
- evidence: `evidence/sv89/acceptance-ledger.md`・
  `evidence/sv89/verification.md`（新規）
- index: `docs/impl/ir/README.md`・`docs/test-specs/README.md`
  を再生成。`docs/impl/exp/README.md` は `.jsonl` index のみ
  管理で `.md` exp memo は対象外（cb-01〜09・sv286 と同じ
  扱い）

## ALIB-05 native mount lane（2026-11-24 追記）

`SYNVELIS_ASSET_LIBRARY`/`SYNVELIS_LIBRARY_DIR`/
`SYNVELIS_PROJECT_DIR` の env gate で `synvelis-native` が
実 `LocalFolderProvider`・実 import ports を bind し、
`syn.library.visual{action:"open"}` → `syn.library.view` の
返却 ViewSpec を `ViewHost`/`MountSpec::Library` に verbatim
mount する経路。`library_native_mount.rs` 7 test（uitest
feature で 7 件・default では 6 件）に写像。

**実 OS window/GPU/IME/focus/keyboard・実 network/
credential・v5 real-data import は一切実行していない。**
headless 検証の Pass を実 GUI 受入（#114）に昇格しない。

### 受入行 → 証拠写像（mount lane）

| 受入行 | 要件 | 証拠（test） | 判定 |
| --- | --- | --- | --- |
| wire spec verbatim mount | `syn.library.view` 返却 spec を第二の spec 形へ再解釈せず `ViewHost` に載せる | `library_view_mounts_wire_spec_verbatim`（`host.spec()` が wire spec と一致） | Pass-measured (headless) |
| 実 provider 配線 | `SYNVELIS_LIBRARY_DIR` で実 `LocalFolderProvider` を bind（fake 不使用・製品と同じ provider 種別） | `mounted_view_projects_real_local_folder_state`（browse/stat/pin `conditional-strong`/preview typed 投影） | Pass-measured (headless)。network/credential provider は NotRun（#88/#90/#91） |
| 実 import ports | `SYNVELIS_PROJECT_DIR` で実 `ImportStore`/`LocalBlobDir`/`StagedBlobPublisher` を bind し `import.submit` が commit される | `import_submits_through_real_import_ports`（`committed-live` receipt・blob が project media root に commit） | Pass-measured (headless) |
| project authority 非汚染 | mount/close/browse/pin/preview は revision・engine・transport・project name を汚さない | `mount_lifecycle_and_browse_ops_leave_project_authority_untouched` | Pass-measured (headless) |
| unbound fail-closed | env 未設定/不正 dir で backend unbound のまま typed `unavailable`・session を殺さない | `unbound_backend_ops_answer_typed_unavailable`（`providers` 空 list・view/action `unavailable`・spec 再読み `None`） | Pass-measured (headless) |
| shell lifecycle・refresh | attach→dock_domain 採用→`syn.library.*` dispatch で dirty mark→run loop が `syn.library.view` 再読み→spec 変更時のみ `update_view` push→destroy で pool | `native_shell_attaches_marks_dirty_and_re_reads_library_spec`（`NativeShell`+`FakeDriver`）・`uitest_adapter_pushes_refreshed_library_spec`（uitest gate・同一 semantics） | Pass-measured (headless)。実 OS window lifecycle は NotRun（#114） |
| `synvelis.library` action routing | spec control の Button/TableSelection action を `syn.library.action` へ route・entries table `select{row}`→`entry.select{locator}` 解決 | `native_shell_attaches_marks_dirty_and_re_reads_library_spec`（routing arm・`dispatch_library_action`・`library_row_locator` の unit 観測）。**mounted control からの wire 到達は `binding.rs::decode_view_command` の params admission が `view`/`autx.*` のみのため現行 decode で reject — 別途記録** | **部分 Pass**: shell routing は実装・検証済みだが、binding admission（リース外・Coordinator 相談中）が未解決のため mounted control 発行 action は未到達。op 直接 dispatch 経路は全機能 green |

### 既知の未カバー（mount lane・正直記録）

- `binding.rs::decode_view_command`（`binding.rs:974-982`）
  は params 付き Action intent を `view`/`autx.*` 以外で
  `invalid_params` reject — mounted Asset Library の全
  spec control（Button は常に params、entries table は
  `select{row}`）が decode で落ちる。admission 拡張は
  binding.rs（本 slice リース外）への最小変更（target
  admission + doc comment）として Coordinator に相談中。
  承認なら別 commit で追加検証、却下なら deferred のまま
  納品（op surface 経路はこの制約に依存しない）。
- `mise run fmt`（`cargo fmt --all`）は `os error 206`
  （Windows コマンド長制限・workspace 全 member 単一
  rustfmt 呼出し）で実行不能 — 環境制約で base commit
  でも同一。`cargo fmt --package synvelis-app -- --check`
  で変更全ファイルの準拠を確認済み（verification.md
  参照）。
- 実 OS window・GPU・IME・focus・keyboard — synvelis#114
  へ deferred のまま。
