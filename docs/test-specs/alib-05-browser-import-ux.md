---
id: "TSPEC-Alib05BrowserUxH000001"
type: "test-spec"
status: "draft"
title: "ALIB-05 Browser/Import UX headless view-model + syn.library.* ops + driver-neutral ViewSpec（synvelis#89 ALIB-UX-H slice）"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/89"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/asset-library.md"
    - "docs/test-specs/alib-07-fault-matrix.md"
    - "docs/test-specs/cb-09-diff-explorer.md"
    - "docs/impl/ir/cb-09-diff-explorer.md"
    - "docs/impl/exp/cb-09-diff-explorer.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/asset-library-design.md"
  derived-by: []
  verifies:
    - "docs/specs/asset-library.md"
    - "docs/designs/asset-library-design.md"
  verified-by:
    - "crates/synvelis-app/tests/library_browser.rs"
    - "crates/synvelis-app/tests/common/library.rs"
  references:
    - "evidence/sv89/acceptance-ledger.md"
    - "evidence/sv89/verification.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/91"
    - "https://github.com/scarecrow173/synvelis/issues/86"
  deferred-by: []
---

# ALIB-05 Browser/Import UX headless view-model + syn.library.* ops + driver-neutral ViewSpec

## Purpose

synvelis#89（ALIB-05 / ALIB-UX-H）の headless slice が固定する
検証意図を記録する。`synvelis-workspace::library_ui` の
per-view `LibraryViewModel`、`synvelis-app` の `syn.library.*`
op surface（`providers`/`visual`/`view`/`action`）、および
driver-neutral `ViewSpec` projection（`library_spec`）が、
`docs/specs/asset-library.md` の AL-R-16/17 と関連する
browse/coverage/pin/preview/import 契約を headless で
型付きに満たすことを保証する。

実 GPUI mount（#114）・実 provider/network/credential
backend（#91）・v5 real-data import（#86）は本 slice の
対象外であり、それらの保証が必要になった時点で別
test-spec/acceptance gate が必要。本 spec の Pass を実
GUI/実 provider/実データの受入へ昇格しない。

## Feature

ALIB-05 Browser/Import UX — Asset Library の headless
view-model・`syn.library.*` op surface・driver-neutral
ViewSpec projection

## Rules

- Rule: browse は metadata-first の opaque cursor paging で、
  `coverage` は `complete`/`partial`/`stale`/`offline-cached`/
  `never-indexed` を区別する typed tag として保持する。
  `browse.more` は保存済み cursor をそのまま provider へ渡し、
  cursor 尽きた後の呼び出しは `Malformed` fault であり
  捏造 page を返さない（AL-R-03/04、LIB-BROWSE）。
- Rule: typed state は決して collapse しない。
  `no-provider`/`unknown-provider`/
  `unsupported-capability`（search/pin/preview 非対応）、
  `offline`、`permission-denied`、`auth-expired`、`missing`、
  `cursor-invalid`、`source-changed`、`integrity-mismatch`、
  `budget-exceeded`、`cancelled`、`stale-revision`、
  `integrity-conflict`、`target-gone`、`malformed`、
  `internal` は `LibraryReason`/`LibraryFault` または各
  surface state（`PinUiState`/`PreviewUiState`/
  `ImportUiState`/`DragUiState`）の型付き値として残り、
  empty listing・`Pass`・generic error 文字列へ丸めない
  （AL-R-04、AL-T-R-02）。
- Rule: capability は `ProviderCapabilities` を wire に
  verbatim で載せ、`supported`/`unsupported{reason}`/
  `PinCapability` tag を潰さない。capability 非対応の
  affordance は typed `Unsupported` であり empty や成功を
  装わない（AL-R-04、AC-9）。
- Rule: wire DTO（`ProviderWire`/`BrowseEntryWire`/
  `StatWire`）は `synvelis-library` の型付き値
  （`AssetObservation<u64>`/`AssetObservation<ObservedAt>`、
  `Availability`、`EntryFacts`、`ProvenanceRecord`、
  `EntryKind`）をそのまま埋め込み、`observed`/`inferred`/
  `unknown{reason}`/`stale{reason}` と availability の
  reason が end-to-end で typed を保つ。stale な byte 長を
  fresh な観測値として表示しない（AL-R-02/05）。
- Rule: すべての mutating `LibraryAction` は対象 locator を
  明示する（`provider.select`/`browse.scope`/`entry.*`/
  `preview.*`/`import.submit`/`drag.begin`）。ambient な
  selection を暗黙の target にしない。`preview.fetch` のみ
  `locator` を省略可能とし、その場合は view の現在
  selection を使う — spec が投影する button は常に
  locator を明示して emit する。
- Rule: preview は import ではない。`preview.plan` は
  `bounded`/`consent-required`/`over-budget` の plan tag を
  返し、whole-object fetch は `allow_full_object` の明示
  consent がなければ `PlanReady{consent-required}` に留まる。
  `preview.fetch` の `Ready` は unverified observation で
  あり、import job・`CommittedLive`・project revision を
  一切 mint/変更しない。`preview.cancel` は in-flight fetch
  を協調 cancel token で `Cancelled` に落とす（AL-R-05、
  AL-R-10 末項、EX-4）。
- Rule: pin は import ではない。`entry.pin` は
  `PinUiState::{Pinned{proof,object,expected_size},
  Unsupported{reason}, Failed{reason,detail}}` の証拠を
  産むだけで project を書き換えない（AL-R-06/08 の UX 面）。
- Rule: `import.submit` は `ImportIntent{import_id
  (caller-supplied idempotency key), expected_project_revision,
  source: PinnedAssetRef, target}` を構築し、submit 前に
  provider `pin` を明示的に実行する。`import_verified`/
  `rebase_import` 経由でのみ commit し、成功は
  `ImportUiState::CommittedLive`（import_id・asset_id・
  source_id・clip_id・revision・content_digest・byte_len・
  reused receipt）として現れる。`CommittedLive` は
  project commit の receipt であり saved `HEAD`/
  `SavedRevision` とは別軸 — save 完了を意味しない
  （AL-R-10/11、AC-4 の UX 面）。
- Rule: `StaleRevision` は `ImportUiState::StaleRevision
  {import_id, expected, actual}` として保持され、明示的な
  `import.rebase` + `import.retry` でのみ回復する。
  `IntegrityConflict`（同一 import id・異 digest replay）・
  `Failed{reason}`・`Cancelled` も typed のまま。
  stage-level manifest recovery（`inspect_imports`）は本
  slice では別 op として surface しない（AL-R-11）。
- Rule: drag は ghost のみ。`drag.begin` は
  `DragUiState::Ghost{locator, display_name}` を立てるが
  Project Asset/Source/Clip record を verified import+commit
  前に作らず、project revision を動かさない。`drag.drop`
  で `Idle` に戻る（AL-R-16 pending drag/drop ghost 条項）。
- Rule: `syn.library.action` の action-level failure は op
  error にせず `state.fault`/typed surface state に着地
  させる。malformed params・未知 action tag・未登録
  view_id・wrong-kind view・unbound backend は typed
  `ErrorBody`（`invalid_params`/`unavailable`）であり、
  backend 未 bind 時に provider catalog を捏造しない
  （`syn.library.providers` は空 list、view/action op は
  `unavailable`）。
- Rule: `library_spec(&LibraryViewModel)` は同一
  `(model, state)` に対し deterministic で、component key
  は `library.*` 名前空間で一意。executable な affordance
  のみが `Button` になり、typed state は `Label`/`Table`
  cell として観測可能に投影される。
- Rule: provider 切替（`provider.select`）は provider-bound
  state を全て invalidate する — entries・cursor・coverage・
  search・selection・stat・pin・preview・scope・drag ghost
  は前 provider のものを残さず、切替後の browse は新
  provider の root からやり直す。未知 provider は
  `unknown-provider` fault で現 provider を保持する。
- Rule: `LibraryViewState` は view-local（provider 選択・
  scope・entries・cursor・search・selection・stat・
  pin/preview/import/drag surface・fault）のみを保持し、
  provider/project の authority を持たない。view 削除で
  model ごと破棄され、非 library view には model を
  materialize しない（lazy materialization）。

## Examples

- Example: `browse_pages_and_typed_states` /
  `browse_pages_and_cursor_invalid` — `Partial` coverage の
  page → `browse.more` で `Complete` に遷移し cursor が
  尽きた後の `browse.more` は `Malformed` fault。
- Example: `unsupported_search_is_typed_not_empty` /
  `unsupported_capabilities_stay_typed` — search/pin/preview
  非対応 provider で `unsupported-capability`・
  `PinUiState::Unsupported`・`PreviewUiState::Failed` が
  型付きで残り、entries は empty のまま「0 件成功」に
  見せない。
- Example: `browse_complete_empty_is_not_failure` — 真の
  0 件は `coverage:"complete"` + entries 空であり fault
  ではない（`Empty` と `Unsupported` の区別）。
- Example: `stat_preserves_availability_distinctions` /
  `entry_wire_preserves_observation_and_availability_tags` —
  `Offline`/`Missing{detail}`/`Unavailable{reason}` と
  `Stale{reason}`/`Unknown{reason}` observation が wire
  でも tag を保つ（stale byte_len が fresh に見えない）。
- Example: `preview_plan_consent_and_fetch_ready` /
  `search_stat_pin_and_preview_flow` — size 不明 entry の
  preview は `consent-required` で gate され、consent 付き
  fetch の `Ready` 後も `import.state=="idle"`・project
  revision 不変。
- Example: `preview_cancel_is_typed` — cancel は
  `PreviewUiState::Cancelled`。
- Example: `preview_cancel_does_not_poison_later_plan` —
  cancel 済み token が後続 `preview.plan` の stat probe を
  汚染せず、plan は正しい size hint で classify される
  （regression: cancel token は fetch 専用）。
- Example: `import_stale_revision_rebase_retry_recovers` /
  `provider_switch_scopes_import_surface` /
  `library_backend_survives_project_new` —
  `StaleRevision`→`import.rebase`→`import.retry` の回復
  経路、provider 切替時の pending import scoping、
  `project.new` 後の backend 再 bind が typed に固定
  される。
- Example: `pin_unsupported_and_supported` — pin 非対応は
  `PinUiState::Unsupported`、対応時は `Pinned{proof:
  "immutable-object"}` であり project revision は不変。
- Example: `drag_ghost_never_mutates` /
  `drag_ghost_never_mutates_project` — ghost 中も import
  surface は `idle`、project revision 不変。
- Example: `malformed_and_implicit_actions_are_typed` /
  `malformed_action_params_are_invalid_params` — provider
  未選択の browse は `no-provider`、未知 provider は
  `unknown-provider`、ports 無し import.submit は
  `malformed`、未知 action tag は op error `invalid_params`。
- Example: `import_committed_live_with_receipt` /
  `import_commits_live_and_replays_idempotently` —
  `import.submit` が `CommittedLive` receipt（import_id・
  asset_id・source_id・digest・byte_len・reused=false）を
  返し live revision を進め、同一 import_id replay は
  `reused=true` の冪等終端。
- Example: `import_without_backend_or_fetch_is_typed` —
  import services 未 bind は `malformed` fault、
  `open_pinned` 非対応 provider は typed fault で
  `committed-live` を捏造しない。
- Example: `ops_registered_and_dispatch_through_runtime` /
  `open_view_projects_providers_state_and_spec` /
  `view_rejects_unregistered_and_wrong_kind` /
  `close_removes_view_and_model` /
  `unbound_backend_is_typed_not_fabricated` /
  `provider_switch_invalidates_bound_state` — op 登録・
  `sys.capabilities` 露出・view lifecycle・deterministic
  ViewSpec・wrong-kind/unbound の typed reject・provider
  切替時の bound state 無効化。
- Example（unit、`library_ui.rs` `#[cfg(test)]`）:
  `providers_and_select_roundtrip`（spec determinism）、
  `browse_complete_empty_is_not_failure`、
  `browse_pages_and_cursor_invalid`、
  `unsupported_search_is_typed_not_empty`、
  `search_mode_survives_refresh_and_more`、
  `pin_unsupported_and_supported`、
  `stat_preserves_availability_distinctions`、
  `entry_wire_preserves_observation_and_availability_tags`、
  `preview_plan_consent_and_fetch_ready`、
  `preview_cancel_is_typed`、
  `preview_cancel_does_not_poison_later_plan`、
  `drag_ghost_never_mutates`、
  `malformed_and_implicit_actions_are_typed`、
  `import_committed_live_with_receipt`（typed failure まで
  — fake の `open_pinned` 非対応を確認）。

## Guarantees

- `syn.library.*` 4 op が `AppService`/`UiRuntime` dispatch
  経路で登録・応答し、`syn.library.view` が providers・
  typed `LibraryViewState`・deterministic `ViewSpec` を
  そのまま返す。
- browse/search/stat/pin/preview/import/drag の全 surface
  state が typed で、unsupported/offline/missing/stale/
  cancelled が empty・成功・generic error に collapse
  しない。
- preview `Ready` ≠ verified import、pin `Pinned` ≠
  project write、`CommittedLive` ≠ saved HEAD — 3 つの
  状態分離が test で固定される。
- import は `import_verified`/`rebase_import` +
  `ProjectController` のみを通り、同一 `ImportId` replay が
  `reused` 冪等になる。失敗は `StaleRevision`→rebase→retry
  または typed `Failed`/`Cancelled`。
- unbound backend は空 catalog + typed `unavailable` で
  あり、provider/catalog を捏造しない。
- `library_spec` が deterministic・stable `library.*` key・
  同一 state で同一 serialization を満たす。

## Non-goals

- 実 GPUI/GPUI-component mount・実 window/OS/IME —
  synvelis#114 が Owner。`ViewSpec` の driver 側
  rendering 実装は別 slice。
- 実 provider・実 network・実 credential store —
  synvelis#91（ALIB-07 fault matrix）と #88/#90/#331 が
  Owner。本 slice の provider は scripted in-memory
  `FakeProvider` のみ。
- v5 real-data import・`inspect_imports` の stage-level
  manifest recovery op・`syn.library.jobs` cross-view
  recovery ledger — deferred。
- `CallOutcome::Job` wrapping による長時間 preview
  fetch/import の mid-flight cancel — 本 slice は service
  thread 同期実行（`syn.vcs.*` と同じ）。協調 cancel
  token は preview read loop に配線済みで、job-thread 化は
  contract 変更なしに後続可能。
- `LibraryCache`-backed preview（ALIB-03/#87 領域）。
- provider capability の実値（Git/LFS/WebDAV 固有の pin
  proof・ETag semantics）は provider 側 gate の責務 —
  本 slice は wire が typed 値を潰さないことのみ保証。

## Risk

- `library_ui.rs` は約 3000 行の単一モジュール —
  view-model・action・wire DTO・ViewSpec projection・
  `LibraryViews` registry が同居する。責務は型で分離済み
  だが、実 provider 配線（#91）や `syn.library.jobs`
  ledger 追加時に拡張点の整理が必要になる可能性。
- `import.submit` の `Io`/`Corrupt`/`PublishFailed`/
  `JobState` `ImportError` は `ImportUiState::Failed
  {reason:"internal"}` に丸められる（`IntegrityMismatch`/
  `CommitRejected`/`PublishIndeterminate`/`Provider` は
  typed）— stage-level の回復可能性（`Recoverable`
  resume token 等）の typed 表現は `syn.library.jobs`
  ledger slice で拡張が必要。
- synchronous op 実行は長時間 fetch で service thread を
  占有する — headless slice の既知制約であり、job 化は
  `CallOutcome::Job` 導入時に対処。
