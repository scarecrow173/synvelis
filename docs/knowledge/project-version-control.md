---
id: "KNOW-LM6L85dtVJTtff8q6KRFe4"
type: "knowledge"
status: "draft"
lifecycle: "draft"
knowledge_kind: "architecture"
title: "プロジェクトバージョン管理"
created: "2026-11-19"
updated: "2026-11-19"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/7"
    - "https://github.com/scarecrow173/synvelis/issues/92"
    - "https://github.com/scarecrow173/synvelis/issues/93"
    - "https://github.com/scarecrow173/synvelis/issues/94"
    - "https://github.com/scarecrow173/synvelis/issues/95"
    - "https://github.com/scarecrow173/synvelis/issues/96"
    - "https://github.com/scarecrow173/synvelis/issues/97"
    - "https://github.com/scarecrow173/synvelis/issues/230"
    - "https://github.com/scarecrow173/synvelis/issues/335"
  informs: []
  explains: []
  supersedes: []
---

# プロジェクトバージョン管理

## 概要

Synvelis の VCS（Semantic Visual Diff）は、プロジェクトファイルの内容を immutable snapshot として固定し、semantic diff で変更を分類し、明示承認のもとで single-writer path 経由の apply または staged publish の `restore_as_copy` として適用する一連の機能群である。実装は VCS-01 snapshot resolver → VCS-02 diff engine → VCS-03 Git read provider → VCS-04 History/Visual Diff UI → VCS-05 Safe Apply/Restore → VCS-06 証拠 matrix の段階で進められ、各段で degraded/missing を silent Equal にしない・headless Pass を実機 Pass に昇格させないという fail-closed と verdict 誠実性の原則が一貫して適用された。Issue #7 の統合契約では docs-only slice として Case 被覆台帳が canonical 化された。

## 詳細

### Immutable Snapshot Resolver の単一 pipeline と read-only 境界

`synvelis-project` に VCS 非依存の additive module として snapshot resolver を実装した。5 つの locator 系統（Live / Saved HEAD / Recovery checkpoint / VCS staged / VCS committed）を、厳密 `SnapshotEnvelope` parse → 既存 migration ladder（`migration::migrate_to_current`）→ `validate_references` + `check_id_watermark`（load と同一 oracle を共有）→ exact-bytes SHA-256 digest → read-only projection の単一 pipeline で `ImmutableSnapshot` へ解決する（synvelis#92）。

`ReadOnlyProjectProjection` は `Arc<Project>` で mutable 経路を持たず、resolver は `&dyn ProjectStore` の read method のみを使い `InMemoryStore`/`DirectoryBundleStore` を変更しない。controller には `snapshot_id_watermark()` read-only accessor のみを追加し、save が stamp するのと同じ `generator.peek()` を返す。Single Writer/Undo/Redo/load/save の semantics は不変である（synvelis#92）。

SHA-256 は crate 内実装とした。`synvelis-project` は `sha2` に依存しておらず、タスク制約で本番依存追加が禁止されていたため manifest 変更なしで内製化し、FIPS 既知解答ベクタで unit test 済みである。`ContentDigest` は `"sha256:<64hex>"` の algorithm-tagged 文字列で、Git object ID（SHA-1/SHA-256 hex）とは別名前空間である（synvelis#92、#94）。

`VcsBytesPort` は `index_generation`/`read_file`/`read_blob` の read-only 契約のみを持ち write method を持たないため、worktree/index 変更は構造上不可能であり、VCS 側の意味確定はこの port 境界の外に置く設計である（synvelis#92）。

race/integrity semantics は locator 系統ごとに異なる。Saved は HEAD→bytes→HEAD→bytes の 4 read window で、HEAD 変化は `HeadConflict`・同名 bytes 変化は `SourceChanged`・`head_ref` pin 不一致も `HeadConflict` となる。Recovery は recovery-rooted store の checkpoint 名を 2 read 比較し saved HEAD を参照も変更もしない。VCS staged は `index_generation` を window 前後で比較（`IndexChanged`）し、locator pin との不一致も `IndexChanged`、index が運ばない project file/blob は `PartialStage` とする。VCS committed は `fixed_revision` pin で、absent blob は Failure ではなく `Missing` 記録（degraded）である（synvelis#92）。

asset `Absent`/`Unknown`・`ProcessorBinding::Unresolved`・probe なし opaque state（`Resolved`→`Unknown` 固定）は manifest/opaque_state に記録され `Partial`/`Degraded` になる。Equal 判定は resolver の責務外で VCS-02 の管轄であり、resolver は状態を正直に記録するだけである。duplicate ID は `BTreeMap` 構造上 JSON から生成不能のため、integrity failure は invalid reference / forged watermark / project-id mismatch / corrupt bytes の各経路で `InvalidSnapshot` に固定した。`SnapshotError` は 11 variant の明示 Failure、`SnapshotCancel` は cooperative cancel、`SnapshotLimits` は resource bound を提供する（synvelis#92）。

実 Git provider・実 OS filesystem atomicity・GUI・実 plugin host probe は VCS-03〜06 / Issue #97 系へ deferred である（synvelis#92）。

### Semantic Diff Engine の identity・tri-state・field 分類

`synvelis-project` に additive module として diff engine を実装した。`compare_snapshots` は 2 つの VCS-01 `ImmutableSnapshot` を入力に `projection.project()` のみを読む pure・non-RT エンジンである。SnapshotResolver / ProjectController / ProjectStore / VCS port / live transport / runtime engine は不変・非接触で、Source Project 変更・Git checkout は実施せず読み取り専用比較のみである（synvelis#93）。

identity = project ID + typed `ObjectKind` + typed stable ID。raw u128 が異 kind に跨る場合は `CrossKindId`、owner/lineage 参照 field の相違は `OwnerConflict`、project ID 不一致は `CrossProject` — いずれも entity diff を出さず `AmbiguousIdentity` issue + `Unknown` に落とす。`Equal` は changes 0・issues 0・両 snapshot `Complete` のときのみで、confirmed change があれば `Different`（issues 共存時は `completeness=Partial`）、changes 0 + issues ありは `Unknown(representative)` である。digest-equal fast path でも snapshot-level issue を必ず収集し partial snapshot を `Equal` に昇格させない。`lineage` は現行 schema に永続化経路がないため常に `None`（捏造しない。Issue #7 の将来 schema extension へ deferred）である（synvelis#93）。

`ChangeSetCache` は derived state のみ保持し project/media truth にはならず、comparison-key digest でのみ index する（capacity 既定 64 の LRU、`Arc<ChangeSet>` 共有）。`ChangeId` は comparison-key digest + domain + object + kind + field paths の deterministic 導出で、deterministic sort は domain→kind→stable ID→change kind→field→change ID である（synvelis#93）。

field 分類 policy が semantic kind を決める。clip anchor のみの差は `Moved`、extent/playback は `Modified`、source 差し替えは `Replaced`、processor chain 内順序は `Reordered`（chain）・chain 間移動は `Moved`（processor）、connection endpoint は `Reconnected`、edge 増減は `Added`/`Removed`、digest 差し替えは `Replaced`、automation region 編集は `Modified` である。clip-relative note は clip 移動に追従し note 差分を出さず、MIDI source の notes は子 `MidiNote` entity として global diff し owner source 相違は `AmbiguousIdentity` とする（synvelis#93）。

schema-version suppression: pre-v6 routing / pre-v7 role・time_scope / pre-v8 MASTER_FADER_RAW は差分を出さない suppression 対象である。SMF documents は v9 導入の keyed-synthetic として実体 diff し、pre-v9 absent = faithfully empty（suppression 対象外）である（synvelis#93）。

fixture 構築の規約: VCS-01 経路（`ProjectController` + `SnapshotResolver::capture_live`）を主経路とし、ops で生成不能な crafted state（identity collision・owner conflict・degraded manifest・旧 schema version）のみ直接 `ImmutableSnapshot` 構築を使う（synvelis#93）。

diff 分類は `DIFF_POLICY_VERSION` で版管理され、apply surface の whitelist はこの version に追従する。`ObjectKind`/`raw_kind_map` に列挙されない kind（variant_sets 等）は compare surface gap となり silent `Equal`/silent drop を起こす — これは VCS-06 で `Blocked` として実測記録された（synvelis#93、#96、#97）。

### Git Read Provider の 2 層構造と read-only 保証

provider-neutral 契約を `synvelis-project::vcs` に、実装を `synvelis-library::git::vcs` に配置した 2 層構造である。`GitVcsProvider` + `GitVcsConfig` が `VcsReadProvider` 全 method と `VcsBytesPort`（staged/committed locator の project file・asset blob read、index generation fingerprint）を実装する（synvelis#94）。

契約型は `FixedVcsRevision`（repository_id/object_format/commit_oid/project_root/context）・`RevisionSelector`（Head/Expr、一回だけ immutable commit に固定）・merge-aware `HistoryNode`/`HistoryPage`/`HistoryQuery`（bounded page・fingerprint-bound cursor）・`WorkingStatus`/`StagedSnapshot`/`IndexEntry`（stage 0–3・mode・gitlink）・`VcsCapabilities` optional feature 宣言・`VcsError`（missing/degraded/conflict を明示、silent Equal 禁止）・`VcsCancel` である（synvelis#94）。

local worktree binding は `rev-parse --show-toplevel/--git-dir/--git-common-dir/--show-object-format` probe で行い、`repository_id = git:sha256(git_common)`、`context` = canonical common dir として foreign binding を `ForeignBinding` で拒否する。`RevisionSelector::Head/Expr` は一回だけ immutable commit に固定し、history cursor は fingerprint bind で mutable ref の pin 解除を防ぐ（synvelis#94）。

全 git 呼出は fixed-argv plumbing（`rev-parse`/`ls-tree`/`cat-file`/`ls-files`/`check-attr`/`rev-list`/`status`）+ `GIT_OPTIONAL_LOCKS=0` + prompt/credential helper 無効で、index への意図しない書込（status refresh 等）を構造的に排除する。read-only oracle（index hash/status/HEAD/staged 前後一致）で repo 非破壊を実測固定し、session oracle（live revision/tracks 不変）も assert する（synvelis#94）。

LFS pointer は local git_common 配下の lfs objects のみ解決し pointer text を content として返さない（fail-safe は LFS 側の `Missing`+`Degraded`）。`sha256:` digest は bytes の SHA-256 一致時のみ `Present` である。remote LFS fetch・submodule 内容取得・rename similarity 検出（tree-level では delete+add）・first-parent-only opt-in は scope 外である（synvelis#94）。

adapter 小変更として context モジュールに `CancellationToken::from_shared`（`VcsCancel` bridge）、objects モジュールの `classify_kind` に `"not a git repository"` → `NotFound` 分類を追加した。所有境界の遵守として #88 Asset Provider（catalog/preview/import）は再実装せず、低レベル `GitRunner`/`GitRemoteUrl`/`parse_ls_tree_zl`/`parse_lfs_pointer`/`is_pointer_sized` のみ再利用した。Project History/Working Status/Asset Catalog は別 Port として分離である。Git object ID（SHA-1/SHA-256 hex）と project `ContentDigest`（`sha256:` prefix）は別名前空間として型・test 双方で固定し、比較しない契約とした（synvelis#94）。

実 OS permission matrix・real remote fetch・auth/credential 実経路・GC/objects 欠損の実 filesystem 条件・GUI/Visual Diff・RT 性能保証は headless fixture で検証不能のため Issue #97 へ `NotRun` として送る。`VcsCancel` は cooperative cancellation を全 operation に提供し、cancel は typed error として観測可能（silent hang しない）である（synvelis#94）。

### unborn HEAD の誤分類と stderr 分類経路の教訓（synvelis#335）

`git rev-parse --verify --quiet` は失敗時に stderr を出さず exit 1 で返る仕様のため、共有 `classify_kind` が stderr パターン全不合致 → exit code フォールバックで `TransientIo` → `VcsError::Unavailable` となり、unborn HEAD・解決不能 revision が typed absence（`RevisionGone`/`Ok(None)`）に写像されなかった。`head_oid` は `ProviderErrorKind::NotFound` のみを `Ok(None)` に写像する設計だったが `--quiet` 経路では `NotFound` に到達不能で、分類器が意味を確定できない silent failure を呼出側が作っていたのが根本原因である（synvelis#335）。

修正は呼出層（`GitVcsProvider` の `resolve_oid`/`head_oid`/`probe_workspace` 3 箇所）の `--quiet` 除去のみである。`--quiet` なしでは unborn HEAD・未知 oid・範囲外 peel の全てで exit 128 + stderr `fatal: Needed a single revision` を返し、既存 `classify_kind` の `NotFound` パターンに合致することを scratch repo で実測した（synvelis#335）。

棄却した案は二つある。一つは `classify_kind` で silent exit-1 を `NotFound` に倒す案で、mirror provider（ALIB-04）の stderr 由来分類契約に波及する blast radius を持ち Issue の境界条件に反する。もう一つは `GitRunner` への exit code/生 stderr 露出 API 追加で、runner の失敗表現を広げるより `--quiet` を外して既存診断経路を使う方が小さく正しいと判断した。共有 `classify_kind`/`GitRunner`/mirror provider・`synvelis-project` API は一切変更していない（blast radius ゼロを git 系テスト群の全件 pass で実測）である（synvelis#335）。

教訓は「stderr 駆動の失敗分類経路では `--quiet` を使わない」である。診断を抑止すると分類器が意味を確定できず、silent failure を呼出側が作ってしまう。意味確定は git 診断 + `classify_kind` に委ねるのが責務境界として一貫する。mirror provider（ALIB-04）は既に `--quiet` なしの同 idiom を使い git 系テスト群がその分類を assertion で固定済みだった — 正しい経路は確立済みで vcs provider 実装側だけが `--quiet` で潰していた（synvelis#335）。

発見経緯は VCS-04 backend wiring（synvelis#95）の production 配線で「既知制限」として記録され、0-commit repo で `syn.vcs.visual refresh` が `service_error`・view が `Failed` status になることを実測してから #335 として修正された。pre-fix fail / post-fix pass を実走で証拠化する手順が取られた（synvelis#95、#335）。git の将来の診断文変更で `NotFound` パターンを外れる可能性は残るが、回帰 test が unborn/未知 ref/範囲外 peel の 3 系を固定する。`probe_workspace` の head probe が unborn 以外の失敗を `head_commit: None` に畳む既存 semantics の再検討は別 slice 候補として範囲外記録である（synvelis#335）。

### History/Visual Diff UI の 3 層構成と backend wiring

VCS-04 は 3 層構成である。project 層に object-safe `VcsHistoryBackend` trait + `VcsHistoryAdapter`、workspace 層に per-view `VcsHistoryState`・stale 判定・共有 `ChangeSetCache`・read-model 投影、app 層に `VcsScreenState` + `history_screen_spec` + `syn.vcs.view`/`syn.vcs.visual` op を置く（synvelis#95）。

`VcsReadProvider` は `detect_workspace`（static method）を持ち object-safe でないため、adapter 用に別 trait `VcsHistoryBackend` を定義し app は `Box<dyn VcsHistoryBackend>` で保持する設計に確定した。`VcsHistoryAdapter` は provider/bytes port を単一 repository_id + project_path に bind し、history page・fixed revision 解決・revision relations・working status・index generation・committed/staged snapshot 解決を passthrough する。foreign repository_id の revision は resolve 前に拒否する（synvelis#95）。

`FixedSnapshotLocator::VcsCommitted` は `fixed_revision: String` のみで `FixedVcsRevision.context` を持たないため、pin は `EndpointPin::Committed{revision: FixedVcsRevision}` で full revision を保持する。`ImmutableSnapshot`/`ChangeSet` は `Serialize` でないため `VcsHistoryState` は session-only とし、persist schema v2 を不変に保つ。`op_vcs_visual` の pin 解決は provider I/O を伴うため non-RT op 面で実行する（RT thread では呼ばない、既存 op と同じ境界）である（synvelis#95）。

stale（Live revision 前進・index generation 変動・Saved HEAD 移動）が Fresh と区別され、Committed pin は immutable のままである。`VcsHistory` view の state は Dock move/detach/reattach/2-window で `ViewState` と同規則で生存し、ReleaseViews で除去される（synvelis#95）。

production backend wiring: `project.new`/`open` が bundle を含む git workspace を `GitVcsProvider::detect_workspace`/`open` で検出し、`VcsHistoryAdapter` 経由で project root 単位に bind/re-bind する（`project_reload` は open 経由で同じ re-bind を通る）。bind は常に再検出（冒頭で `vcs_backend = None` にクリア）し、非 repo・検出失敗・bind 失敗は `None` → `NoBackend` を正直に返す。失敗は closure に集約して `tracing::warn!` のみで project new/open を失敗させない（infallible-by-contract）。fake backend は bind せず、`set_vcs_backend` は fake backend を注入する test 経路として温存する（production 経路は `bind_vcs_backend` のみ）（synvelis#95）。

VCS-facing project file `main.json` 規約: `DirectoryBundleStore` の revision 連動名 snapshot ファイル + `HEAD` pointer の layout は安定 path を持たないため、bundle 直下に HEAD snapshot bytes の mirror を置く。mirror は tmp+`fs::rename` で publish（派生物で crash 一貫性要件が store 本体より弱く次回 refresh で修復される）。非 repo bundle にも書かれる冗長 mirror だが「常に HEAD と一致」の不変条件が単純で、VCS 化時点で既に最新 mirror が存在する利点を取った。`project_root` が `""`（bundle = worktree root）なら bundle 直下の `main.json`、それ以外は project_root 配下の `main.json` である（synvelis#95）。

native mount/dock 配線: 静的 `dock_view_info`（`view-1`/`view-2` のみ既知）では `syn.vcs.visual{open}` が mint する動的 `view-{n}` を解決不能だったため、registry descriptor（kind/title 保持）を fallback とする `dock_domain_view` を ui_ops モジュール 1 箇所に集約し、`view-{n}` 形で静的表にも registry にも無い id は fail closed とした（「UI attached だが dock 未登録」の半端状態を防ぐ）。`syn.view.attach` result に `dock_domain: bool` を載せ、shell は `HashSet<String>` で動的 dock-domain id を追跡する — shell が workspace registry を直接読まない driver-neutral 境界を維持する（synvelis#95）。

state publication の集約: `main_screen_cell` と同型の hand-out cell（view ごとの `VcsScreenState` map）を `SynvelisApp` に追加し、全 op の post-op tail（`publish_project_length`）から `publish_vcs_screens` を呼ぶ。これにより VCS op 後と project 変更（Live endpoint を stale にする commit 等）の双方で mounted spec が追従し、「op 後に古い投影が残る」経路を作らない。publish は登録済み view のみ再投影し close 済み entry を落とす（synvelis#95）。

epoch 強制: `mount_main_view`/`mount_secondary_view` は固定 root id のため採用済み `ViewInstanceId` の VCS view には使えない。`mount_screen_view` ヘルパーを抽出し session-owned `AttachEpochAllocator` を共有する mount helper を追加した。呼出側が epoch を渡せない形にすることで fresh mint を強制し stale command namespace の衝突を防ぐ（RF387 fresh-host 契約）。`history_screen_spec` の title は `String` 化した（registry descriptor が owned title を持つため `DockViewInfo.title` の `&'static str`→`String` 変更は全 constructor 更新が必要で compile で網羅）。`syn.vcs.visual{open,close}` の `view-{n}` は単調 mint/解放で ID 再利用なしである（synvelis#95）。

gpui-test 構成: `ViewHost::new(view, allocator.mint())` → `host.mount(spec)` → `driver.create_view_window(WindowSpec::default(), ViewMount{host,theme:None})` → `gui.settle()`。semantic port で component resolve・select/invoke 拒否（read-only）・detach・2 window remount を検証する。`SYNVELIS_VCS_HISTORY` gate は native bin の起動時 env で、test 側は `set_vcs_backend` 注入 + `syn.vcs.visual{open}` op 経由で同等経路を辿る。実 OS GUI/IME/pixel/plugin dock は `ScenarioVerdict::Unsupported` として記録し simulated pass を昇格させない（synvelis#95）。

Windows verbatim path の落とし穴: `TempTree` canonicalize の `\\?\` prefix は git CLI 引数に使えないため、fixture は `plain_path` で非 verbatim spelling に戻す（library fixture と同じ対処）。`project.new`/`open` 毎に git subprocess が追加で走る（detect + open の probe、rev-parse probe ×2）が、non-RT 境界の project 操作であり許容範囲と判断した。実 GPUI Window・実 OS window/dock・IME/keyboard/a11y・実機 2Window は `NotRun`（#114 gate・VCS-UI-05）で headless Pass を実 GUI Pass と偽装しない（synvelis#95）。

### Safe Apply の guard・journal・transplant 規約

`apply_selected` は明示承認済みの `ChangeSet` subset を `ProjectController` の single-writer path で適用し exactly 1 undo step を commit する。approval・`ProjectRevision` CAS・recomputed target fingerprint で guard し、content-keyed `ApplyJournal`（memory/dir 実装）で retry・ACK-gap・crash recovery を idempotent にする。journal `Pending` は prior record 検査後にのみ arm する。`Applied` は stored receipt を replay、`Pending` は live-state reconcile（`Replayed` vs fresh attempt）を行う（synvelis#96）。

absent entity は JSON `null` の両側符号化で fingerprint 可能にし（`Added` 対象が live 不在でも fingerprintable）、presence rules は planner 側に残す。fingerprint object set は declared selection + target object から plan より先に構成する — これにより live==after でも ACK-gap reconciliation が機能する（synvelis#96）。

`Removed` changes は全 kind で `Unsupported`（cascade removal deferred）で、他の unsupported kind/change/field も fail-closed である。entity transplant は whole-entity 単位で、`entity_value`（fingerprint 用の canonical 値）と `write_entity`（typed write）の対で実装する。apply は `ApplySelectedOp` 単一 `Operation` を scratch-clone 経由で適用し、`project.monitoring` 等の payload は `_` arm で素通しして undo/redo・`validate_references` oracle に自動的に乗る（synvelis#96）。

### Restore-as-copy の staged publish

`restore_as_copy` は fixed `ImmutableSnapshot` を sibling staging directory に materialize → staged bytes を全 verify → same-directory rename で publish する。source project・Saved HEAD・Git index・working tree は一切触れない（synvelis#96）。

staging manifest は `Started → Materialized → Validated → Published` の stage machine で、rename 後・manifest `Published` 前の crash window は dest manifest `Validated` 検出で bookkeeping 完了 or replay する。restore lineage は receipt-only で schema v11 を不要と spec で決定した。dir fsync 非対応 platform は `DirSyncEvidence::Unsupported` を receipt に正直記録し gate にしない（synvelis#96）。

`quick_bundle_check` on post-rename reconcile path は envelope のみ verify で全 blob digest は pre-publish の full readback に依存する既知制限である。effects reports は group booleans を `untouched`/`source`/`git`/`copy` sub-structs に分けて `struct_excessive_bools` を suppression なしで満たす（synvelis#96）。

restore の destination safety: unsafe destinations・existing destination・low disk・cancelled は fail-closed で、staging manifest の stage machine が crash window を recover する。`restore_as_copy` は `project.plugin_instances` を verbatim 保持する（watermark・全 `PluginInstanceId`/`PluginRef` 値を resolved/unresolved processor 問わず保持）。fork されるのは project ID のみで、これが唯一の意図的な identity 再割当である（synvelis#96、#97）。

### apply surface 拡張の規約（monitoring・plugin instance）

apply surface の whitelist は `DIFF_POLICY_VERSION` で管理され、bump（2→3、#324 plugin_instances diff）のたびに apply surface の mirror helper（`apply_surface_kind`）を `supported_kind` 全 kind へ同期する必要がある（synvelis#96、#97）。

monitoring domain（`DIFF_POLICY_VERSION = 2` で diff 分類済みの `TrackMonitorDecl`/`PunchMarks` を whitelist 追加、`Added`/`Modified` のみ）: `TrackMonitorDecl` は宣言が id を持たず `stable_id` が `TrackId` 空間を借用する keyed-synthetic view のため `keyed_insert!` 非使用の専用 arm（`stable_id` → `TrackId::from_tagged` → `monitoring.tracks` 直接操作、parse 失敗は fail-closed）とした。decl は id 非保持のため `max_new_id` に寄与しない — 対象 track 自体が同 apply で `Added` なら `Track` write の `keyed_insert!` が reseed を担う。`PunchMarks` は declaration-presence 規則（`punch_in`/`punch_out` いずれか `Some` = 宣言あり、全 `None` = absent）を entity view に適用し、diff classifier（`diff_punch_marks`）と同一規則で fingerprint の absent=`null` encoding と `Added`/`Removed` 分類の presence を一致させた。規則の正本は diff 側で、2 箇所化の drift リスクは presence 遷移 fixture で固定する。`Removed` は monitoring kind でも許可せず plan 層の既存拒否に委ねた（synvelis#96）。

monitoring domain の所属: `ObjectKind::TrackMonitorDecl` と `PunchMarks` は `ComparisonDomain::Recording` に属し、diff add/modify/clear と punch-mark 宣言 presence 遷移がテスト対象。partial/inverted 宣言は range を捏造しない（synvelis#96、#97）。

plugin-instance apply（`DIFF_POLICY_VERSION = 3` で diff 分類済みの `PluginInstance` を whitelist 追加、`Added`/`Replaced`）: persisted `PluginInstanceIdentity`（issued `PluginInstanceId` + pinned `PluginRef`）は after snapshot から byte-faithful transplant（remap/再発行しない）。理由は、issued id は authored state であり、apply 後 fingerprint が after と一致することで CAS 検証と ACK-gap replay の reconcile が機能する — remap すると post-apply fingerprint が after と乖離し replay 判定を壊く。instance `watermark` は `max(live, after)` で単調 reseed し never-reissued 保証を保つ。`PluginInstanceId` は別 u64 domain のため entity `id_watermark`（u128）の `max_new_id` に畳み込まない。entry↔binding 整合・id range・一意性は write 後の `validate_references` → `PluginInstanceIdentities::validate_against` oracle に委譲し、plan 層への追加 preflight は YAGNI で見送った（`Processor` write が `plugin_instances.sync_processor` を呼ぶため Processor + PluginInstance 同時 selection では write 順に依らず最終状態が after の verbatim identity に収束する）（synvelis#96）。

`Removed` changes は cascade removal deferred で全 kind `Unsupported` — entity 層に Removed write を実装せず plan 層の既存拒否に委ねる設計。monitoring/plugin-instance でも同じく許可しない（synvelis#96）。clippy 対応の規約として、`write_entity` が `too_many_lines` (104/100) を超えたため `write_plugin_instance` helper へ抽出し `#[allow]` による抑制は行わない（契約の禁止事項）である（synvelis#96）。

### 証拠 matrix と verdict の誠実な記録

VCS-06 は test-only slice 群（プロダクションコード変更なし・既存 fixture/helper と公開 API の外部観測のみを oracle にする）で matrix の未消化行を補完した: monitoring-domain matrix（diff 2・apply 1・restore 1）、UI headless matrix（4 tests）、real-git/FS fault（real-git 4 + Windows FS fault 5）、variant-domain matrix（3 tests）、evidence completion（RESTORE-08 fixture + COMPAT CI 記録）。per-domain matrix file の先例に倣い domain ごとに 1 file へ束ねる構成である（synvelis#97）。

honest recording 原則: `Equal`/`Complete`・silent drop・`Validation` 拒否は観測値をそのまま pin し、ledger verdict（Pass/Pass*/NotRun/Unsupported/Blocked/Fail）を実測に一致させる。silent `Equal`/silent drop/restore 拒否を Pass に繰り上げず、headless Pass を実機受入に換算しない。variant fixture は「将来 #457/#458 で実装が入ると assert が fail し ledger 更新が必要になる」tripwire として意図的に機能させる（synvelis#97）。

variant_sets（schema v14、synvelis#278 CB-01）の実測 gap: `ObjectKind` に variant kind が無く `raw_kind_map` 未列挙のため compare surface gap — fork/adopt のみの authored op 差分が `Equal`/`Complete`・0 changes になる（`identity.digest` 差と `variant_sets` map の実差を assert し AA fast path でないことを確認）。mixed diff（track rename + variant 差分）では variant 差分が silent drop する。`restore_as_copy` は `fork_project` が `project_id` のみ再採番し nested `SnapshotIdentity.project_id` が source のまま残るため `validate_set_shape` が `origin belongs to a different project` で `RestoreError::Validation` fail-closed（publish 前・silent corruption なし）である。修復は #458 の設計判断（nested identity 再採番 vs validate 規則見直し — `SnapshotIdentity` が「どの snapshot を指すか」の参照なら fork 後も source を指し続けるのが意味的に正しい可能性）である（synvelis#97）。

ledger verdict は VCS-DIFF-11=`Blocked`・VCS-APPLY-10=`Blocked`（diff が variant change を emit しないため apply 対象不存在、fixture で apply を実行せず honest 記録）・VCS-RESTORE-10=`Fail` で、follow-up Issue #457（variant diff adapter — `ObjectKind` variant kind・`raw_kind_map`・`DIFF_POLICY_VERSION`・apply surface 設計）・#458（restore defect）を起票した（synvelis#97）。

apply の実測 fail 点は fingerprint CAS 段であり、error の `change` field は選択した change kind ではなく常に `Modified` を報告する（`plan_changes` の `change_supported` に届かないため）。assert はこの実測値に合わせ variant・kind・detail を個別に固定して偽装を防いだ。monitor-only changeset の apply fixture は asset/processor 無しで構成し `Complete`/`Different` を満たすようにした — でなければ `resolve_verified` の `NotApplicable` で fingerprint 段に到達しない。staged fixture は asset 未登録 snapshot を指す（asset 入りだと staged resolve が `PartialStage` で失敗し pin 自体が立たない）（synvelis#97）。

Windows FS fault の設計: dir read-only は Windows では配下ファイルの作成/置換を阻害しないため信頼できる fault 境界にならず、dir 内ファイルを対象に share-mode hold（`share_mode(0)`/`share_mode(0x1)`）と file read-only で staging manifest・staged HEAD・journal record を直接 lock する。read-only 復元は `set_readonly(false)` ではなく fault 前に捕捉した `Permissions` を `set_permissions` で戻す（`permissions_set_readonly_false` は Unix で world-writable 化するため clippy 対象、`#[allow]` 不使用）。`#[cfg(windows)]` guard で POSIX compile から除外し lane を混ぜない（synvelis#97）。

real-git fixture の教訓: bare repo は `git init --bare` 直後に `symbolic-ref HEAD refs/heads/main` が必要 — platform 既定 branch（master）との不一致で clone が unborn HEAD・空 checkout になるのを初回 test 実行で実測して修正した。pruned revision の検証は `reflog expire --expire-unreachable=now --all` を gc 前に実行しないと reflog が object を保持して `cat-file -e` が成功してしまうため、`cat-file -e` 失敗を precondition assert して「真に除去された」ことを測る。`file://` remote は local transport であり認証付き実 remote lane を Pass にしない境界を test コメント・ledger・spec の 3 箇所で維持した（synvelis#97）。

CI 証拠の扱い: CI workflow は #145 で `pull_request` trigger を撤去済みの `workflow_dispatch` 専用のため PR head に check が無い（PR head CI は NotRun by design）。COMPAT 証拠は `gh run list`/`view`/`api` の実測 snapshot とし推測補完しない。run 43428177865（main `7c206289`、manual workflow_dispatch・actor `scarecrow173`）全 leg success・vcs06 全 6 binary が ubuntu/windows/macos/msrv/rt-verify で実行・pass を記録した。windows verify の `synvelis-library --test webdav` MockServer TransientIo failure（os error 10053/10054）は VCS-06 非関連の library lane 問題として区別し、flake/残存 defect の裁定は記録のみに留めた。COMPAT-03 の錨付け替えとして、独立 CI gate の #83 が Closed（2026-11-04）になったため、COMPAT-03 の錨を現行 CI workflow の verify/full lane（`evidence` pr-mode gate + `ci-summary` full-mode gate、定義の正本は CI summary ツール）へ明示的に付け替えた。required leg（pr-mode evidence gate 対象: verify ubuntu/windows・rt-verify・gw10-verify・msrv-verify・ui-verify・dep-guard）と observed leg（verify-macos・gw10-verify-macos・ui-verify-macos — pr-mode では observed、full-mode では required）を区別して記録する。CI leg の pass/fail は workflow 実行の証拠であり実機受入に換算しない（synvelis#97）。

`op_vcs_view` は `blobs: None` で `VcsSession` を構築する既存契約のため live 側 asset は常に `Unknown` となり `UnresolvedSource` が出る — 同一 project 内容でも equality は fail closed になることを UI matrix で固定した。no-backend の `EndpointFreshness::Unknown` 経路は同じ `vcs_history_view` 投影境界を `VcsSession{backend: None}` で直接観測した（`op_vcs_view` は常に attach 済み backend を持つため）。`index_probe_fail` は `Arc<AtomicBool>` を test と provider で共有し、pin 解決**後**に立てて freshness probe のみを失敗させる（adapter が provider を所有するため test からは flag 経由でのみ到達可能）（synvelis#97）。

fixture 構築の補足: variant matrix は `snap_manual` helper で crafted `ImmutableSnapshot` を直接構築し、fork/adopt のみ差分・mixed diff（track rename + variant 差分）・restore の 3 系を pin し、`identity.digest` 差と `variant_sets` map の実差を assert して AA fast path でないことを確認する。plugin-instance matrix は resolved processor 2 件（`vendor.dist`/`1.0.0`/rev3・`vendor.eq`/`2.1.0`/rev7）+ unresolved 1 件の project を `restore_as_copy` し、publish 済み `SnapshotEnvelope` の `project.plugin_instances` が source と `PartialEq` 全 field 一致することを pin する。UI headless matrix の 4 test は異 project_id の committed 端点 → `overall:"unknown"` + issues（`equal`/0-change に潰れない）、asset `Absent` → `Partial` + issue 行、staged freshness `Unknown` 経路 ×2（no-backend の `EndpointFreshness::Unknown` を `VcsSession{backend: None}` で直接観測）である。monitoring matrix の test 構成は diff 2（add/modify/clear・punch-mark presence 遷移）・apply 1・restore 1 で、apply surface 拡張後は 6 tests に再構成され、`Partial`/inverted punch 宣言は range を捏造しないことを pin する。real-git matrix の test 構成は packed/GC・pruned RevisionGone・file:// remote・linked worktree の 4 系 + FS fault 5 系で、git 不在時は honest skip する。variant matrix の test 構成は diff gap pin 2 + restore fail-closed 1 の 3 tests、plugin-instance matrix の test 構成は apply 8 tests（`Added`/`Replaced`・watermark reseed・byte-faithful transplant・fail-closed）+ restore-as-copy verbatim 1 test（RESTORE-08）である（synvelis#97、#96）。

### VCS Integration の統合契約

Issue #7（VCS Integration / Semantic Visual Diff）の統合契約を docs-only slice として canonical 化した: spec/design/plan/task/test-spec/impl-ir/impl-exp の 7 文書 + sv7 evidence 領域の Case 被覆台帳と検証記録。コード・CI・本番 crate・既存 slice spec・既存 evidence ledger は一切変更していない。実装 Owner（#92〜96）と統合 Gate（#97）の責務境界を維持し、既存 vcs-01..06 spec を正本として参照する統合契約のみを追加した（synvelis#7）。

verdict は `vcs-06-fixture-matrix-ledger` の実測を転記のみとし、集計・再評価・昇格を行わない。`Pass*` は persisted/headless 層のみの Pass、`NotRun`/`Unsupported`/`Blocked` は理由付きで維持する。Issue #7 の 14 受入条件 → Case family の写像表を ledger 冒頭に置き、spec 受入条件の各行がどの Case/証拠/設計記録に対応するかを一意に追跡可能にした（synvelis#7）。

8 view 採否は契約レベル記録である。Issue #7 §6 の HTML mockup は chat 添付で repo 未収録（Issue が「GitHub リポジトリへの HTML 本体の追加は別作業」と明記）のため pixel レビューは実行不能。採否判定は Issue 本文の画面構成と既存 slice spec の確定済み view mode（SideBySide/ChangeFocus 採用、Overlay/Evolution Map/Rendered Audio Diff 延期、`plugin_instances` diff は #324 系）を根拠に記録した。Clip Detail は「条件付き採用」（独立 view mount は後続 slice）、Plugin State は「未採用（現時点）」、Audio は「延期」で、これは実機 UI レビューの代替ではなく未実施を Deferred として明示する記録である（synvelis#7）。

Issue #7 本体の統合受入判定は #97 Owner の責務で、本 slice は証拠を提供するのみで Issue を Close しない。実 GUI/IME/2Window/pixel/plugin dock 受入は #114 gate、実 remote/auth/GC・他 provider・OS matrix・consumer precision は #97 REAL lanes / #219/#294、mockup HTML のリポジトリ収録と pixel レビューは Issue #7 が別作業と明記、Clip Detail 独立 mount・Plugin State view・Rendered Audio Diff は後続 slice、`plugin_instances` diff 可視化は #324 系である（synvelis#7）。

### 検証・文書運用の規約

canonical 検証 = mise lanes: `fmt-check`/`check`/`lint`/`test`/`test-rt`/`test-msrv`/`build` + app-native lanes（`check-app-native`/`lint-app-native`/`build-app-native`）+ gpui feature gate lanes（`check-app-gpui`/`lint-app-gpui`/`test-app-gpui` — gpui-test は default features では compile されない）。直接 `cargo test`/`clippy`/`fmt` の実行は「補助証拠（canonical = mise）」として明記する運用で、全 slice で Windows・pinned rustc 1.99.0（MSRV lane は rust 1.89.0）で実測した（synvelis#92〜97）。

`test-msrv` の扱いに揺れがある。多くの slice は `mise run test-msrv`（rust 1.89.0 で workspace test）を実走し Pass を記録する一方、vcs-05-plugin-instance-apply と vcs-06-variant-matrix では「ポリシー文書 §5 により MSRV 別再コンパイルは Daily Full CI へ委譲」として NotRun 記録した（後者は「MSRV レーンは通常開発では行わずデイリーCI（daily-ci → CI workflow）で行う」というユーザー指定を根拠に必須検証から除外）。時期により記述が変わっており、ポリシー文書 §5 の現行運用を確認する必要がある（synvelis#96、#97）。

review-protocol は Reviewer A（spec 適合）/ Reviewer B（敵対的正当性）の分離 pass → Validator → Judge の順で実施し、CONFIRMED 指摘の修正・REJECTED の理由記録・verdict（ACCEPT / ACCEPT WITH NON-BLOCKING NOTES）を残す。worker 契約として、review-protocol のレビューで blocker なしをユーザー承認局面の承認として扱う（plan-doc の design gate「approved design 必須」は既存 approved design 群で充足される）。自分で作った design を `approved` にした点は review-protocol での事後検証対象とした（synvelis#96、#97、#7）。

doc chain 規約: spec → design → plan → task/test-spec → impl-ir/impl-exp の relations を full 形式で揃える。旧 compact 形式（`source`+`verified_by`+`implemented_by` のみ）は audit が要求する `implements`/`derives-from`/`verifies`/`verified-by`（kebab）key の欠落で `plan-missing-test-spec-evidence`・`traceability-missing-upstream`・`test-spec-missing-verifies` の blocking finding を生じるため、vcs-06 チェーンと同じ full relations 形式へ統一した。`defers` の自由記述は `broken-relation-link` finding を生むため frontmatter に入れず Follow-ups 節に記録する。test-spec の `implemented-by`/`verified-by` は指す対象（document）が存在しないため空のままにし、`verifies` → spec+design と `derives-from`/`depends-on` で実態を表現する。legacy artifact ID の `invalid-id-prefix` finding は repo 全体の migrate 対象であり ID 変更は relation 参照更新を伴う repo-wide migration の責務として据え置く（synvelis#95、#7）。

doc tooling 運用: spec/design/plan/task/test-spec の doc 生成スクリプトは `--name`/`--dir` で指定ファイル名どおりに生成し opaque ID（`SPEC-034X…` 等）をそのまま使う。impl record/experiment log の生成スクリプトには `--name` がなく生成ファイル名が指定パスと一致しないため `mv` で改名する（ID は front matter 内で不変）。event log（JSONL）が要求されない場合は markdown experiment log 先例に倣い手書きする。doc index メンテナンスタスクは preview-first（`index plan` で planHash 確認 → `--expect` 付き apply）で index を手編集しない。impl index 同期ツール（`--kind ir|exp`）・doc status lint・impl record/experiment log audit・route graph（`--focus`）で検査する。doc graph の pre-existing broken relations（legacy `ISSUE-*`/`SPEC-*` token が現行 graph で解決不能、route graph は focus なしで `blocked`/`artifact-graph`）は repo 全体の既存状態であり、本スライス起因の新規 broken relation のみを修正対象とし既存分は記録に留める方針である（synvelis#7）。

CI 運用: CI workflow は `pull_request` trigger を持たない `workflow_dispatch` 専用（#145 で撤去済み）で、PR head CI は NotRun by design。CI 証拠は `gh run list`/`view`/`api` の実測 snapshot として COMPAT 証拠レポートに記録し、required（pr-mode evidence gate 対象 leg）/observed（macOS 系 leg）を区別する。lane 分類の正本は CI workflow 定義 + CI summary ツールである（synvelis#97）。

fixture 規約: git fixture は実 git CLI で temp worktree を構築し（SHA-1/SHA-256 init・commit/branch/tag/merge/conflict/gitlink/LFS pointer+手置き object）、git 不在時は honest skip、hooksPath/lfs/gpgsign を neutralize して決定的にする。fake provider/backend fixture は共通 test fixture 群に抽出し view_id パラメータ化 helper と authority snapshot helper を持たせる。`snap_manual` 系 helper は `ProjectRevision(0)`・`SnapshotOrigin::Live` の合成 identity を生成するが、計測対象は snapshot 生成経路ではなく差分/restore の観測契約であるため契約に影響しない（synvelis#94、#95、#96、#97）。

read-only oracle パターン: resolve/diff/apply/restore 前後で controller revision/undo/saved・store HEAD/bytes・VCS port call journal・live engine builds が不変であることを assert する検証構造を全 slice で再利用した（synvelis#92〜97）。

Windows 環境のハマりどころ: `TempTree` canonicalize の `\\?\` verbatim prefix は git CLI 引数に使えない → `plain_path` で非 verbatim spelling に戻す。Windows 並列 link 時の `LNK1102`（メモリ不足）は一過性で再実行で exit 0、コード欠陥ではない（TIM-02 記録と同型の環境要因）。`proc-macro-error2 v2.0.1` の future-incompat warning は依存由来で本変更と無関係（warning のみ・失敗なし）。`permissions_set_readonly_false` は Unix で world-writable 化するため捕捉済み `Permissions` の復元で解消する（synvelis#95、#97）。

### 文書状態と食い違いの扱い

各 slice は spec → design → plan → task/test-spec → impl-ir/impl-exp の docs chain で構成される。plan/task/test-spec の多くは `approved`/`in-progress`/`draft` のまま残り、実装完了は IR の `completed` と実測で判定する。計画段階の記述を実績として扱わないことが運用の前提である（synvelis#92〜97）。

具体的な齟齬として、vcs-01 の task1（status `in-progress`・Done When/Verification 全て未チェック）と plan（Implementation Sequence が未チェックのまま、status `approved`）、vcs-05-plugin-instance-apply の task（`in-progress`・全 task 未チェック）と plan（`approved`・sequence 未チェック）、vcs-06-fixture-fault-matrix task（`in-progress`・「push → PR 作成」が未チェック）は、対応する IR が `completed` で実測済みだが task/plan 側が未更新のまま残っている。一方 vcs-04 history は merge 後の doc closeout slice で task `done` 化・relations 充填の先例があり、vcs-05-monitoring-apply-surface-task は `done` だが plan の sequence checkbox は未チェックのまま、という混在状態である。IR の実績を正とし、task/plan の未更新は文書運用の遅延として扱うのが妥当である（synvelis#92、#95、#96、#97）。

vcs-04 history 系では merge 後の doc closeout slice で task `done` 化・relations 充填の先例があり、実測件数は dispatch spec 記載（12/8 件）ではなく実ファイルの `#[test]` カウントと `mise run test` 出力で再確認した値（workspace 13・app 9）を正直に記録した。vcs-05-monitoring-apply-surface 系は task は `done` だが plan の sequence checkbox は未チェックのままで、前セッションの未コミット変更を引き継いで検証・commit・merge した実績がある。vcs-05-plugin-instance-apply 系は task・plan とも未更新のままだが IR は `completed`（9 tests all Pass）である。vcs-06 系は fixture fault matrix task が `in-progress`（push/PR 未完）のままだが後続 slice がその ledger 行追加を前提に作業しており実態とのずれがあり、variant matrix task は `done` だが evidence には意図的な `Blocked`/`Fail` が含まれる — task 完了と証拠 verdict は別軸である。vcs-integration 系は integration task が `done`、integration plan は `in-progress` のまま（docs-only slice の運用方針）である（synvelis#95、#96、#97、#7）。

Git 操作の並行 worker 運用: 複数 slice が fixture matrix ledger を共有するため、占有中は編集せず PR 本文の「未対応の作業」で Coordinator に統合依頼する運用が取られた（evidence completion slice）。`origin/main` merge は各 slice で実施し conflict なしの実績が多いが、`DIFF_POLICY_VERSION` の bump（2→3、#324 plugin_instances diff）を含む merge では apply surface の whitelist mirror helper（`apply_surface_kind`）を `supported_kind` 全 kind へ同期する必要があった。前セッションの未コミット変更を引き継ぐ運用実績として、vcs-05-monitoring-apply-surface では前セッションの未コミット変更（entity 層・module 層・両 test file）を引き継ぎ、compile・test で実測確認後に commit・merge・doc/ledger 更新を行った。`git merge origin/main` で #324（`ObjectKind::PluginInstance` 追加・`DIFF_POLICY_VERSION` 2→3）を conflict なしで取り込み、apply surface は `PluginInstance` 非対応のまま（#324 の fail-closed 設計判断と整合）だった（synvelis#96、#97）。

`mise run sync-ui-deps`: Cargo の lockfile が記録する ruviform 解決 rev が変わる変更（pin 更新・`cargo update`・ブランチ切替）を取り込んだ後は ruviform vendor ディレクトリ（gitignore された生成物で worktree include 設定により worktree へコピー）を lock rev へ再同期する必要がある。`check-ruviform-rev` の REV 不一致はこの stale vendor を検出する fail-closed であり、復旧は `sync-ui-deps` の再実行である（synvelis#230）（synvelis#97 の UI matrix slice で ruviform pin `b02942c2` へ同期した実績あり）。

status 方針の運用: spec/design/test-spec は `approved`（Issue が承認済み契約であり review-protocol が承認ゲートを代替）、plan は `in-progress`→完了時更新、task は `in-progress`→`done`、impl-ir は `in-progress`→`completed`。docs chain の ID は `new_*` script 採番の opaque ID をそのまま使用し手動での ID 付与は行わない。`prek run --all-files`（pre-commit 設定のフック全実行）は variant matrix slice で Pass を記録した補助検証として使われた。canonical 検証は引き続き `mise run` lanes である。evidence レポート領域の COMPAT 証拠は `gh run list`/`view`/`api` の実測 snapshot を URL 付きで転記し、required/observed leg を区別する。将来 run の結果を推測で補わない（synvelis#97、#7）。

実 GUI/IME/2Window/pixel/plugin dock・実 remote/auth/GC・他 provider・OS matrix・consumer precision は全 slice を通じて `NotRun`/`Unsupported` のまま残る。これらは #114 gate・#97 REAL lanes・#219/#294 の責務であり、本グループの headless Pass を実機受入に換算しない運用が一貫している。Issue Close 運用として、#95/#96/#97 はいずれも残存 DoD（Removed cascade apply・native mount 後続・REAL lanes・実機 gate 等）があるため slice 完了でも Open 維持、PR は `Refs #N` で close しない、Close 判断は Coordinator に委ねる方針が全 slice で一貫している（synvelis#95、#96、#97）。

設計実験ログは skim 対象であり、個別の実験結果は対応する IR の実測（test 数・lane 結果・ledger verdict）に集約されている。実験段階の検討事項と最終実装の差異は IR の「下された判断」節に記録される（synvelis#92〜97）。

### 再利用できる手法・パターン一覧

- monotone watermark reseed: apply transplant で `watermark = max(live, after)` とすることで never-reissued 保証を保ちつつ byte-faithful 移植するパターン。issued id は発行せず watermark のみ reseed すれば `id < watermark` 不変条件を維持できる。`PluginInstanceId` のような別 id domain（u64）は entity `id_watermark`（u128）の `max_new_id` に畳み込まない（synvelis#96）。
- keyed-synthetic entity view: 宣言型 entity（id 非保持・`stable_id` が他 kind の ID 空間を借用）は `keyed!`/`keyed_insert!` macro を使わず専用 arm で `stable_id` parse → map 直接操作とし、diff 側の keyed-synthetic 規約と対称にする。parse 失敗は `InvalidIntent`/`InvalidId` で防御的 fail-closed（通常経路では発生しない）（synvelis#96）。
- declaration-presence 規則: serialized 形状が常に field を持つ struct でも authored 意味で「全 field `None` = 宣言なし」とする場合、diff classifier と apply entity view の 2 箇所に同一規則を適用し、fingerprint の absent encoding と `Added`/`Removed` 分類の presence を一致させる。規則の正本は diff 側とし drift リスクは presence 遷移 fixture で固定する（synvelis#96）。
- oracle 委譲（YAGNI）: write 層で entry↔binding 整合・id range・一意性の事前検査を追加せず、write 後の `validate_references` → `validate_against` authoritative oracle に委ねる。`Processor` write が `plugin_instances.sync_processor` を呼ぶため write 順に依らず最終状態が after の verbatim identity に収束することを review で検証済み（synvelis#96）。
- staged publish + resume/replay: restore は sibling staging dir への materialize → 全 byte verify → same-dir rename publish で、source/Saved HEAD/index/worktree を不変に保つ。manifest stage machine（`Started`→`Materialized`→`Validated`→`Published`）で crash window を recover し、post-rename reconcile の `quick_bundle_check` は envelope のみ verify（全 blob digest は pre-publish の full readback で済）という既知制限を明記した（synvelis#96）。
- placeholder fingerprint + 実測 assert: fingerprint 計算自体が先に fail する経路では `expected_target_fingerprint` に到達不能 placeholder digest を置き、assert は実測 error（`Unsupported` exact match 等）に合わせる。原因を偽装しないための規約で、将来計算順序が変われば test が明示的に fail する（黙って通る経路はない）（synvelis#96、#97）。
- cross-project fixture: `SnapshotEnvelope` を deserialize → envelope/project 双方の `project_id` を差し替えて serialize して wire 上の cross-project read を再現する手法。異 `project_id` の committed 端点が `overall:"unknown"` + issues で `equal`/0-change に潰れないことを固定した（synvelis#97）。
- fault injection の到達制御: adapter が provider を所有するため test からは共有 `Arc<AtomicBool>` flag 経由でのみ到達可能 — pin 解決**後**に flag を立てて freshness probe のみを失敗させる等、観測面を絞った fault 注入を行う（synvelis#97）。
- git plumbing の固定 argv + 環境無効化: `GIT_OPTIONAL_LOCKS=0`・prompt/credential helper 無効・fixed argv で index/worktree 非変更を構造保証し、repo oracle（index hash/status/HEAD/staged 前後一致）で実測する。ambient global 設定は fixture-local config で無効化して fixture を決定的にする（synvelis#94、ALIB-04 先例）。
- docs-only evidence slice: ledger 占有・プロダクションコード非依存の test/report/docs のみの変更で coverage gap を補完する分割。新規ファイルのみのため revert は commit 取り消しで完結し、ledger verdict の最終判定は Coordinator に委ねる（synvelis#97）。
- headless→native の段階的配線: headless slice（op 面 + screen spec）→ production backend wiring（実 provider bind）→ native mount/dock 配線（registry fallback + publish 集約 + epoch 強制）→ gpui-test scenario の順で、各段階で外部 gate（実 OS GUI・IME・pixel・実 Git・Domain Oracle）は `NotRun`/`Unsupported` として明示的に記録し、headless Pass を実機 Pass に昇格させない運用（synvelis#95、#97）。

## 適用対象

この知識は Synvelis の VCS（Semantic Visual Diff）全般に影響・説明する。`synvelis-project` の snapshot resolver・diff engine・vcs 契約、`synvelis-library` の Git provider、synvelis-workspace の VcsHistory state/read-model、synvelis-app の VCS op・screen spec・native mount/dock 配線、apply/restore の entity 層・plan 層・journal、fixture matrix ledger と CI/evidence 運用、docs chain・review-protocol の文書運用に関係する。将来の VCS 機能拡張（variant diff adapter #457・restore defect 修復 #458・Removed cascade apply・実機 gate #114・REAL lanes）を行う際の前提知識となる。

## 出処

VCS-01 Immutable Snapshot Resolver（synvelis#92、親 Issue #7）、VCS-02 Semantic Diff Engine（synvelis#93）、VCS-03 Git Read Provider（synvelis#94）、VCS-04 History/Visual Diff UI + backend wiring + native mount（synvelis#95）、VCS-05 Safe Apply/Restore + apply surface 拡張（synvelis#96）、VCS-06 fixture/fault matrix と証拠補完（synvelis#97）、VCS Integration 統合契約 docs-only slice（synvelis#7）、unborn HEAD 誤分類修正（synvelis#335）、ruviform vendor 同期手順（synvelis#230）からの抽出。
