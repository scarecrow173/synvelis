---
id: "KNOW-rn9wDxLJvcWzND0fWWJQJT"
type: "knowledge"
status: "draft"
lifecycle: "draft"
knowledge_kind: "codebase"
title: "スクリプト基盤と個別機能スライス"
created: "2026-11-19"
updated: "2026-11-19"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/28"
    - "https://github.com/scarecrow173/synvelis/issues/47"
    - "https://github.com/scarecrow173/synvelis/issues/83"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/175"
    - "https://github.com/scarecrow173/synvelis/issues/176"
    - "https://github.com/scarecrow173/synvelis/issues/177"
    - "https://github.com/scarecrow173/synvelis/issues/185"
    - "https://github.com/scarecrow173/synvelis/issues/230"
    - "https://github.com/scarecrow173/synvelis/issues/245"
    - "https://github.com/scarecrow173/synvelis/issues/262"
    - "https://github.com/scarecrow173/synvelis/issues/263"
    - "https://github.com/scarecrow173/synvelis/issues/265"
    - "https://github.com/scarecrow173/synvelis/issues/267"
    - "https://github.com/scarecrow173/synvelis/issues/271"
    - "https://github.com/scarecrow173/synvelis/issues/272"
    - "https://github.com/scarecrow173/synvelis/issues/273"
    - "https://github.com/scarecrow173/synvelis/issues/274"
    - "https://github.com/scarecrow173/synvelis/issues/293"
    - "https://github.com/scarecrow173/synvelis/issues/296"
    - "https://github.com/scarecrow173/synvelis/issues/488"
    - "https://github.com/scarecrow173/ruviform/issues/559"
    - "https://github.com/scarecrow173/ruviform/issues/692"
  informs: []
  explains: []
  supersedes: []
---

# スクリプト基盤と個別機能スライス

## 概要

Synvelis の個別機能スライス群（スクリプト基盤、recorder wire、binding UI、native shell と view mount、mixer gain、theme/preset、RCP selection、Ruviform pin 更新、CI 導入、webdav flake 是正）から得られた設計上の決定と教訓をまとめる。共通する原則は、上流（Ruviform）との契約境界を明確にし、検証を独立 oracle と実 dispatch 経路で組み、fail-closed な typed reject と persisted state の先読みで wedge を防ぐことである。headless での green は実機受入に昇格させず、未実施の検証は NotRun として明示する運用が一貫している。

## 詳細

### Ruviform 依存 pin の更新は lockfile 正本と vendor 再生成で回す

Ruviform 依存の pin 正本は `Cargo.lock` のみである。`Cargo.toml` の git 依存は既定ブランチ追従・`rev` 未指定であり、更新は `--precise` を使わず scoped な `cargo update -p ruviform-*` で行う。更新後は `git diff Cargo.lock` で source fragment の SHA 更新以外の version drift がないことを確認する（synvelis#28）。

pin 更新の定型手順は次の通りである。scoped `cargo update -p` の後、`mise run sync-ui-deps` で `ui/vendor/ruviform`（gitignore された生成物で手編集禁止）を再生成し、`check-ruviform-rev --require-vendor` で lockfile・manifest・vendor の単一 SHA 一致を確認する。続いて upstream 差分を `git log`/`git diff <old>..<new>`、`pub` 削除行スキャン、synvelis 消費面 grep、`cargo check --workspace --all-targets` で精査し、canonical 検証 lane（fmt-check・check/lint/test×14 package・test-rt・check-headless-deps・app-webview/app-native/app-gpui・recording-real-input lint・prek）を実走する。

upstream 差分評価の実績では、破壊的変更は多くの場合 synvelis 未使用面（ParentLease 語彙・wire v5・bridge counter 再構成・ui-test driver 語彙など）に限定され、消費面の削除・signature 変更は 0 件だった。新規 API・新規 crate の採用判断は各 pin slice で「今回は採用しない」に統一し、対応する要件 slice の判断事項として記録する。

`check-ruviform-rev` の REV 不一致 fail は stale vendor（`ui/vendor/ruviform` が lockfile と別 SHA）の検出であり、fail-closed として機能する。復旧は `mise run sync-ui-deps` の再実行であり、pin 更新 slice でも実際に stale vendor を検出して復旧した例がある（synvelis#114。AGENTS.md はこの fail-closed を synvelis#230 に紐付けて説明する）。

### main 既存の破損と flaky は pin 更新から切り分けて記録する

pin 更新 slice で観測された失敗の多くは pin 非起因であり、切り分けの記録が判断材料になる。

- main 既存の破損: cc7d5296 slice では `crates/synvelis-player/src/routing_engine.rs` の rustfmt 差分（PR #192 由来、pin と無関係）で `fmt-check` が fail したが、スコープ外として未修正・報告に留めた（synvelis#28）。なお同日の後続 slice では pass しており、間に修正が入ったと推測されるが明記はない。
- flaky 判定: 単発 fail は単独再実行・全件再実行で pass を確認し、pin 非起因と切り分ける。実例として webdav test の `TransientIo`（os error 10053、synvelis#245）と test-app-webview 中の rustc `0xc0000409` クラッシュ（synvelis#28 fd1d4d5f slice）がある。
- feature 統合でのみ発火する lint: synvelis-app の `native`/`gpui-test` feature 経路で `serde_json/preserve_order` が feature 統合され `serde_json::Value` が肥大化し、synvelis-workspace の `LibraryAction` enum が `clippy::large_enum_variant` で fail する。単体の `mise run lint synvelis-workspace` は通るが `lint-app-native`/`lint-app-gpui` でのみ発火する。workspace member の path dep は `--cap-lints` 対象外のため `-D warnings` がそのまま効く。修正は synvelis#488 が占有しており、pin slice では HARD BOUNDARY として切り分け記録のみ行い修正しない先例がある（synvelis#488、synvelis#28 cbcc8343 slice）。feature 統合の確認には `cargo tree -e features` を使った。
- docs index（`docs/**/README.md`）の再生成は slice の lease 次第で運用が分かれる。worker が `doc_maintenance apply --force-index` で再生成する slice と、lease 外として明示的に触らず Coordinator 委譲とする slice があり、lease 外なら Coordinator に委ねるのが安全である。

### Windows socket flake の原因は accepted socket の nonblocking 継承

webdav test の `TransientIo` flake（synvelis#245）の原因特定は、fixture 層の socket 前提を疑う好例である。

Winsock は accepted socket が listener の nonblocking 属性を継承する（POSIX は継承しない）。mock `TcpListener` が nonblocking だと `handle_conn` の `read` が client の先頭バイト到着に競ち勝って `WouldBlock` となり、conn drop → request 未消費のまま close → RST → client が os error 10054/10053 の `TransientIo` になる。socket mode 前提（blocking + timeout）を持つ層では、fixture の `handle_conn` 冒頭で `set_nonblocking(false)` に正規化するのが正しい境界である。`read_request`/`write_response` への `WouldBlock` retry 追加や provider 側の retry は症状側補正として棄却された。

flake の deterministic 回帰 test は、client が accept poll 間隔を大きく超える遅延（150 ms）で request を書き、修正前 Windows では決定的に `os error 10053`（ConnectionAborted）で失敗・修正後 pass することを一時 revert で証明する形で作る。client read は timeout を設定して wedge を hang ではなく fast fail に変換する。原因特定には fixture への env-gated 診断 logging（`WEBDAV_FIXTURE_DEBUG`）が有効で、証拠取得後に revert して残さない運用とした。並列 3 worker × 40 iter のストレスで修正前約 3 %/run の再現率を確認している。

### headless E2E は実 dispatch 経路と独立 oracle で組む

headless E2E の定型は、`UiRuntime::new(DataRegistry)` + `dispatch_op` + `Rc<RefCell<AppService>>` + `SharedService` delegation で実 dispatch 経路を駆動し、test-local fake（`Device`/`InputStream` + `MockPcm`、`FakeDriver`、`FakeScriptBackend`/`FakeSession`）で外部依存を差し替える構成である（synvelis#114、synvelis#273、synvelis#175）。

mounted fixture は app → `ViewHost` → `NativeShell` → `FakeDriver` の composition を `tests/common` に集約し、`deliver_to_host` で `RvfView::drain_inbox` と同一の `Result`→`CommandReceipt`/`Notify`→`StateProjection` decode を再現する。これにより Fake lane と GPUI lane が同一 wire を踏むことを保証する（SV-GUI-05、synvelis#114 関連）。

oracle は独立経路から取る。`syn.view.list`/`syn.project.info`/bundle ファイル直接 parse を独立 Oracle とし、製品 State/Receipt を `syn.*` op / `rvf.ui.command.v1` → `AppService` → receipt/projection → host の実経路で検証する（SV-GUI-05）。

transient 状態の assert 境界にも規則がある。`view_stale=true` のような transient は `ui_pump` 内で解消されるため、app op 境界では resync 済みの resolved state（`status=ok`）のみ観測可能である。transient 自体は下位層（`VizUi`）の test が保証し、app 境界では resolved-state 契約を assert する。

E2E assertion の教訓として、mixer fader gesture の commit は `AutomationMode::Off` の `SetParameterBase` で session control 層に留まり undoable `ProjectOp` にならない。undo 検証には `set_ppqn`（`ProjectOp::SetProjectPpqn`）等を使う。transport play Button の terminal phase は receipt 権威の `Applied` で、`Observed` は correlated projection が land した場合のみである。

doc 関係 router（`route_graph.js`）は repo 全体の既存 broken `defers` relation で fail-closed `blocked` になり得る。宣言済み delegate 順（spec→design→plan→task→test-spec→impl）を手動で踏襲して記録した先例がある（SV-GUI-05）。

### session owner は稼働状態を app 直下に置き commit retry は persisted state を読む

session owner 配置の先例として、`MonitorUxState` が稼働 session を `SynvelisApp` 直下に保持する構造を踏襲し、`RecorderOwner` を `recording.rs` に新設して `RecorderSlot` は互換 re-export とした（synvelis#114）。`capabilities`・`close_veto_reasons`・`recording_info` が同一 owner を読む実態一致が目的であり、`syn.recorder.*` は既存 `syn.recording.*`/`syn.monitoring.*` と独立した新規 namespace とした。

lifecycle の順序は、device open（`InputSession::open`/`add`）を project mutation 前に置き、device 失敗が project を dirty にしないことを保証する。post-mutation 失敗は best-effort `CloseSession` で open session を残さない。`TapWriterEnds` は device/stream handle であり `project.recording.streams` 登録は mutation 段で行うため、`build_payload` の事前登録前提と両立する。

commit retry wedge の教訓が複数ある。`RecordingOp::CloseSession` は `Open` session のみ受理するため、`commit_session` 失敗後の retry で無条件に `CloseSession` を再適用すると validate で永久 wedge する。persisted session state を先に読み、`Open` のときだけ適用し、`Closed`/`Finalized` は `commit_session` の `commit_status` 冪等経路に委ねる。

`StreamWriter::finalize` は非冪等（既 Finalized または `finalized.json` 存在で `AlreadyFinalized`）だが `checkpoint` は冪等である。部分 finalize 失敗からの retry は `AlreadyFinalized` を成功扱いにしないと `stop` が永久 wedge し、`commit`（`Stopped` 要求）に到達不能になる。

`commit_session` は `Failed`/`IntegrityConflict`/`NothingCommitted` も `Ok(CommitReport)` で返す。outcome が `Applied`/`AlreadyApplied` のときだけ active session を破棄しないと、wire に session 除去 op が無い場合に project session（`Closed` 残留）が孤立する。また `commit_session` が `CommitRecording` 適用後に marker 書込み失敗で `Err` を返す経路では、retry の `AlreadyApplied` が revision を進めず通知が永久 skip されるため、`Err` arm でも revision 前進を検出して通知する。

undo/redo は snapshot 復元で per-op validation を通らないため、稼働 session 中は `recorder.active()` 等で `invalid_params` 拒否する境界規則が必要である（`syn.monitoring.punch` の `require_no_session` と同型）。これが無いと recorder の `OpenSession`/`RegisterStream` apply を巻き戻して commit を永久に不能にし得る。

eager allocation を伴う wire param には上限が必要である。`targets[].channels` は `max_block_frames × channel_map.len()` の callback queue と `chunk_frames × channels` の PCM pool を eager 確保するため、上限なしでは巨大確保で process が abort し得る（`vec!` の alloc failure は catch 不能）。`MAX_TARGET_CHANNELS=256` を設定した（synvelis#114）。消費・検証されない dead wire params（`count_in`/`pre_roll`）は parse されるだけの surface なので残さず削除する。

domain op の状態機械を app 層で補正するときは persisted state を読んでから適用する。op validate 規則を迂回する再適用は wedge の温床になる。

### binding UI は receipt を 3 項で routing し detach は tombstone を残す

pinned ruviform rev が `ui/components`/`ui/binding` を未含有のとき、依存 rev を上げず upstream main の契約意味論を non-RT gateway 境界で mirror する先例がある（synvelis#47）。code copy なし・generic 契約のみ採用し、DAW 固有仕様は Synvelis 側に残す。rev 引き上げは別 issue/follow-up とし、Synvelis 側での gesture state machine 実装も棄却した。

receipt routing は `(view, target, command_id)` の 3 項で行う。view+command id の 2 項だと sibling target が同一 command id を合法に mint した場合に誤った binding へ着弾し得る。

detach/close は tombstone（`attached=false` + generation bump）を残さないと、view record 完全削除後の late intent が binding/owner を再生成して domain を mutate し得る。

generation は view attach で mint し binding は view の epoch を引き継ぐ。terminal receipt 到着時に観測済み projection の revision が効果を包含していれば head を `Observed` へ settle する。upstream の「newer projection retires terminal head」規則を projection-arrival order 非依存で実現するため、command mint 時に target revision を記録して照合する。

cancel/gesture 破棄時は revision bump だけでなく全 bound view へ revert broadcast しないと、破棄された preview が他 window で truth のまま残る不変条件違反になる。scrub（seek）gesture は playhead が単一のため transport/position の begin は全 gesture と排他にする。

### native shell は driver port を隔離し well-known target は migration で守る

`NativeShell` は driver-neutral に `WindowDriver` port を消費し、GPUI 型は `synvelis-native` bin の composition root に隔離する（synvelis#175）。`BindingAdapter` は typed intent mint（`rvf.ipc.v1`）と inbound `Event` demux のみを担い、`ControlsUi`/`VizUi` の gesture/receipt authority を複製しない。`syn.app.quit` は silent destroy でなく close request であり、最後の window の `Destroyed` で view teardown + loop exit する。

mount 側の規約として `control_id == target` を要求しないと、`ViewHost::apply_projection` が `control_id` で相関するため projection が黙って落ちる（synvelis#175-02）。

`ViewInstanceId`（永続 identity、`origin_view` へ写像）と `AttachEpoch`（runtime 保証、session allocator が mint）は別物である。host 再生時は同一 allocator から新 epoch を mint し、command-id namespace が dead namespace に隔離される。

theme は `synvelis-theme` カタログの DTCG 文書を embed して `ViewMount.theme` へ渡し、解釈は upstream（RF385 `ruviform-ui-theme`）に委譲する。SV 側に第 2 interpreter を持たない方針である（synvelis#175-02、synvelis#177）。

実窓証拠の取得では、`PrintWindow(PW_RENDERFULLCONTENT)` は GPUI 窓で古い/代替フレームを返し得る（48 色 vs 実画面 168 色の実例）ため `CopyFromScreen` を正とする（synvelis#175-02、synvelis#185）。

well-known `ParameterTarget`（`MASTER_FADER_RAW` = `WELL_KNOWN_BASE + 6`）の bootstrap では、fresh project の master strip に Fader target を seed し、load oracle が存在 + `role == Fader` + `owner == Strip(master)` を要求する。ID に擬態した foreign entry も拒否する。name/kind/mapping/base は re-key されたユーザー fader を通すため拘束しない（synvelis#185）。

well-known ID 導入の schema migration では、v7 で合法だった issued `Fader` target が存在する project は 2 個目挿入で singleton rule 違反になるため、既存 target を well-known ID へ re-key し `lanes[].target`・`assignments[].target`/`depth_source`・`sends[].gain_target` の参照を全て張り替える（v7→v8）。

well-known target は `RemoveParameterTarget` だけでなく send gain adoption 経路（`AddSend`/`SetSendGainTarget` の apply が owner/role を `Send`/`SendLevel` に書き換える）でも拒否が必要である。adopt されると send teardown が target を削除し oracle が project を reject する wedge になる。id 順ソートで well-known ID が末尾に来る場合、件数 cap（`TARGET_CAP`）は issued target のみに適用しないと構造 target が先に落ちる。

既定 `AutomationMode::Read` では gesture commit は preview を authoritative base に戻すだけで project に永続化されない。これは全 parameter target 共通の domain 契約であり slice 固有の欠陥ではない。base 永続化は `AutomationMode::Off` の `BaseEdit`/`SetParameterBase` である。ただし `AutomationMode::Off` にする UI/wire 経路は現時点で存在しない既知制限である（synvelis#185）。

projection wire の `display_value` は adapter で単一キー形（`{"normalized": ...}`）へ明示変換する。ruviform の `projection_value_input` は多キー object を decode しない（RF#414、synvelis#175-02/synvelis#185）。

### theme/preset は検証を envelope に絞り merge と意味論は上流に委譲する

DTCG 文書の SV 側検証は envelope-only（`schema`/非空 field/`layer` tag）に留め、token 意味論（type/value/alias/循環）は上流 resolver に完全委譲する（synvelis#177）。単一文書循環拒否は後段 override で合法な集合を誤拒否する P1 根因であり、RF385（`ruviform-ui-theme`・`rvf-dtcg-profile.v1`、ruviform main PR #397 squash `df45d10`）着地に伴う Rev.2 でこの形に縮退した。`ThemeDocument` は原文 JSON を保持する。`$extensions` を除いた `TokenTree` では上流 `TokenDocument::from_json` が要求する profile block を再構成できないためである。

`apply_preset` は破壊前に `LayoutSnapshot::restore`（Result 版）で全構造検証する。`restore_or_default` は検証失敗を黙って default 化するため採用しない。`PresetViewState` は tool/grid/snap/inspection_mode/viewport の部分集合に限定し、実 ViewInstanceId・EditBinding・gesture・revision_inbox 等の session/entity 状態を template に持ち込まない。既存 view への preset state 上書きも棄却した。

cross-document alias 解決は未規格のため dark theme は alias ではなく literal 値で self-contained に出荷する。catalog は merge 済み値ではなく文書を保持して `documents_for` で層集合を返し、merge/interp は上流の責務とする。rev pin での candidate 接続ではなく branch 追従を選択した（synvelis#177）。

### スクリプト基盤は第二 registry を持たない read-only consumer として設計する

スクリプト基盤（synvelis#271〜#274）は、script が第二 Capability Registry・独自認可・Job 正本を持たない read-only consumer であることを「解析成功 ≠ 実行権限」の fake analyzer fixture で固定している。

`ScriptApiModel` は Gateway の `CapabilityRegistry`/`SchemaRegistry` を read-only 正本として、公開 read API（`list`/`describe`/`versions`/`resolve`）のみで versioned `RegistrySnapshot` を凍結する。`resolve` は `pub(crate)` で公開面に無い。dangling schema 参照は snapshot 全体を fail させず model 側 `UnresolvedSchema` verdict に写像する。1 件の schema 欠落で surface 全体が死ぬのを防ぐためである。

digest 連鎖は canonical JSON（object key 再帰 sort）+ SHA-256 lowercase hex で model/binding/stub/recipe の digest を連鎖させ、drift を `StaleArtifact`/`BindingStubMismatch`/`SourceDrift`/`DefinitionMismatch` で fail-closed 検出する。

i64 の Luau 投影では、f64 で表現不能な `Integer`（min/max が f64 正確整数範囲外または無制限）のみ 10 進文字列 `SynvelisI64` に落とし、`assemble_call` は schema `Integer` 位置と `TimeRange.start`/`len` のみ strict decode する。範囲内の `Integer` は `number` のままにする。全 Integer 文字列化は stub 可読性を損なうため棄却した。

`InvocationDraft` は gateway wire 構成要素を field に持ち `verify_wire()` で `WireRequest::parse` を通す precursor に留める。task/grant/idempotency は opaque `Value` で trusted 型は型として存在しない。draft→実経路は `submit()` → `Gateway::handle` の 1 呼出に留め、Gateway には read-only `schemas()` accessor のみ追加し bind/invoke/handle の意味論は無変更とした（synvelis#271、synvelis#272）。`WireRequest` 直接返却や registry live 参照は棄却した。

`ActionRecipe` の分類は、member 1 件は `Single`（`ProjectMutation` なら prepare 包含 draft、それ以外は直接 draft）、2〜64 件は全 member が `ProjectMutation`・同一 project・同一 consistency・非 nested batch のときのみ `Batch` とする。domain 混在は atomicity を保証できないため typed reject で fail-closed にし、Workflow 誘導は型ではなく verdict の解釈に委ねる。domain 混在 batch の Workflow 自動降格、`$output.` placeholder の script 写像、Gateway 側 draft 専用経路はいずれも棄却した（synvelis#272）。

batch lower は literal-only `MacroStep`（`m{index}` id・`capability@major.minor` pin・literal args/target）へ翻訳し、変更なしの `MacroCompiler::compile` に掛ける。availability・domain/execution・schema・ceiling・nested・64 件 bound の全検査が AUTX-04 単一実装に残る。script recipe は `bindings[]` を生成しない（実行時にしか存在しない id を名指しできない）ため、gateway の batch bindings fallback は synvelis#263 deferred のまま維持する。

automation の `RegistrySnapshot` は live registry 投影ではなく serialized YAML mirror であり、script 側 snapshot（live capture）と automation 側 snapshot（serialized mirror）は別 view である。この差自体を層状 drift 検出に使う。live→automation 投影関数の新設は AUTX-04 の serialized・source-agnostic 契約を越えるため棄却した（synvelis#272）。

Workflow script の中間表現は canonical import document JSON（`WorkflowDefinition` wire 形 + `kind:"workflow"` ノード）であり、script 専用 Plan IR・第二 compiler を作らず既存 decode 経路へ合流する。`action()` の step kind は capability の domain/execution から契約側で決定し、script が kind を名指す経路は型に存在しない。script 専用 typed IR・script 名指し step kind は棄却した（synvelis#273）。

異種 `workflow()` 参照は runtime VM 間呼出しでなく compile 時 inline 展開とする。`(language, version, source_digest)` pin を resolver が報告し importer が照合する（resolver は自己申告しない）。`<call>__<step>` 名前空間・`$input` 引数代入・宣言 output の virtual output map で既存 AST に還元する。`MAX_IMPORT_DEPTH=32`・`MAX_STATIC_STEPS` で fail-closed とし、runtime VM 間呼出し・別 plan 連鎖は棄却した。

式射影は新設 `Expr::Field { base, path }` で構築値へ path を合成する。importer 独自式評価器（第二意味論）は棄却した。compiler は共有 `project_path` で静型付け、eval/macros は fail-closed に path 解決する。

`FileWorkflowResolver` は canonical root 内の `.yaml`/`.yml` のみ解決し、`..`・root/prefix 成分・canonicalize 後の root 外（symlink escape）を `PathEscape`/`UnsupportedSource` で拒否する。digest は parse した byte 列そのものの SHA-256 で、TOCTOU は drift として検出する。

mlua adapter は `mlua = 0.12.2` + `luau` feature を `synvelis-script-luau` のみに隔離し、他 crate は言語中立の port trait のみを消費する。`configure_luau` が無条件 install する `require`/`loadstring`（digest pin 外の filesystem code 実行）・`print`・`debug` は session 返却前に global から strip 必須であり、これは sandbox escape の P1 であった。

VM 外の Rust 側 lift は VM budget の外で動くため、共有 subtable DAG が 2^n 経路に爆発し得る。`MAX_LIFT_NODES=1M` の node budget を Rust 側に持つ。`evaluate` は single-shot で、成功経路でも interrupt flag を再検査する。`pcall` に呑まれた interrupt/budget が success を装えないことを `pcall_cannot_swallow_wall_clock_budget` test で実測確認した。

Option B 境界の判断例として、`.luau` → `ExecutablePlan` の plan compile は registry/surface/`host.rs` accessor がリース外・未存在で不可能と判明し、coordinator 承認で typed definition/import までに留め synvelis#262/#274 へ defer した（synvelis#273）。Option A 結合は棄却。lease 外変更は retro-approve として PR 本文に明記する運用である。

`ScriptEditor` 側では、`AnalyzerRegistry` は `Vec<Box<dyn ScriptAnalyzer>>` + `BTreeMap<language, index>` で一意言語所有を強制し、`resolve` は `Unavailable`/`VersionMismatch`/`Unsupported` を fail-closed で返す。`LuauPortAnalyzer` は `DefinitionInjection` の `TypeDefinition` テキストのみを根拠にし、Hidden/Retired/Disabled/Planned/Unknown/非対応 version の API は stub に存在しないため診断・補完から「存在する」ことを漏らさない。`ScriptEditorDocument` は `set_definitions` 時に `model_digest`/`stub_digest` を pin し、`apply_result` は `SourceRevision` + digest の二重照合で stale を `StaleResult` で reject する（consumer-side stale reject、RF-SCRIPT-00 F-14）（synvelis#274）。ruviform 直接依存 analyzer は棄却した。

Deferred として明記された範囲は、Rhai backend・`.rhai` file 解決・RF-SCRIPT 正式 contract 整合が ruviform#559 gate（製品 port 追跡は ruviform#692）まで、plan compile の CLI 結合が synvelis#262/#274、Coordinator/Plan/Job 統合が synvelis#265/#267、製品 E2E が synvelis#267 である。

### RCP selection は二重 gate で stale を engine 前に拒否する

`structure_digest` は `routing_fingerprint(project).0` とし、engine `checked_key` と field 一致させる。receipt `build_key` が一致するための共有 digest である（synvelis#293）。

stale selection は reconciler の publish 時再検証だけでは engine activation が先行し得るため、engine prepare 前に `selection_pin_fault` で拒否する二重 gate が必要である。

`EdgeKind::Return` は source strip が Return bus 所有の Main edge（return bus → mix feed）であり、destination strip 判定ではない。send tap は `EdgeKind::Send` のままであり、SF-06 クロスゲートの test-spec と整合する。

`StaticProcessorHost::connect` は登録 instance を consume する。`capability_generation=0` の consume は drift 扱いされないため、oracle は drive 毎に host を再武装する。

RCP 契約型（`PortFormat`/`SignalDomain`/`AudioFormat` 等）は `ruviform_graph`/`ruviform_audio_core` の型であり、consumer 側は workspace 継承依存の追加が必要で、coordinator 承認を得て追加した。headless green を実機受入に昇格しない原則に従い、RCP-EX-06（PCM/real plugin/OS/RT）の実機受入は NotRun のまま synvelis#296 RCP-GATE へ defer した。

### CI は 6 job 構成で not-run を pass と区別する

独立 CI は 6 job（verify 3-OS matrix / rt-verify / gw10-verify 3-OS matrix / msrv-verify / dep-guard / evidence）で構成する（synvelis#83）。`rt_` test filter は部分文字列一致のため、厳密接頭辞 `(^|::)rt_[^ ]+: test$` の非空ガードを前段に置く。evidence job は未導入 lane を NotRun 行として固定記録し、全 not-run または 1 lane でも failure/cancelled の run は fail とする。「not-run ≠ pass」を強制する設計である。

ruviform git 依存の access は composite action `ruviform-dep-access` で解決する。deploy key（secrets があれば SSH + host key fingerprint 3 種 pin + url.insteadOf + `GIT_SSH_COMMAND`）/匿名 HTTPS probe/両方不可 fail の 3 経路を持つ。composite action 内で `GITHUB_ENV` に書いた `GIT_SSH_COMMAND` は同 step 内の `git` に効かないため `export` を併記し、Windows の `$RUNNER_TEMP` は cygpath で正規化する。

doc 関係の自己レビューで検出される既存破損として、CI の verify lane が `prek run --all-files` を実行するため、既存文書の end-of-file 違反（`end-of-file-fixer`）が初回 run を fail させ得る。CI 導入の必須前提として同梱修正した先例がある（synvelis#83）。

## 適用対象

この知識は Synvelis の個別機能スライス全般に適用される。Ruviform 依存の pin 更新・vendor 同期・upstream 差分評価の運用、Windows socket と headless E2E の fixture/検証設計、recorder・monitoring の session owner と wire 契約、binding controls UI の receipt/generation 規則、native shell・view mount・mixer parameter target の bootstrap/migration、theme/preset の責務分界、スクリプト基盤の read-only consumer 設計と digest 連鎖、RCP selection の fail-closed 構成、および独立 CI の job 構成と依存アクセスが対象である。主に synvelis-app・synvelis-workspace・synvelis-script・synvelis-script-luau・synvelis-theme・synvelis-native および gateway/automation 系 crate の変更時に参照する。

## 出処

この知識は、スクリプト基盤スライス（SV-SCRIPT-01〜04）、recorder wire（synvelis#114）、binding controls（synvelis#47）、native shell/view mount（synvelis#175-01/02）、theme/preset（synvelis#177）、mixer gain fader（synvelis#185）、RCP semantic→selection（synvelis#293）、Ruviform pin 更新（synvelis#28）、webdav flake 是正（synvelis#245）、独立 CI 導入（synvelis#83）、SV-GUI-05 product e2e（synvelis#114 関連）の各作業記録から抽出した。関連 Issue は synvelis#28、#47、#83、#114、#175、#176、#177、#185、#230、#245、#262、#263、#265、#267、#271、#272、#273、#274、#293、#296、#488 と ruviform#559、ruviform#692 である。
