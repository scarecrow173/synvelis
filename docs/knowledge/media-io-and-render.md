---
id: "KNOW-6hZUefL49KIX4ft9Cqf46b"
type: "knowledge"
status: "draft"
lifecycle: "draft"
knowledge_kind: "architecture"
title: "メディア入出力とレンダリング"
created: "2026-11-19"
updated: "2026-11-19"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/21"
    - "https://github.com/scarecrow173/synvelis/issues/22"
    - "https://github.com/scarecrow173/synvelis/issues/24"
    - "https://github.com/scarecrow173/synvelis/issues/25"
    - "https://github.com/scarecrow173/synvelis/issues/26"
    - "https://github.com/scarecrow173/synvelis/issues/290"
    - "https://github.com/scarecrow173/synvelis/issues/291"
    - "https://github.com/scarecrow173/synvelis/issues/292"
    - "https://github.com/scarecrow173/synvelis/issues/293"
    - "https://github.com/scarecrow173/synvelis/issues/295"
    - "https://github.com/scarecrow173/synvelis/issues/296"
    - "https://github.com/scarecrow173/ruviform/issues/253"
    - "https://github.com/scarecrow173/ruviform/issues/275"
    - "https://github.com/scarecrow173/ruviform/issues/294"
    - "https://github.com/scarecrow173/ruviform/issues/308"
    - "https://github.com/scarecrow173/ruviform/issues/309"
    - "https://github.com/scarecrow173/ruviform/issues/384"
  informs: []
  explains: []
  supersedes: []
---

# メディア入出力とレンダリング

## 概要

Synvelis のメディア入出力とレンダリングは、Ruviform を汎用の media/runtime 基盤、Synvelis を DAW 固有 policy の Owner とする境界のもとで、canonical identity・verified artifact store・render policy/job・missing plugin resolver・driver-neutral な GUI 投影という一連の契約として設計・実装された。headless の縦断スライス L1/L2/X と、render checkpoint（RCP-01〜07）の実装を通じて、identity の再計算による検証、fail-closed の判定、実行していない証拠を実績にしない記録規律が確立された。実機・実 plugin・実 OS の受入は未了であり、本書の設計は headless 検証の範囲で成立している。

## 詳細

### Ruviform と Synvelis の責務境界

Ruviform は汎用 media/runtime 基盤として、media reader/decoder、WAV encoder/muxer、offline 実行 driver、probe/dock/clock/ui-data、staged-file primitive を担う。Synvelis は DAW 固有 policy として、Source/Asset/Project/Import/RenderSpec/JobOrchestrator/Inspector/SignalFlow を担う。この境界は Media & Render Lifecycle の設計契約（synvelis#21）と target architecture の設計レビュー（synvelis#22）で固定された。

汎用 runtime の不足は Synvelis 側で重複実装して隠さず、Ruviform 側の Issue（ruviform#253/#275/#294/#308/#309 等）へ起票する。一時的な回避策には理由・関連 Issue・解消条件を記録する運用が両リポジトリの AGENTS.md で規定されている（synvelis#21）。

GUI は driver-neutral 契約で設計する。Synvelis#175 の ADR（accepted、旧 sv-gui-01 を supersede）と Ruviform#384 の ADR（proposed・仮決定）は方向が一致するが、ruviform#384 の最終採用は ruviform#262/#388/#389 の実機証拠後と明記されており、「方針の整合」と「GPUI 最終採用 PASS」は別物として台帳で分離された（synvelis#22）。plugin GUI embedding の wry `build_as_child` ADR は accepted のままだが WebView 時代の旧経路であり、ruviform#389 で再設計予定である。supersession 管理は Ruviform Owner の責務とされた（synvelis#22）。

### canonical identity と provenance

`RecipeKey` は domain separator と versioned canonical bytes の SHA-256 である。`RecipeBranchPin` は canonical `scope_digest` と `shared_context_digest` の 2 要素のみで構成し、branch authority 識別子（variant set/branch/revision/payload digest/locator）は hashed identity から除外して非 hash の `ResolutionProvenance` に保持する。これにより同一音響 graph が別 branch にあっても同一 key になる（synvelis#290）。pin 全体を recipe に含める案は RCP-REC-01 の契約に反するため棄却された。

`scope_digest` は display name を含むため render identity には流用せず、name を再帰除去した canonical fingerprint を別途計算する。`shared_context_digest` は live semantic objects の scope 外補集合の fingerprint であり、shared edit が全 branch の key を失効させ、scope 内の編集は当該 branch のみを動かす（synvelis#290）。

canonical 抽出の規則は、display `name` の再帰除去、非有限 float（NaN/±inf）の reject（`0`/`null` への正規化はしない）、map/set の key ソート済み直列化、revision・作成時刻・artifact location・無関係 subgraph の除外である。rename と挿入順の変更で key が動かないことは fixture で確認済みである（synvelis#290）。

Artifact identity は canonical manifest bytes の SHA-256（domain separator `synvelis/render-artifact/v1\0` 付き）である。manifest が recipe key・ports・codec・digest・renderer・reproducibility・receipt・provenance・trust をすべて内包するため、recipe key や blob digest から identity を推測する必要がない。blob dedup は ContentDigest CAS で行い、同一 blob を共有する複数 manifest は manifest ID で区別する。`hash(recipe_key, blob_digest)` を artifact ID にする案は 1 対多を表現できないため棄却された（synvelis#291）。

`RecipeRecord` は verify してから信じる。schema version の確認と canonical bytes からの key 再計算が一致しない record、および version skew は `Unknown` に落とし、`Match` に昇格させない（synvelis#290）。

### dependency closure と temporal invalidation

render boundary port を起点に routing を BFS で逆走査し、typed edge 語彙（Audio/Event/Control/Parameter/Sidechain/ExternalMedia/Transport/TimeMapping）で依存閉包を構築する。processor は `PluginStateProbe`/`ExternalAssetProbe` の注入境界で品質ゲートを受け、missing entity・unresolved processor・degraded/unpinned/weak external asset・unverified source・live clock・schema skew はすべて `Unknown`/`Unsupported` に fail-closed で落とし、`Resolved` に混ぜない（synvelis#290）。

temporal invalidation は over-approximate が正しい。既知の dirty interval を lookahead+PDC 分だけ後方、declared tail 分だけ前方へ拡張し、loop region との overlap は loop 全体、seek boundary を跨ぐ変更は render 終端まで、temporal-domain または未知 extent の変更は render 全域へ失効させる。過少区間の再利用を許さない設計であり、時間演算は checked `Tick`/`TickDuration` のみを使う（synvelis#290）。

closure 外の変更は key を動かさないが、boundary 定義の誤りは closure の盲点になり得るため `RenderBoundary` の port 解決責務が残課題として記録された。canonical entity 写像は entity kind ごとの手書きであり、新 kind 追加時に写像漏れがあると key が過小になり得る。schema gate と `validate_references` で fail-closed に倒すが、kind 追加時のレビュー観点として残る（synvelis#290）。

### artifact store の publish・lookup・GC

publish は固定順序で行う: stage write → seal(expected len) → staged bytes の独立 readback と whole/chunk digest → Ruviform の Demuxer/SourceReader（symphonia backend）による独立 decode（writer の view を信用しない）→ codec/container identity・frame count・sample rate・channel 数・codec class 一貫性・port channel tiling・optional PCM predicate の照合 → manifest 構築 → expected_index CAS precondition 付き atomic commit。all-or-nothing であり、index 競合は `Conflict` で reject する。latest-wins は持たず、lookup は deterministic rank で選択する（synvelis#291）。

codec class↔codec identity の一貫性・port channel tiling・per-port PCM digest は manifest-domain の共有 invariant として集約し、publish・remote registration・lookup の全境界で再検査する。adversarial review で「publish のみが enforce し、commit_publish/register_remote/lookup が publish を経由しない manifest を受理しうる」gap が判明し、`Lossless`+`"mp3"` のような矛盾 manifest が `EligibleExact` に昇格しうる問題を `Corrupt`/`BlobInvalid` の reject へ修正した（synvelis#291）。blob 再 decode は lookup の責務ではなく、blob 実測照合は publish/remote 境界の責務という強度配分である。remote 登録は untrusted 境界として manifest の receipt 自己申告を信用せず decode を独立に再実行して receipt との矛盾を `BlobInvalid` で検出し、local publish と同一の atomic commit 経路（staging + PublishCommit）を再利用して独自 commit 経路を持たない（synvelis#291）。

lookup の fitness gate は `RecipeResolution` を authoritative として消費し、`Unknown`/`Unsupported` resolution は `Unknown` outcome（hit を主張しない）とする。`EligibleExact` は lossless codec と certified/verified/replay reproducibility のみを許し、Preview は codec/repro を緩和するが Unknown reproducibility は Preview でも拒否する。quarantine は serve しない。candidate 選択は eligibility rank → reproducibility quality → lowest artifact ID の順で deterministic であり、`FitRejection` evidence を保持する（synvelis#291）。

GC は conservative mark/sweep + grace + tombstone である。Active/Pending reference と external manifest attachment を reachability root とし、Pending は remote reachability 未解決として保守的に live 扱いする。Released は grace_ticks 経過後のみ保護解除し、blob は残存 manifest の digest 参照が無い場合のみ解放する（CAS refcount）。store/read 失敗時は sweep を中断し unsafe delete しない（synvelis#291）。

external manifest attach は VCS provider 非依存の registry で commit SHA・commit bytes を不変に保つ。locator は traversal/区切り/制御文字/空白を reject し長さは 512 以下とし、quarantine ID は attach 不可とする（synvelis#291）。

`InMemoryArtifactStore` は filesystem/OS 非結合の契約参照実装で、`arm_fault(FaultPoint)`（stage/seal/readback/commit/blob/manifest/index/read/reference/external/deletion の各点に任意 StoreError を注入）と `corrupt_next_readback`（one-shot readback 破壊）を備える。ただし実 filesystem/OS の atomicity・fsync・ACL・disk-full semantics は未検証であり、headless の PASS を実 OS/storage Gate の PASS に昇格しない（synvelis#291）。

### render policy と job orchestration

`RenderPolicy` は 3 層の `PolicyPatch`（project declared → user local override → operation intent）を field 毎に explicit > local > declared > default の順で最初の `Some` を採用し、`FieldProvenance{field, layer}` を記録する。default は fail-closed 側（`RequireComplete`・`max_concurrent_jobs: 1`）である。各層を完全な policy 文書にする案は、provenance が「値の由来」ではなく「文書の由来」になり意味を失うため棄却された（synvelis#292）。

`LocalGrants` は環境供給であり policy ではない。shared project の宣言が他者の CPU/GPU/network を強制しない設計である。`RenderJobKey` には `action` と `grant` を含める。entry surface を key に含めると同一要求が surface 毎に別 job になり二重 work が発生する一方、grant を除くと異なる grant が同一 job に coalesce して認可境界を越える。action+grant+target が同じなら GUI/CLI/Workflow/Script を跨いで coalesce し、entry surface は receipt provenance に留める（synvelis#292）。`resolution_digest` を job key に含めるため policy 変更は新 job identity になるが `RecipeKey` は不変で、artifact は lookup reuse により二重 publish しない。

job state は実行結果のみ（Published/Partial/Failed/Cancelled）を持ち、`CompletenessReport` は store の `lookup` 結果から独立に評価する。job state に Complete を含める案は state と verdict の二重管理で不整合の余地があるため棄却された。Share gate が completion policy（BestEffort→AllowedPartial / RequireComplete→Blocked{gaps,retry}）を適用し、verdict は job state から推測しない（synvelis#292）。

Crash は `RenderExecution::Crashed` の typed executor outcome としてモデル化し、orchestrator は job を `Rendering` に残したまま `CrashObserved` receipt を記録し、`resume(key)` が同じ key で render stage を再駆動する。panic catch は headless で再現性がなく test 不能なため棄却された。publish は content-addressed + CAS なので再実行は冪等である（synvelis#292）。headless slice の `run()`/`drive()` は同期 scheduler の等価物であり、state machine と receipt は async driver と同一である。実 OS/VCS/async/RT/Gateway/GPUI は対象外である（synvelis#292）。

### missing plugin resolver と plan reconciliation

`SelectionVerdict` は Exact/NonEquivalent/Blocked である。plugin 検証（identity version/binary digest drift・availability・state restore・advertised port layout・host capability）→ typed cutset 検証 → deterministic 選択 → plan reconciliation の順で進み、未知 state を live 扱いしない（synvelis#293）。

cutset の boundary output は domain/format/latency の全検査を通過した場合のみ `covered_sources` に記録する（`edge_ok`）。reject 済み出力を covered に記録すると「reject された出力が別入力を feed している」二重会計・dangling 判定の偽陰性が起きるため初版を修正した（synvelis#293）。optional 入力 port（`multiplicity.min == 0`）は dangling 強制の対象外だが、region 出力境界の artifact output 被覆要件は optional でも維持する。出力側の網羅性と入力側の必須性は別規則である（synvelis#293）。

preview intent + 明示 opt-in は未知/欠落 plugin を `NonEquivalent` + skipped node として許容するが、`skip_keeps_outputs_fed`（全 out-edge の行き先が fed のまま）と required live region 非所属を条件とする fail-closed 規則を伴う。MainPlayback/OfflineRender/Export では skip/degrade を一切適用しない。「preview + Unknown state を Blocked に倒す」案は編集プレビュー用途を過度に制限するため棄却された（synvelis#293）。

Ruviform の `Direction`/`SignalDomain` が `#[non_exhaustive]` で `Ord` 非実装のため、port layout 比較は数値 ordinal + port name に正規化した `LayoutKey` で決定的に行い、wildcard arm で将来の variant 追加に panic しない構造とした（synvelis#293）。

plan reconciliation は新機構を作らず、`synvelis-project` の `RoutingBuildKey`/`RoutingFault`/`RoutingActivationState`/`RoutingReconcileMismatch` を再利用して stale publish 拒否・last-good 保持・delivery gating を行う。publish 前に project/branch revision・capability generation・graph fingerprint・engine/source generation・clock epoch・runtime instance を再検証し、stale/superseded は拒否して last-good を保持する。`observe_active` は generation と clock epoch の両方一致を要求し、capability snapshot generation と snapshot pin の一致も検査する。pin が証明できない変更は fail-closed であり、`EditKind::SourceContent` は snapshot pin が media bytes を含まないため `Rebuild` 固定とする。render invalidation と plan rebuild は別概念として分離される（synvelis#293）。

### GUI への投影（driver-neutral read-model）

2 段投影を採用する。`synvelis-workspace::render_ui` が authority 型（PluginVerdict/ArtifactEligibility/LookupOutcome/ExecutionSelection/EffectivePolicy/JobRecord/TriggerReport）を display read-model に投影し、`synvelis-app::render` がそれを `ruviform_ui::view` spec に落とす。app 層は `synvelis-render`/`synvelis-gateway` 型を名指せず、authority 語彙は display 文字列（label/reason code/pin text）としてのみ到達する。CLI/Workflow/Script が同じ read-model/code を消費でき、GUI 側に第二正本が生まれない（synvelis#295）。

5 軸を独立 enum として投影する: `Editability`/`Executability`/`RenderValidity`/`PlaybackQuality`/`RuntimeState`。各々 `label()` で安定文字列を返し、Missing Plugin では `edit-possible` + `blocked` + `render-missing` + `quality-unavailable` が同時に成立する。「編集できるが正確には再生できない」を色なしで表現できる。`StatusTone::of_label` は label 語彙を tone に写すだけであり、意味は label が担う（synvelis#295）。

`RenderReason` は `capability.*`/`state.*`/`source.*`/`boundary.*`/`render.*`/`policy.*`/`job.*` の category prefix 付き安定 code を単一語彙化し、UI table cell・disabled reason・CLI 出力が同一 code を共有する契約である。executable candidate のみを button 化する fail-closed spec で、disabled reason は text として残り「なぜ押せないか」が消えない。decode は未知 target/action・malformed params を `None` で reject し、view-local state は project/render/job/policy の authority を持たない。Manual trigger は無条件 enabled で toggle off 不可とする（synvelis#295）。

却下した代替案は、GPUI 直接 View 実装（driver-neutral 契約に反し他 driver を阻害）、display 側での eligibility/verdict 再判定（authority の二重実装）、nodes table の選択式化（view-local state に node 選択権限がなく YAGNI）、app 層での JobRecord fixture 構築（app が名指しできない境界に反する）である（synvelis#295）。

### メディアの import・export・録音 publish

import pipeline は、staging 外配置 → bytes 完全読取 → size/hash/container/codec/frame/channel verify → atomic publish → `detect_dir_sync == Supported` のときのみ parent dir sync → manifest stage 記録 → ExpectedRevision commit → `applied_imports` registry での `Applied`/`AlreadyApplied`/`IntegrityConflict` 分類、の順である。失敗時は manifest は残るが部分 Project mutation は起きず、blob/orphan は明示分類される。未検証の Path は Source になれず、resolver は `SourceState >= Verified` を必須とし、asset/digest blob 探索 → blob hash 再検算 → probe → PCM canonical SourceReader → `Prefetch::manual` の経路を取る。全 Project mutation は `ProjectController` の検証済 Command のみで、Graph 層に Project/Track/Clip 公開型は導入しない（synvelis#24）。Windows では dir sync が `Access is denied` になるため、`detect_dir_sync` の capability 判定を必須とし、`Supported` のときのみ sync して、それ以外は receipt に unsupported を記録する。fsync 省略を黙って成功扱いしない（synvelis#24）。

lowering は Clip bindings を `ClipWindow`（engine_start/engine_len/source_start/source_len）へ lower し、musical anchor は TempoMap で media 変換し、source offset/mute/channel layout/rate 不一致を処理し、skip は `SkippedClip` diagnostic として明示する。SourceFrame/EngineFrame/tick を混同せず、tick→frame は最終 frame 境界で丸める（synvelis#24）。

playback 経路では、`PrefetchConsumer::fill` の `expected_first` は engine 側の期待値ではなく stream 位置の resync assertion であり、ruviform 自身のテストも `consumer.position()` を渡している。boundary は fill 内でのみ adopt され、adopt すると `position()` が seek target に reset されるため、bulk fill が boundary を跨ぐと per-frame 帰属が失われる。resync 規則は「generation < base+issued_seeks または position != want の間は single-frame probe、整合後のみ bulk」である（synvelis#24）。

`pump()` は 1 呼出 ≤1 item（seek boundary・1 decode block・marker のいずれか）である。1 block あたり 1 pump では boundary が consumer に届かず全無音になる不具合があったため、`Pushed` の間は反復する bounded pump loop に修正した。seek の sample-exact 性は「先に drain、後に seek」で担保する。`exec_cmd` は boundary を pending 末尾に積み stale content は documented に play-through するため、`driver.seek` が quiesce process で ring を drain（discard・`stats.mismatch` に記録）してから `handle.seek` を発行し、boundary が consumer の最初の item になるようにする。activation は validation/compile/publish 成功後にのみ producers/handles/stats を差し替え、失敗時は last-good plan を保持する（synvelis#24）。

L2 の offline export では、`RenderSnapshot::freeze` が project を clone し live/saved/tempo revision と `source_id → content_digest` を記録する。controller・undo/redo に触れず snapshot は immutable である。preflight は 2 段で、`preflight_static`（range・tail 分類・destination conflict・WAV capability・mono/stereo geometry・`max_block`/insert magnitude bound 1<<20・tail-carrying insert vs `TailPolicy::None` の整合・wire-format 幅による RIFF 上限・source registration/Verified/asset）と `preflight_canonical`（source rate == engine rate）に分かれる（synvelis#25）。

decode-to-memory は rendered span ∩ window に限定する（`[play_pos, end+tail+latency)` ∩ window のみ decode し `base` 付きで返す）。clip 全 window や巨大 range の materialize を避けつつ、`total_latency > declared` は compile 後に refuse するので bound 不足で silent output にならない（synvelis#25）。source open は同一 fd で streaming digest（`content_digest_stream`、fwd+bwd CRC32 を 64KiB chunk）+ symphonia demux + codec 先出し check + `SourceReader::open` を行い、digest↔decode の TOCTOU なし・全 blob の二重読込なしを実現する（synvelis#25）。

`Completed` outcome は verify+publish 成功後にのみ構成され、Failed/Cancelled/Partial は決して publish しない。staged temp は全失敗経路で除去し、既存 destination は `replace_existing` なしでは保持する。Live playback の processor/plugin instance とは一切共有しない（synvelis#25）。

`Insert::Delay{frames:0}` は identity である。従来 `max(1)` ring が 1-sample shift を量産していたため、spec の `y[n]=x[n−frames]` どおり pass-through に修正した。`TailPolicy::None` は provable bounded tail（Delay のみ。OnePole の declared latency は PDC 補正で range 内に drain されるが自然減衰は unbounded）>0 の chain に許さず `InvalidRange` で refuse する。「provably no tail」の evidence が嘘になるのを防ぐためである（synvelis#25）。

codec gate は open 時に一本化する。非 PCM は `SourceReader::open` 前に `UnsupportedCodec`（`source_id` 付き）とし、export が Preflight stage の `UnsupportedCodec` に remap する。decoder の有無で `Corrupt` と `UnsupportedCodec` が揺れないようにするためである。zero clip node は ruviform の opaque graph error に落ちる前に `NothingToRender` で明示する。独立 reopen verify は symphonia demux + `SourceReader`(Canonical) で全 frame decode し、rate/channel count/frame count を `OutputFacts` と照合し、`expect_pcm` で全 sample を比較する（layout variant は跨いでよい。unpositioned WAV は `Custom` として reopen される）（synvelis#25）。

録音側では、commit が raw `f32le-interleaved` bytes + fold digest を `content_digest` に載せていたが、resolver は `media_root/blobs/<content_digest>` に symphonia-decodable container を要求するため、state gate は通るが blob が解決不能だった。canonical publication 境界を選択して修正した（synvelis#26）。`publish_recorded` は `load_finalized` で `Complete` ref のみを対象とし（`partial-finalize`/`missing-finalized`/`finalized-unreadable`/`capture-fault` は `skipped` 分類）、staging manifest と ref の identity 照合（不一致は `PublishError::Conflict`）を行い、整合 receipt があれば `reused`、別 media を指す receipt は `Conflict` とする。raw media を frontier（`media_byte_len`）まで読み、`WavMuxer`/`WavEncoder`（L2 と同一の ruviform-io-wav）で PCM f32 WAV へ encode し、import と同一 `content_digest` で blob へ `publish_atomic` し、receipt を atomic 記録 + dir sync する。`ensure_blob` は既存 blob を `content_digest_stream` で再検算し、digest 名と異なる内容は上書きせず fail-closed とする。layout は 1ch Mono/2ch Stereo のみで、他は `UnsupportedChannels` の typed refuse（capability gap）である（synvelis#26）。

録音 media の canonical 化境界は verified-media 境界（`synvelis-player::publish`）に置き、resolver/graph/render には format 特例を入れない。録音 raw f32le + fold digest は symphonia 系 resolver が decode できないため、PCM f32 WAV blob へ encode して import と同一 content-digest scheme に揃える（ADR X-1）（synvelis#26）。commit は publish 済み receipt を採用するだけで publish 自体は行わない（command 内で media I/O しない契約）。publish を `commit_session` 前に呼ぶのは呼び出し側契約だが、manifest の fingerprint 再検算が stale manifest を破棄・再構築するため canonical digest に自己修復する型付き回復になる（synvelis#26）。

録音 commit の canonical digest 採用は、receipt が ref と schema/format/identity/raw_digest/frames/rate/channels で整合する場合に限る。不整合 receipt は `CommitError::Manifest` で fail-closed とし、receipt 不在時は raw digest に fallback して VS-D の直接 commit 挙動を保持する（synvelis#26）。`publish_recorded` 以降の経路は `prepare`/`lower`/`HeadlessDriver`/`export` の generic API のみを呼び、resolver/graph/render には録音分岐を追加していないことが静的に立証された（X-06）（synvelis#26）。

### cross-gate fixture の配置と oracle 設計

cross-gate fixture は `synvelis-render` の test に集約する。crate 依存方向（render → player/project/recording）により、recipe・artifact・orchestrator・resolver・variant/branch 型を一つの test file から名指せるのは render test のみである。`synvelis-app` は render 型を名指せないため app test は追加しない判断が取られた（両端は RCP-04/RCP-06 の既存 fixture が被覆し、coverage/cost が不均衡）（synvelis#296）。

store→resolver の接合は `lookup()` の `CandidateReport` を `RenderCandidate` に直接転記し、eligibility を fixture で再判定しない。「store が verified と言ったものを resolver がどう扱うか」という権威間の受け渡しだけを pin し、判定ロジックの二重実装を避ける（synvelis#296）。branch adopt の oracle は独立構成した muted live project の `resolve_live` key である。「adopt 後 live key == candidate branch key」の当初仮定は、`RecipeBranchPin` が hashed identity で live/branch key を設計上分離するため不成立と実測で判明し、branch resolution の出力を期待値に使わない独立 oracle に修正した（synvelis#296）。

queued cancel は `JobOrchestrator::submit`（non-driving）経路で検証する。`handle` は non-coalesced submit を即 `drive` するため、queued cancel を handle 経由で検証すると job が既に terminal で `cancel()` が false を返す（初回実行で実際に失敗を確認）（synvelis#296）。

実行しない lane（実 Window/GPUI・実 plugin ABI・実 PCM clock・AudioLens consumer）は stub/`#[ignore]` で疑似配置せず、ledger の status 語彙（NotRun/Deferred/Blocked/Unsupported）と所有 gate の明記に留める。実行していない証拠を fixture で偽装しない（synvelis#296）。test file 間の helper 共有機構は存在せず、contract 独立性のため各 test file が自前 helper を持つ慣行である（synvelis#296）。

### 証拠の正直さ・status 語彙・docs 鎖

evidence 語彙は `Pass`/`Fail`/`Unsupported`/`NotRun`/`Pass(部分)` の 5 status である。headless fixture の Pass は `Pass(部分)` として記録し、実機/GUI/Plugin/OS 受入に繰り上げない。未実装は `NotRun`、実機未検証は `Pass(部分)` とし、設計レビューの Pass と実測の Pass を区別する。「ソースに存在する」こと（契約実在 = Pass pin）と「実行して成功した」ことを台帳で明確に分け、実測なし Pass = 0 とする判定規則を固定した（synvelis#21、#22、#296）。

canonical docs 鎖は spec→design→plan→task→test-spec→impl-ir/impl-exp の 7 文書 + Issue 毎の evidence ディレクトリ（case ledger・verification）である。L1/L2/X は discovery/spec/ADR/design も含む縦断鎖を持つ。front matter の relations（implements/derived-by/verifies/references/depends-on 等）を相互に整合させ、docs index は `doc_maintenance.js index apply`・`sync_impl_index.js` で再生成し手編集しない（synvelis#21、#24〜#26）。

RCP 系の証拠は Issue 毎の evidence ディレクトリ（acceptance ledger + verification + fault matrix）に集約し、実行した検証コマンド・OS・toolchain・SHA・exit code を実測記録する。実装記録（IR）には Summary/Context/Changes/Decisions/Acceptance Matrix Traceability/Validation/Risks/Known Limitations/Follow-ups/Rollback を含め、未計測・未実施項目（cancel phase の一部、Estimated tail の別検、実 codec coverage 等）を明示的に記録する慣行が確立した（synvelis#24〜#26、#290〜#296）。

review-protocol の adversarial pass を実装後レビューとして実施し、発見した実害は修正し、検証済みの非修正判断は制約として記録する運用である。RCP-02 では media fact 検証の境界漏れ、RCP-03 では attach 失敗の無記録破棄と receipt 遷移の逆行/欠落、RCP-04 では capability snapshot generation 検査漏れが修正された（synvelis#291〜#293）。

### 運用知識（toolchain・依存・検証 lane）

開発ツールとタスクの正本は `mise.toml` である。通常の検証は `mise run <task>` 経由で行い、`fmt-check`、`check <package>`、`lint <package>`、`test <package>`、`test-rt`、`check-ruviform-rev`、`sync-ui-deps`、`check-headless-deps` を使う。`check`/`test`/`lint` は package 引数必須である。docs/evidence-only slice では crate 変更がないためこれらを実行せず、ledger に NotRun 根拠を記録する（synvelis#22、#25）。

Ruviform 依存は既定ブランチ追従の git 依存で、解決 revision は `Cargo.lock` に pin する。`Cargo.lock` は再現性のため追跡する。lock rev が変わる変更（pin 更新・`cargo update`・ブランチ切替）を取り込んだ後は `mise run sync-ui-deps` で `ui/vendor/ruviform`（gitignore された生成物）を再同期する。`check-ruviform-rev` の REV 不一致は stale vendor を検出する fail-closed であり、復旧は `sync-ui-deps` の再実行である（synvelis#22、#230）。

依存 DAG の cycle 検査は `cargo metadata --format-version 1 --locked --offline` + node DFS（`dep_kinds.kind=="normal"` の辺のみ対象、dev/build 辺は別カウント）で再現可能な記録とする。`cargo tree` の目視では dev/optional/target 辺の分類が不明瞭なためである（synvelis#22）。

`ruviform-media-core` の `Tick`/`TickDuration` は unchecked `Add`/`Sub` のみ公開であり、Synvelis 側の時間演算はすべて checked（`checked_add`/`checked_sub`）で行い、同演算子の直接利用は lint 対象である（synvelis#290）。

toolchain は mise 管理で `rust-version = "1.89"`（MSRV）、CI toolchain pin は `mise.ci.toml` の `1.99.0` であり、記録上は rustc 1.99.0 で検証されている（synvelis#25、#293）。

docs 検証補助コマンドは `doc_status.js lint --type {spec,design,plan,task,test-spec}`、`doc_maintenance.js index apply`、`audit_impl_record.js`/`audit_experiment_log.js --json`、`audit_docs.js --type all --json`、`route_graph.js --focus <topic> --json`、`review-protocol` である（synvelis#21、#22）。pre-existing の無関係な broken relations（route graph の `artifact-graph` blocked 等）は本 slice 範囲外として修復せず記録のみとする。無関係な既存 docs の問題を勝手に直さない（synvelis#22）。

### 再利用可能な検証パターン

決定的 WAV fixture は、44-byte canonical header + `((frame*31 + ch*7) % 4096 - 2048) * 16` の i16 パターンで、期待 canonical f32 は `v / 32768` である。fixture bytes の sha256 を表で記録し、同一入力・反復実行で同一 hash を確認する（synvelis#24、#25）。

`MockPcm` 純関数（`input_frame, channel, seed`）を bit-exact oracle として使い、独立 reopen decode（symphonia demux + SourceReader canonical decode）で rate/channel/frame count/PCM を照合する。writer の view を信用しない（synvelis#25、#26）。

fault injection は `InMemoryArtifactStore::arm_fault(FaultPoint)`（各 stage/seal/readback/commit/read/reference/external/deletion 点に任意 StoreError）と `corrupt_next_readback`（one-shot readback 破壊）、export 側は `OutputHooks` seam、cancel は `CancellationToken` を prepare/process phase に通す（synvelis#25、#291）。

canonical serialization は domain-separated SHA-256 + `deny_unknown_fields` serde + schema version gate を traversal 前に fail-closed で置く。canonical bytes ↔ ID の一致は `verify_manifest`/`RecipeRecord::verify` が再計算で検証する（synvelis#290、#291）。

独立 oracle 構成の原則は、system-under-test の出力を期待値に使わないことである。branch adopt は「同じ acoustic edit を施した独立 live project の `resolve_live` key」を oracle にし、pin 分離自体を assert 対象にする（synvelis#296）。

idempotent publish は、content-addressed blob + receipt 採用 + `ensure_blob` が digest 名の内容を再検算してから再利用する形で実現する（synvelis#26）。temporal invalidation の over-approximation、conservative GC（typed root + grace + tombstone + CAS blob refcount）、VCS 非依存 external manifest registry は、cache/artifact 系で再利用可能な設計パターンである（synvelis#290、#291）。

## 適用対象

media import/export、録音 commit の canonical publish、render recipe と artifact cache、render policy/job orchestration、missing plugin 解決と plan reconciliation、render availability の GUI/CLI 投影、証拠 ledger と docs 鎖の運用、Ruviform 依存 pin と vendor 同期に影響・説明する知識である。headless 検証の範囲で成立しており、実機・実 plugin・実 OS の受入は別 Gate の責務である。

## 出処

Media & Render Lifecycle 設計契約（synvelis#21）と target architecture 設計レビュー（synvelis#22）、headless 縦断スライス L1/L2/X（synvelis#24/#25/#26）、render checkpoint 実装 RCP-01〜RCP-07（synvelis#290〜#296）、および upstream の generic capability・job 基盤に関する Issue（ruviform#253/#275/#294/#308/#309/#384 等）からの抽出。
