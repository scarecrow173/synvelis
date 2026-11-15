---
id: "EXP-alib05-browser-import-ux"
title: "ALIB-05 Browser/Import UX headless view-model + syn.library.* ops 設計実験メモ (synvelis#89)"
created: "2026-11-15"
updated: "2026-11-15"
---

# 実験記録: ALIB-05 Browser/Import UX headless スライス（synvelis#89）

## E1 browse/stat wire DTO は library の型付き値を verbatim で埋め込むか、flatten するか

**仮説**: `BrowseEntryWire`/`StatWire`/`ProviderWire` を
`synvelis-library` の型へどう写像するかで、wire 上の
「観測値の鮮度」と「availability の理由」が生きるか死ぬかが
決まる。`AssetObservation` は `observed`/`inferred`/
`unknown{reason}`/`stale{reason}`、`Availability` は
`available`/`missing{detail}`/`offline`/`unavailable{reason}`
を持つ — これらを `Option<u64>` や裸の tag 文字列へ flatten
すると、stale な byte 長が fresh な値に、unavailable の
理由が失われて「不明」と「未対応」が UI 上で区別不能になる。

- 調査: spec AL-R-04/05 と AL-T-R-02 は coverage・
  availability・observation の typed 区別を要求する。
  driver（Fake/Mock/将来の GPUI）が wire JSON をそのまま
  消費するため、projection 層で情報を落とすと driver 側で
  復元不能。`entry_wire_preserves_observation_and_
  availability_tags` はこの契約を regression test として
  固定するために追加した。
- 案 A（flatten — `byte_len: Option<u64>` + `"stale"` 等の
  並列 tag）: 実装は単純だが `unknown{reason}` と
  `stale{reason}` の reason payload を運べず、複数の
  表現規約が wire に増える。却下。
- 案 B（`AssetObservation<u64>`/`AssetObservation<
  ObservedAt>`/`Availability`/`EntryFacts`/
  `ProvenanceRecord`/`ProviderCapabilities` を serde で
  verbatim 埋め込み）: library 側の型がそのまま wire 契約に
  なり、tag・reason・provenance が end-to-end で保持される。
  view-model は値を解釈せず運ぶだけで、責務境界も明確。

**観察**: 案 B を採用。`entry_wire_preserves_observation_and_
availability_tags` で `Stale{value:4096}`・
`Unknown{RequiresFetch}`・`Unavailable{PermissionDenied}` が
serialize 後も `"observation":"stale"`/`"unknown"`・
`"state":"unavailable"` の tag を保つことを検証。

## E2 `LibraryAction` を internally-tagged enum にするか、action 文字列 + params struct の dispatch にするか

**仮説**: `syn.library.action` の params
`{view_id, action:"<tag>", ...fields}` をどう decode するか。
18 種の action があり、mutating 系は全て対象 locator を
明示する契約（ambient selection を暗黙 target にしない）。

- 調査: serde の internally-tagged enum（`#[serde(tag =
  "action")]`）は `{action:"entry.pin", locator:"..."}` を
  variant field へ直接 decode し、`deny_unknown_fields` で
  未知 key を拒否できる。action 名→handler の match は
  `LibraryViewModel::apply` の 1 箇所に集約される。
- 案 A（`action: String` + `params: Value` を op 側で
  手動 dispatch）: 各 action の field 検証を op 層に書く
  必要があり、型安全性と網羅性が失われる。却下。
- 案 B（`LibraryAction` enum + `#[serde(rename =
  "<tag>")]`）: wire がそのまま型付き action になり、
  malformed/未知 tag は serde error → op 境界の typed
  `invalid_params` に落ちる。action-level failure は
  `apply` が `state.fault`/surface state に着地させるため
  op error と混ざらない。

**観察**: 案 B を採用。`malformed_action_params_are_
invalid_params` で未知 tag・tag 欠落が `invalid_params`、
`malformed_and_implicit_actions_are_typed` で action-level
failure が `state.fault`（`no-provider`/`unknown-provider`/
`malformed`）に着地することを検証。唯一の例外として
`preview.fetch` の `locator` は `Option` で selection
fallback を持つ — spec button は常に locator を明示 emit
するため、ambient target 化は投影面では起きない。

## E3 `syn.library.*` op を同期実行にするか `CallOutcome::Job` で包むか

**仮説**: preview fetch と verified import は provider の
blocking read と disk/Project CAS を含み、service thread を
占有し得る。`syn.vcs.*` 系は同期 op の先例があるが、
import は長時間化し得る。

- 調査: `CallOutcome::Job` wrapping は mid-flight cancel・
  progress 通知のための機構だが、本 slice の要件は
  headless contract の固定であり、job 化は service 層の
  変更を伴う。preview read loop は既に協調
  `CancellationToken` を chunk 間で見るため、後から job
  thread 化しても contract は変わらない。
- 案 A（最初から `CallOutcome::Job`）: cancel・progress の
  UX 面は強いが、job registry・outcome polling・view への
  非同期結果反映まで本 slice の範囲を超える。却下。
- 案 B（同期 op + cancel token 配線済み）: `syn.vcs.*` と
  同じ discipline で slice を閉じ、`preview.cancel` は
  in-flight fetch を `Cancelled` に落とせる。長時間化の
  実害は headless/test では観測されず、job 化は後続 slice
  の明示的 follow-up とする。

**観察**: 案 B を採用。`preview_cancel_is_typed`・
`search_stat_pin_and_preview_flow` で同期経路の typed
state を検証。`evidence/sv89/verification.md` と IR の
Follow-ups に job 化を記録。

## E4 import に必要な `&mut LibraryViews` + `&mut ProjectSession` をどう同時に渡すか

**仮説**: `syn.library.action` の import 系
（`import.submit`/`rebase`/`retry`）は view model の
`&mut` と `ProjectController` の `&mut` を同じ呼び出しで
必要とする。`WorkspaceManager` の field 借用では 2 つの
`&mut` を同時に取れない。

- 調査: 既存の `with_vcs` は `(&VcsHistoryViews,
  &ProjectSession)` の共有借用だが、import は
  `ProjectController` への commit（唯一の project
  mutation 経路）を行うため `&mut` が必要。
- 案 A（`Arc<Mutex<LibraryViews>>` や model の共有化）:
  lock ordering・poison・view-local state の ownership が
  曖昧になり、split borrow の単純さを失う。却下。
- 案 B（`WorkspaceManager::with_library(f)` で
  `(&mut LibraryViews, &mut ProjectSession)` を closure
  に渡す）: field 分割借用を 1 箇所に閉じ込め、import
  ports（`LibraryImportPorts` は `ImportStore`/
  `ProjectBlobStore`/`BlobPublisher` を借用するだけ）と
  controller を同じ scope で組み立てられる。非 import
  action は ports/controller を `None` で呼べる。

**観察**: 案 B を採用。`op_library_action` が
`with_library` 経由で `model.apply(&action, ports,
Some(session.controller_mut()))` を呼び、
`import_commits_live_and_replays_idempotently` で実
`ImportStore`+`LocalBlobDir`+`StagedBlobPublisher`+
`ProjectController` を通した `CommittedLive`→idempotent
replay（`reused:true`）を検証。
