---
id: "IMPL-Sv89Alib05Browser00001"
type: "impl"
status: "in-progress"
title: "ALIB-05 Browser/Import UX headless view-model + syn.library.* ops（synvelis#89）"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/89"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/src/library_ui.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/common/library.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/library_browser.rs"
      - type: "file"
        path: "docs/test-specs/alib-05-browser-import-ux.md"
      - type: "file"
        path: "docs/impl/ir/alib-05-browser-import-ux.md"
      - type: "file"
        path: "docs/impl/exp/alib-05-browser-import-ux.md"
      - type: "file"
        path: "evidence/sv89/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv89/verification.md"
    modified:
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "crates/synvelis-workspace/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/view.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/workspace.rs"
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/service.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/common/mod.rs"
      - type: "file"
        path: "docs/specs/asset-library.md"
      - type: "file"
        path: "docs/designs/asset-library-design.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
  implements:
    - "docs/specs/asset-library.md"
    - "docs/designs/asset-library-design.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/asset-library.md"
    - "docs/test-specs/alib-07-fault-matrix.md"
    - "docs/impl/ir/alib-07-fault-matrix.md"
    - "evidence/sv89/"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/alib-05-browser-import-ux.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/alib-05-browser-import-ux.md"
    rejected: []
---

# ALIB-05 Browser/Import UX headless view-model + syn.library.* ops（synvelis#89）

## Summary

synvelis#89（ALIB-05 / ALIB-UX-H）の headless slice を実装
した。`synvelis-workspace::library_ui`（新規、約 3000 行）が
per-view `LibraryViewModel`・typed wire DTO・driver-neutral
`ViewSpec` projection（`library_spec`）・`LibraryViews`
registry を提供し、`synvelis-app` が `syn.library.providers`/
`visual`/`view`/`action` の 4 op として service dispatch に
配線する。browse（opaque cursor paging）・coverage・
search・stat・pin・preview plan/fetch/cancel・verified
import（`import_verified`/`rebase_import` +
`ProjectController`）・drag ghost を全て typed state と
して投影し、unsupported/offline/missing/stale/cancelled を
empty・成功・generic error に collapse させない。
HEADLESS ONLY — 実 GPUI mount（#114）・実 provider/network/
credential（#91）・v5 real-data import（#86）は対象外。

## Context

synvelis#89 は `docs/specs/asset-library.md`（AL-R-01〜17）
と `docs/designs/asset-library-design.md` の Browser/Import
UX 契約を消費する実装 Issue である。本 slice はそのうち
headless view-model + op surface + ViewSpec projection
までを対象とし、実 GUI mount・実 provider 登録・実データ
import を後続 gate（#114/#91/#86）へ明示的に defer する。
`docs/test-specs/alib-05-browser-import-ux.md` が本 slice
の検証意図を固定し、`evidence/sv89/` が受入写像と検証
証跡を保持する。Issue #89 は open 維持（`Refs` のみ・
Close しない）。

## Implementation

### File map

| File | 変更 | 内容 |
|---|---|---|
| `crates/synvelis-workspace/src/library_ui.rs` | added（約 3000 行） | `LibraryViewModel`・`LibraryViewState`・`LibraryAction`・wire DTO（`ProviderWire`/`BrowseEntryWire`/`StatWire`/`ImportTargetWire`）・surface state（`PinUiState`/`PreviewUiState`/`ImportUiState`/`DragUiState`）・`LibraryFault`/`LibraryReason`・`LibraryImportPorts`・`LibraryBackend`/`LibraryViews`・`library_spec` → `ViewSpec`・`#[cfg(test)]` 14 unit test |
| `crates/synvelis-workspace/src/view.rs` | modified | `ViewType::AssetLibrary` variant 追加 |
| `crates/synvelis-workspace/src/workspace.rs` | modified | `library: LibraryViews` field・`library()`/`library_mut()`/`with_library()`（import 用 split borrow）・view 削除・window ReleaseViews teardown 時の `library.remove` cleanup |
| `crates/synvelis-workspace/src/session.rs` | modified | `ProjectSession::notify_committed_change()` — `ProjectController` 直 commit（verified import）のみを対象に、revision が実際に進んだ場合だけ engine/control session へ通知（冪等 replay は通知しない） |
| `crates/synvelis-workspace/src/lib.rs` | modified | `pub mod library_ui` + 公開型 re-export |
| `crates/synvelis-workspace/Cargo.toml` | modified | `synvelis-library` path dep 追加 |
| `Cargo.lock` | modified | `synvelis-workspace` → `synvelis-library` dep edge（Ruviform rev 変更なし） |
| `crates/synvelis-app/src/app.rs` | modified | `LibraryImportServices{store,blobs,publisher}`・`library_import` field・`set_library_backend()`/`set_library_import()` injection seam・`library_backend` retain + `bind_library_backend()`（`project.new`/`open`/`reload` の workspace 再生成後に再 bind） |
| `crates/synvelis-app/src/lib.rs` | modified | `LibraryImportServices` export |
| `crates/synvelis-app/src/service.rs` | modified | 4 `OpInfo` + dispatch arms |
| `crates/synvelis-app/src/ui_ops.rs` | modified | `op_library_providers`/`op_library_visual`/`op_library_view`/`op_library_action` + `library_view_instance`/`library_unavailable` helpers・action 前後 revision 比較による `notify_committed_change()` 呼出 |
| `crates/synvelis-app/tests/common/mod.rs` | modified | `pub mod library` 登録 |
| `crates/synvelis-app/tests/common/library.rs` | added（440 行） | scripted `FakeProvider`（`AssetProvider` impl）・`app()`/`app_with()`/`app_with_import()`/`app_with_import_registry()` fixture（temp-dir `ImportStore` + `LocalBlobDir` + `StagedBlobPublisher`） |
| `crates/synvelis-app/tests/library_browser.rs` | added（777 行・16 test） | `syn.library.*` op surface の integration test |

### Op surface

| Op | Params | Result | 備考 |
|---|---|---|---|
| `syn.library.providers` | `{}` | `{providers:[ProviderWire]}` | backend 未 bind でも空 list（捏造しない）・view 不要 |
| `syn.library.visual` | `{action:"open"}` / `{action:"close",view_id}` | `{view_id:"view-N"}` / shared close teardown | `ViewType::AssetLibrary` を adopt |
| `syn.library.view` | `{view_id}` | `{view_id, providers, state: LibraryViewState, spec: ViewSpec}` | deterministic 投影・typed state verbatim |
| `syn.library.action` | `{view_id, action:"<tag>", ...fields}` | `{ok:true, view_id}` | `LibraryAction` に verbatim decode |

Action tag: `provider.select`・`browse.refresh`・`browse.scope`・
`browse.more`・`search.set`・`search.clear`・`entry.select`・
`entry.stat`・`entry.pin`・`preview.plan`・`preview.fetch`・
`preview.cancel`・`import.submit`・`import.rebase`・
`import.retry`・`import.dismiss`・`drag.begin`・`drag.drop`。
action-level failure は `state.fault`/typed surface state に
着地し op error にしない。malformed params・未知 tag・
未登録/wrong-kind view・unbound backend は typed
`ErrorBody`（`invalid_params`/`unavailable`）。

### Design decisions（詳細は `docs/impl/exp/` 参照）

- `LibraryAction` は internally-tagged serde enum
  （`{action:"<tag>",...}`）— wire params が variant field
  へ verbatim decode され、`deny_unknown_fields` で余分な
  key を拒否。action 名の string dispatch を介さない
  （exp E2）。
- wire DTO は `synvelis-library` の型付き値を verbatim で
  埋め込む — `AssetObservation`/`Availability`/`EntryFacts`/
  `ProvenanceRecord`/`ProviderCapabilities` の
  observed/inferred/unknown{reason}/stale{reason} と
  availability reason が flatten されず end-to-end で
  typed を保つ（exp E1）。
- `syn.library.*` op は service thread で同期実行
  （`syn.vcs.*` と同じ discipline）。preview fetch は
  read chunk 間で協調 cancel token を見るため、後続の
  `CallOutcome::Job` 化で mid-flight cancel を contract
  変更なしに載せられる（exp E3）。
- `LibraryViews` は model を lazy materialize する —
  backend bind 済みでも `syn.library.*` op/test が実際に
  要求した view にのみ model を作り、非 library view や
  backend 未 bind では `None` → typed `unavailable`。
  provider catalog を model 内に捏造しない。
- import は `with_library` の split borrow
  （`&mut LibraryViews` + `&mut ProjectSession`）で
  `LibraryImportPorts`（借用 `ImportStore`/`ProjectBlobStore`/
  `BlobPublisher`）と `ProjectController` を同時に渡す —
  `Arc<Mutex>` 共有を避け、唯一の project mutation 経路を
  expected revision CAS に限定する（exp E4）。
- `import.submit` は submit 前に provider `pin` を明示実行
  し `ImportIntent{import_id, expected_project_revision,
  source: PinnedAssetRef, target}` を構築 — ambient
  selection や live re-resolution を import source に
  しない。pin の `expected_size` が whole-object consent
  閾値超過なら `allow_full_object` 必須。
- provider 切替（`provider.select`）は browse/selection/
  preview surface を無効化する — cursor・pin・search は
  provider-bound であり、前 provider の state を残さない。
- spec の `import.submit` button は `import_id` を
  `(provider, locator)` の FNV-1a hash
  （`import_template_id`）で埋める — hash アルゴリズムを
  固定し provider 間衝突を避ける。同一 asset の再 submit
  は `reused` 冪等 replay になり、異なる asset が
  `IntegrityConflict` に衝突しない。driver は params を
  上書き可能。locator-targeted button（import.submit・
  drag.begin・preview.plan/fetch）は selection 存在時
  のみ投影される。
- verified import の project commit は
  `ProjectController::apply` を直接通るため、
  `ProjectSession::apply` が担う engine/control 通知を
  迂回する — `op_library_action` が action 前後の
  revision を比較し、実際に進んだ場合のみ
  `notify_committed_change()` を呼ぶ（冪等 replay・
  typed failure では通知しない）。
- `provider.select` は provider-bound state を全て
  破棄する — entries・cursor・coverage・search・
  selection・stat・pin・preview・drag ghost・scope・
  pending import intent・非 `CommittedLive` の import
  surface。`CommittedLive` receipt は project truth と
  して残る。`search.set` 失敗時は search mode に入らず
  既存 listing を browse label のまま保持し、
  `browse.refresh`/`browse.more` は search mode 中は
  `SearchQuery`（同一 cursor・scope・text）を継続する。

## Related Experiments

### Adopted

- `docs/impl/exp/alib-05-browser-import-ux.md` E1（wire DTO
  が library 型付き値を verbatim 埋め込み）— 採用。
- 同 E2（internally-tagged `LibraryAction` enum）— 採用。
- 同 E3（同期 op + cancel-token readiness）— 採用。
- 同 E4（`with_library` split borrow）— 採用。

### Rejected

- なし（実験は全て採用。代替案は各 E 節に記録）。

## Validation

実行環境・全コマンド・exit code・NotRun 一覧の正本は
`evidence/sv89/verification.md`。受入行→証拠写像は
`evidence/sv89/acceptance-ledger.md`。

| Command | Exit | Result |
|---|---|---|
| `mise run fmt` | 0 | pass — `cargo fmt --all` で整形適用 |
| `mise run check synvelis-workspace` | 0 | pass |
| `mise run check synvelis-app` | 0 | pass |
| `mise run lint synvelis-workspace` | 0 | pass・warning 0 |
| `mise run lint synvelis-app` | 0 | pass・warning 0 |
| `mise run test synvelis-workspace` | 0 | pass — `library_ui` unit 14 件（`search_mode_survives_refresh_and_more`/`preview_cancel_does_not_poison_later_plan` regression 含む）を含む全 suite green |
| `mise exec -- cargo test -p synvelis-app --test library_browser` | 0 | 16/16 pass |
| `mise run check-headless-deps` | 0 | pass — "no GUI framework deps in 12 headless crates" |
| `mise run check-app-native` | 0 | pass（`cargo check -p synvelis-app --features native --all-targets`。future-incompat warning は既存依存 `proc-macro-error2` 由来で本 change set と無関係） |
| `mise exec -- prek run --all-files` | 0 | pass（全 hook pass） |

## Risks

- `library_ui.rs` は view-model・action・wire DTO・ViewSpec
  projection・registry を単一モジュールに収める（約 3000
  行）。責務は型で分離済みだが、#91 の実 provider 配線・
  `syn.library.jobs` ledger・`CallOutcome::Job` 化で
  さらに拡張する際に分割を検討する余地がある。
- `import.submit` は `Io`/`Corrupt`/`PublishFailed`/
  `JobState` の `ImportError` を `ImportUiState::Failed
  {reason:"internal"}` に丸める（`IntegrityMismatch`/
  `CommitRejected`/`PublishIndeterminate`/`Provider`/
  `StaleRevision`/`IntegrityConflict` は typed）—
  stage manifest の `Recoverable` resume 等の細かい
  typed 回復は `syn.library.jobs` slice の責務。
- `ProviderContext` は `LibraryBackend` が principal のみ
  を持ち、budget/deadline/credential は未配線 — 実
  provider 配線（#91）で `LibraryBackend` 拡張が必要。
- preview は `provider.open_preview` 直読み（bounded・
  unverified）— `LibraryCache` 経由の preview（ALIB-03/#87）
  ではない。`Ready{cached:false}` 固定はこの制約を正直に
  反映している。
- 全 op が service thread 同期 — 長時間 fetch で thread を
  占有し得る。headless slice の既知制約（`syn.vcs.*` と
  同じ）で、job 化は contract 変更なしに後続可能。

## Follow-ups

- synvelis#114: 実 GPUI mount — `ViewSpec`/`LibraryViewState`
  を実 driver が消費する受入 gate。
- synvelis#91: 実 provider registry 配線 —
  `set_library_backend` へ実 `AssetProvider`
  （Git/WebDAV/Local/Project）を登録し、credential・
  network・fault matrix の実環境受入。
- synvelis#86: v5 real-data import・`inspect_imports` の
  stage-level manifest recovery を op surface へ接続。
- `syn.library.jobs` cross-view recovery ledger・
  `CallOutcome::Job` wrapping（mid-flight cancel の job
  thread 化）— 別 slice。
- `LibraryCache`-backed preview（ALIB-03/#87）接続。
