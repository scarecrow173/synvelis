---
id: "KNOW-gc1vOhVuopS5XF2sd0yAfq"
type: "knowledge"
status: "draft"
lifecycle: "draft"
knowledge_kind: "codebase"
title: "CLI・配線・本番統合スライス"
created: "2026-11-19"
updated: "2026-11-19"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/262"
    - "https://github.com/scarecrow173/synvelis/issues/274"
    - "https://github.com/scarecrow173/synvelis/issues/283"
    - "https://github.com/scarecrow173/synvelis/issues/286"
    - "https://github.com/scarecrow173/synvelis/issues/324"
    - "https://github.com/scarecrow173/synvelis/issues/365"
    - "https://github.com/scarecrow173/synvelis/issues/368"
    - "https://github.com/scarecrow173/synvelis/issues/514"
    - "https://github.com/scarecrow173/synvelis/issues/515"
    - "https://github.com/scarecrow173/synvelis/issues/516"
    - "https://github.com/scarecrow173/synvelis/issues/517"
    - "https://github.com/scarecrow173/synvelis/issues/518"
    - "https://github.com/scarecrow173/synvelis/issues/521"
    - "https://github.com/scarecrow173/synvelis/issues/530"
  informs: []
  explains: []
  supersedes: []
---

# CLI・配線・本番統合スライス

## 概要

Synvelis の CLI・配線・本番統合に関する個別 Issue スライス群から得られた設計上の決定と教訓をまとめる。対象は、headless CLI の実 VM 実行配線（synvelis#262）と `script analyze`（synvelis#274）、strip-fields gateway exercise（synvelis#283）、Difference Explorer の app wiring（synvelis#286）、plugin_instances の VCS semantic diff 分類（synvelis#324）、impl index/IR 文書衛生（synvelis#365・#368）、session engine selection の判定（synvelis#514）、OS credential session 供給（synvelis#515）、mlua→ruviform adapter（synvelis#516）、render/branch pipeline の本番配線（synvelis#517）、production job system（synvelis#518）、Git LFS の HTTPS/TLS transport（synvelis#521）、stateless `PluginStateDecl` 契約（synvelis#530）である。

共通する原則は三つある。第一に、composition root や `VariantPorts::standard` のような権威ある登録点へ責務を集約し、facade 側の手動 install や暗黙共有を排除する。第二に、fail-closed な typed reject と `Unavailable`/`Pending` の正直な返却を契約とし、成功・証拠・対応を捏造しない。第三に、計画段階・NotRun・Deferred の事項は実績と区別して記録する。文書 status の drift が複数 slice で観測されており（task が todo のまま残る、IR が in-progress のまま残る等）、実績の判断には文書種別ごとの status を個別に確認する必要がある。

## 詳細

### headless CLI の実 VM 実行配線と script analyze

`synvelis-cli workflow run --file`（`.yaml`/`.yml`/`.luau`）と `action run --file`（`.luau`）を分類+deferred 実装から実 headless 実行へ置き換えた。composition root で単一 Gateway/CLI adapter/commit boundary を共有し、live lanes・script surface・registry compile が同一権威を見る構成である（synvelis#262）。`HeadlessSessionHost` の共有 handle は field の `Arc` 化 + clone を返す getter で実現し、`new`/`without_commit_boundary` の signature と既存呼出 3 箇所を不変に保った（暗黙共有の getter 案より composition root での明示を優先）。canonical `ScriptSurfaceSpec` 生成器は命名無関知の `synvelis-script` でも cli（#274 と lease 共有不可）でもなく `synvelis-automation` の `surface.rs` に置いた。surface 命名規則は `synvelis.` 除去 → `.`/`-`/`_` 分割 → camelCase で deterministic。衝突（別 id 同名・同 id 別 version — version 違いも script 名を shadow するため）は `DuplicateApiName` で fail-closed、identifier 不適合名は emit して model の `InvalidName` verdict に委ねる（synvelis#262）。

run 終端 state は envelope/exit code へそのまま写像する: Completed→completed/0+outputs、Approval→approval-required/0+references（resume 非対応を明示）、Failed→lane の最後の `ErrorEnvelope` を verbatim で gateway exit code（denied→3 等）、非終端→accepted/7。`RecordingStepLane` が lane の最後の `ErrorEnvelope` を保持し、成功を捏造しない（synvelis#262）。`--inputs`/`--inputs-file` は workflow のみ対象（`InputDecl` は全必須・default 無し）。action recipe は request-level fields を `RecipeSpec` が持つため inputs flag の対象外で、両 flag 同時指定・非 JSON object は fail-closed（synvelis#262）。`MacroRegistry::empty()` は named macro を compile 時 typed reject に留める意図的 fail-closed（macro load 経路は AUTX-04 後続）。`policy_epoch=None`/`task=None`/`grant=None` で authority を mint せず、in-memory proposal store のため Approval references は cross-invocation resume を約束しない（envelope に明記）（synvelis#262）。実 job step・named macro 実解決・approval→apply 継続・durable run journal は外部 gate（synvelis#267・AUTX-04 系）に deferred され、synvelis#262 は OPEN 継続である。

e2e の落とし穴: 空の policy store は全 capability を denied にするため `denied`(3) が正直な golden であり、emit-only workflow のみ `Completed`(0) に到達する。`boot_project` は saved bundle を要求するため空 dir の `--project` は boot(9) で失敗し、fixture は `project_dir` の saved bundle を使う。YAML frontend は flow collection（`outputs: []`）非対応で block 記法/省略が必要（synvelis#262）。

`synvelis-cli script analyze --file [--cursor]` は merge 済みの既存 seam（`boot_discovery()` ScriptStack → `LuauStub`/`DefinitionInjection` → `AnalyzerRegistry`+`LuauPortAnalyzer` → `analyze_with_injection` → `verify_revision`）をそのまま consume する配線のみで、新規依存・第二 registry/authority の追加は行わなかった（synvelis#274）。analyze は `script::classify` の kind 軸を流用せず専用の小さな `classify_language` を新設した（`.luau`→luau、`.rhai`→rhai id、`.lua`→Unsupported、その他→InvalidInput）。`script` 名詞の parse は `analyze` のみ許可し、他 verb・裸 `script` は Usage reject で「analyze は存在するが実行面は存在しない」を文面で伝達する。analyzer diagnostics は result データであり Error severity でも `completed`+exit 0。`SourceRevision(1)` 固定（単発同期 invocation に stale domain は無い）。`AnalyzerError` は `#[non_exhaustive]` のため写像は wildcard arm を含み、contract drift（`VersionMismatch`/`StaleResult`/`Internal`/未知 variant）は `cli.internal`(10) に写像する（synvelis#274）。

human 描画は analyzer の 0-based position を 1-based `line:col` に変換するが、JSON envelope は契約どおり 0-based のまま。e2e の wrong-arity 検証は completion `detail` の `declare function` signature から arity を導出する 2 段構成で、live registry の関数名/arity を fixture に固定しない（synvelis#274）。「代替実装の実験が無い」slice では exp.jsonl に start/summary の 2 event のみを記録し「no experiments」の意図を明示する — 試していない実験イベントを捏造しない。実 Luau.Analysis FFI 切替は ruviform#560・rf#692 gate に帰属し、synvelis#274 は OPEN 継続である。

### variant diff の gateway exercise と Difference Explorer 配線

CB-10 typed contract lifecycle（fork → typed edit reject → adopt CAS/approval/digest pin → discard → describe/list/compare）を `VariantPorts::standard` 経由で headless exercise する test-only slice が strip-fields scope で実施された（synvelis#283）。wire 表面に strip-domain inner op capability が無いため positive mask-gated edit は表現不能で、edit 面は typed reject の一様性（ordinary op 3 種 + stale pin 2 種）で証明し、routing/external op の fail-closed を境界証拠とする方針が採用された。production code の変更は含まない。adopted branch への edit は live edit であるため confinement 負例は candidate branch のみで構成する。scope-identical candidate でも `AdoptOutcome::NoChange` ではなく `Applied` が返るのは pointer swap/freeze が実 project truth だからであり、正しい期待値である（synvelis#283）。

CB-09 Difference Explorer の app wiring では、adapter 登録の authoritative な一箇所は `VariantPorts::standard`（synvelis#539/#542 が CB-07 adapter を登録）であり、facade 側の手動 install は stale 前提の冗長コードとして除去された（synvelis#286）。view-local action（select/audition/layer visibility/set-target）は persisted target を要求しない authority 非依存経路 `apply_variant_diff_view_local` が必要で、fresh view（target 無し）で selection/layer visibility が fail-closed になる設計 gap を app test で実測して判明した。authority action は persisted target、無ければ action 自身の explicit pin で context を解決し apply 時に再検証する二段構成である。adopt commit は compare target を mint しない。target は明示 pin のみが権威を持ち、adopt 結果の ambient 昇格は stale-context 検証を迂回するため、view は untargeted/unavailable を正直に維持する。adopt は `Adopt` intent のみ受理し preview/compare intent は malformed reject で、preview が commit 経路へ混入する経路を型で閉じた（synvelis#286）。

project/bundle 切替時は engine・全 session・target・publish observation を全クリアする。identical bundle copy の別 project で branch id が衝突し前 project の session/cache を誤再利用する経路を回帰 test で再現した。op 面は `syn.variant-diff.visual`（projection read）/`view`（lifecycle）/`action`（dispatch）の 3 本で責務分離し service registry に収まる。projection は live authority/context を毎回再導出し、persisted target の pin が一致する場合のみ session evidence を付与する。signal provider 出力は facade 内 adapter で provenance 保持のまま `AuxLayerEvidence` へ写像し、unavailable/unknown/unsupported を zero-diff に変換しない。exact audition は plugin/render probe 未 bind のため `Unavailable` を維持し捏造しない（editability のみ CAS adopt 経路の存在で available）。app は gateway 型を名指しできない（AC-05）ため、workspace 側 facade が `VariantPorts`/`simulate`/`execute` を opaque に所有し、app import は workspace 型の module path のみに限定する（synvelis#286）。

### plugin_instances の VCS semantic diff 分類

`plugin_instances.instances`（schema v13 の persisted `StablePluginInstanceId` store）を VCS-02 semantic diff の分類対象に追加する際、`ObjectKind::PluginInstance` は enum 末尾に追加して既存 variant の `as u8` discriminant を保持した（change ID・issue dedup key が discriminant を材料にするため）（synvelis#324）。keyed-synthetic collection（`ProcessorId` 空間借用）は `raw_kind_map` に登録してはいけない。登録すると全 Resolved processor が `{Processor, PluginInstance}` の内部衝突で `AmbiguousIdentity` を捏造する（`SmfDocument`/`AppliedCommit`/`TrackMonitorDecl` と同型の既存規約）（synvelis#324）。

diff 分類規約: entry の `id`/`plugin` 差は `Replaced`（persisted identity の content-identity replacement、`Clip.source_id`/`Asset.digest` と同じ意味）、その他 field 差は `Modified`。`watermark` は store 直上の allocator bookkeeping で `diff_map` 対象外（drift のみなら `Equal` 維持）。pre-v13 側の `id` は migration 採番の投影 artifact で authored 履歴ではないため `Suppressed("pre-v13-plugin-instance-id")` → `PartialProjection`（`pre_v7` と同型の either-side 規則）。instance-only 差分が v2 の `Equal` から `Different` に変わるため `DIFF_POLICY_VERSION` を 2→3 に bump した。version は `ComparisonKey` の材料なので bump しないと旧 policy の cache key と衝突する（RCD-03 v1→v2 と同根拠）（synvelis#324）。

apply surface は `supported_kind` 非追加の fail-closed を維持する。select-all fixture（`changes.iter().collect()`）が新 kind を拾うと `target_fingerprint` で `Unsupported` panic するため、apply-surface kind に絞る helper を test 側に追加した（`supported_kind` が `pub(crate)` で integration test から到達不能なため `matches!` 複写）（synvelis#324）。`PluginInstance` change の apply/restore（byte-faithful な `id` 値復元 + instance watermark reseed）は monotonic never-reissued 契約と live 採番履歴の統合設計が必要で別 Issue 候補であり、本 slice は apply surface fail-closed を維持する（synvelis#324）。

### render/branch pipeline の本番配線

`DirectoryBranchContentStore` は content-addressed `<root>/<sha256-hex>` の one-file-per-digest store（StagedFile + CreateNew + FileAndDirectorySync）。`CreateNew` の exists-check→publish 間 race は readback digest 検証で解決する。content addressing により「同名・別 bytes」は honest writer では起きない。corruption は fail-closed（synvelis#517）。`JournaledRuntimePublish` は常に typed `Pending` のみを返し `Published` を返さない。synvelis-project 内に activation を確認する runtime が存在しないため契約違反になる。RCP-BR-04（project commit ≠ runtime activation、last-good 保持）に従い、journal 書込み失敗は typed `Failed`（synvelis#517）。

`synvelis.render.*` 9 op は `RenderDispatch` trait 経由で dispatch し、`Dispatched`/`Pending`→committed provenance、`Unavailable`/`Failed`/port 不在→determined rejection である。render op は project state を変更しない（candidate 不変を pin、provenance に dispatch evidence）。`VariantPorts::standard(Option<&Path>)` で durable/read-only project boot は `Some(&ctx.canonical)`、discovery は `None`（synvelis#517）。per-op `required_permissions`/`data_access`（`CapabilityDescriptor` 上）は宣言的 metadata であり enforced grant dimension ではない。実効 floor は wire-capability permission + capability list + scoped resource effects（`Target::None` の render op では scoped effect なし）で、branch op と render op で同一機構 = parity である。idempotent apply replay: 同一 idempotency key + 同一 payload → `GatewayError::DuplicateRequest`（determined replay、二重 effect なし）、同一 key + 異 payload → `IdempotencyConflict`（branch ops と同一機構）（synvelis#517）。

restore fork-copy（VCS-RESTORE-10）は Captured `payload_digest` を全て dest `branch-content/<hex>` へ digest 検証付きで materialize し、receipt の `copied` は staging 時点でなく publish 後の dest 実在で再計算する（部分失敗時の虚偽報告を防止）。source 不在/port 不在は restore 失敗でなく per-reference `unavailable` verdict、transport/digest 失敗は restore abort（synvelis#517）。

### session engine selection と stateless PluginStateDecl

session rebuild 経路が selection を消費しないのは意図的判定（分岐 B: 実装不要）と確定した（synvelis#514）。`SelectionInputs` の production 供給経路がなく、空 evidence は `MissingPluginEvidence` fail-closed、部分 evidence の gate 化は `ProcessorHost::connect`（ground truth）より弱い情報で false-block を持ち込む。substitution は plan shaping に使われず published plan は変わらない。selection を「観測のみ」で rebuild に併走させる案も棄却した。consumer がなく evidence 供給もないため verdict は常に fail-closed 系で、コストと複雑性のみ増える。`RoutingEngine` が selection を直接消費する案も棄却した。依存方向（workspace→player）に反し、engine は plan 実行に専念する設計が SF-03 slice5 で確定済みである（synvelis#514）。変更は `crates/synvelis-app/src/engine.rs` の module doc comment のみ（selection 非消費が意図的である旨と将来の配線 trigger を明記）で、dep 追加・テスト追加はない。rollback は doc 削除のみである。

stateless `PluginStateDecl` の表現は当初 workspace 側では不可能と判定された。`verify_plugin` の Live 条件が `digest.is_some()` を無条件要求し、`StateRestoreSupport` に「適用外」variant がなく、判定ロジックは player 内にあり workspace 側に verdict を変える lever がない（synvelis#514 → synvelis#530 で player 契約変更として解決）。stateless 契約は既存語彙の再利用で実現した: `PluginStateDecl::stateless()` predicate（`digest: None` + `quality: StateCaptureQuality::Unsupported` + `status != Known`）を定義し、env 側は既存 `StateRestoreSupport::Unsupported` を dual-use、`verify_plugin` の `Unsupported` arm のみで Live 経路を開く。`digest: None` の「stateless by design / 未 capture」は閉じた enum `StateCaptureQuality` の variant で型上区別され、未 capture は `stateless() == false` で fail-closed を維持する（synvelis#530）。

variant/field 追加案は全て棄却した: `capture_applicability` field（`quality == Unsupported` と論理同値の二重表現 + lease 外 `rcp07_gate.rs` の構造体 literal 破損で scope 拡張 ask が必須）、`StateRestoreSupport::NotApplicable`（既存 `Unsupported` が既に env 側 stateless を意味し供給経路 `HostError::Unsupported → PluginStateCaptureError::Unsupported` も実在）、`PluginVerdict` stateless variant（`Live` と意味同一 + lease 外 `render_ui.rs` の `_` arm なし exhaustive match 破損）。lease 外波及ゼロを優先する判断である。`SessionRoutingEngine` 配線は synvelis#514 で実装不要確定済みのため非変更とした（synvelis#530）。

### OS credential session の供給

durable `CliSessionStore` は `FilePermitStore`（`mcp_permit.rs`）と同型の digest-only JSON + tmp/fsync/rename 原子書込で実装し、open 時に Human-kind invariant を全 record に再適用して forged file を fail-closed にする。restart 跨ぎの expiry/revocation/一度きり返却を test で pin した（synvelis#515）。OS credential vault は `keyring` v4（`v1::Entry` API → `windows-native-keyring-store` → Windows Credential Manager）を `cfg(windows)` target 依存として採用した。`unsafe_code=forbid` のため windows-sys/DPAPI 直叩きは不可で safe wrapper が必須。service=`synvelis-cli`、username=session id、secret=password。keyring の deps は windows-sys/zeroize/byteorder/regex のみで GUI/GPU/WebView なし（synvelis#515）。

`--session <id>` は commands 層で vault から secret を in-process 読取し既存 `session_credential` claim 経路へ写像する。secret は argv/env/log に出ない。`--session`/`--session-credential` 同時指定は Usage error、非 Windows は `CliError::Unsupported`(exit 5)、vault miss は `cli.invalid-input`(exit 2)、record revoked は gateway `Authentication` で fail-closed（synvelis#515）。session provisioning の補償設計: `session issue` は store.issue 成功後 vault 書込失敗時に record を revoke し、live だが secret 未配備の孤児 session を残さない。`session revoke` は record revocation が権威で vault delete は best-effort（失敗しても record が fail-closed）。store 既定 path の env（`%APPDATA%`/`$HOME`/`XDG_STATE_HOME`）は場所解決のみに使用し権限証拠にしない（synvelis#515）。

棄却した代替: OS keychain を record backend にする案（trait の列挙/expiry 契約に不適合）、windows-sys/DPAPI 直叩き（unsafe forbid）、`--session-credential` のみ（argv 露出・AC 不充足）。対話存在証明（credential prompt/Windows Hello）は synvelis#114 系へ deferred で、本 slice は OS user account 束縛まで。model は `mcp-poc permit` と同型の operator provisioning である。task 文書は `status: "todo"`・checkbox 未チェックのまま残っているが、IR/exp は completed と記録しており文書 drift がある（synvelis#515）。

### mlua interim 実装から製品 backend への adapter 移行

製品 backend への移行は anti-corruption adapter（薄い binding）で行い、synvelis port 公開面（`synvelis_script::backend`）を維持する（synvelis#516）。`synvelis-script-luau` の interim mlua 直実装は trait 署名（by-ref vs by-value・`evaluate()` vs `evaluate(Option<&str>)`）・error enum payload・`SessionConfig` 形状・digest 形式（`sha256:` vs `sha256:v1:`）・artifact payload 形状が全面的に異なり drop-in 交換不可だった。棄却案: 製品実装の恒久的二重保有（interim 維持）、port 語彙の ruviform 型への再 binding（consumer 破壊 + 重複残存）である。

ruviform git 依存の pin 更新は workspace 全 crate に一括で効く。cargo が git source の revision を統一するため、`cbcc8343`→`84a68f25` の一括更新は必然的帰結であり差し戻し不可（PR 本文に明記）。旧 pin `cbcc8343` には `ruviform-script-luau` が存在しない（runtime backend `dce45712`/ruviform PR #729 以降）のため rev 更新は必須である。「意図しない広範囲変更」と疑ったら旧 pin での crate 存在確認を行う（synvelis#516）。

製品契約は interim より厳格な点がある: artifact payload は `BTreeMap`（Record 必須）で bare scalar/非 record は `Internal` 拒否。adapter は Record map を `ScriptValue::Record` で包み戻すが fail-closed は緩めず、test を record 形へ更新し bare scalar → `Internal` を検証する形に変更した。lifted-shape 違反（sparse/mixed table・depth/node 上限・non-record artifact payload）は製品側 `Internal` で、interim の `ContractViolation` から分類差がある（consumer は区別していないことを確認済み）（synvelis#516）。

中立 `ScriptErrorKind::BudgetExceeded` は次元 payload を持たないため port を `Option<BudgetKind>` へ拡張し、`budget` の `requested_kinds()` ∩ `enforced` が単一のときのみ `Some(kind)`、複数併用時は `None` で「不明」を正直に返す（推測しない）。`max_instructions`/`max_host_calls` は製品が host call 経路を持たず vacuous 受理するため adapter が `UnsupportedBudget` で拒否し、port 契約「要求次元は enforce 必須」を保持する（synvelis#516）。digest の扱い: consumer の `sha256:<hex>` tag は open 時に検証し、製品側 `sha256:v1:` は `rf::ScriptSource::new` が自動計算。artifact digest は製品 `DefinitionDigest` を使わず synvelis canonical `digest_of(to_json(payload))` で再計算（interim と同一式）。`produced_by.impl_version` には実際に VM を走らせた製品 backend の descriptor 値を記録する（synvelis#516）。実 Luau.Analysis native FFI bridge 切替は synvelis#274 gate、macOS/Linux 実機受入は synvelis#267 gate（NotRun）である。

### Git LFS の HTTPS/TLS transport

TLS transport（Git LFS `https://` remote）は rustls 0.23（default-features off、`std`/`tls12`/`ring`）+ `rustls-native-certs` で実装した（synvelis#521）。`rustls::Stream` は `prepare_read` が `wants_read` の限り内部 loop し `WouldBlock` を呼出側へ返さない経路があり得て cooperative `IoGuard`（cancel/deadline/stall の 250ms poll）を bypass する危険があるため、1 op = `complete_io` 1 pass + `reader().read` の手動 `TlsConn` を採用した。vendored rustls source を確認し `complete_io` は buffered plaintext がある間 `wants_read` を落とす（plaintext 喪失なし）ことを確認済みである。trust root seam は `ProviderContext.tls: Option<TlsTrust>` で、`None` は process 内 1 回 lazy の native root store（`rustls-native-certs`、0 件は typed error で fail-closed）、`Some` は `TlsTrust::from_pem` の明示 CA。rustls 型は公開 API に出さず検証強度は不変（trust anchor の選択のみ）。`session_for` は IP literal → `ServerName::IpAddress`、その他 DNS name である。

error 分類: `rustls::Error::InvalidCertificate` → `ProviderErrorKind::TlsVerification`（`RetryAdvice::Never`、enum を `#[non_exhaustive]` 化し `library_ui.rs` に `"tls-verification"` wire tag を追加 — Coordinator 承認の範囲拡張 option A）。その他の TLS/record 失敗は `TransientIo`。detail に cert bytes・peer data・credential・署名 URL を出さない（synvelis#521）。TLS transport は `HttpEndpoint.tls` opt-in の caller（Git LFS）のみが使用し、WebDAV `send()` は `https` を従来どおり `UnsupportedCapability` で拒否する。silent downgrade なし。WebDAV-over-HTTPS の解放は synvelis#90 の責務として残す（棄却案として記録）。`.lfsconfig`/`lfs.url` override・custom transfer adapter・SSH/`git://` transport は対象外である（synvelis#521）。

silent HTTPS→HTTP downgrade 対策: protocol-relative href/redirect は remote の scheme（href）/現在 target の scheme（redirect）を継承するよう修正し、https 有効化で新たに到達可能になった経路の downgrade を塞いだ（review で検出、回帰 test で固定）。`remote_parts` の scheme 大文字小文字正規化、`TlsConn::write` の非 timeout `complete_io` エラー即時伝播、`HttpEndpoint::from(&WebDavUrl)` の scheme-drop footgun 除去も実施した。TLS fixture は in-process `MockTlsServer`（rustls `StreamOwned` accept loop）+ rcgen 生成 CA/leaf で実 network・実 remote は使わず、request I/O を `Read + Write` generic 化し plaintext/TLS で共有する（synvelis#521）。

### production job system

production render-job system は `ruviform_exec::JobExecutor`（ruviform#744）ではなく `OfflineJob` worker thread 1 本 + bounded `sync_channel`（既定 cap 64）の actor 型 `JobScheduler` を採用した（synvelis#518）。`JobOrchestrator` が既に single-writer の scheduling authority であり、per-job executor primitive を被せると scheduling 責務が二重化するためである（ADR に記録）。`JobExecutor` の将来採用検討は orchestrator の scheduling authority 設計が変わる場合に限る。scheduler quantum は `drive_step`（1 stage 遷移 or 1 demand render）で、`drive()` を呼ぶと 1 job の全 demand render が終わるまで worker が専有され cancel/retry/grant update が飢餓する問題を round-robin candidate 選択で解決した。既存 sync API は `drive_step` の Progress loop として保存し呼出側契約を壊さない（synvelis#518）。

crash した job は自動再開しない契約である。crashed job が `state == Rendering` + `crashed == true` のまま残り worker の candidate picker がそれを再選択して `drive_step` を回し続ける欠陥を実装中に発見・修正した。`drive_step` が crashed job に `DriveOutcome::Crashed` を返し、`pick_candidate` が crashed を除外、`resume_crashed` が flag 解除 + `ReceiptKind::Resumed` 記録のみ行う（同期 drive しない）（synvelis#518）。bounded queue の admission 拒否 `SchedulerError::QueueFull` は worker 死ではなく backpressure（transient）であり、`wait_terminal` は deadline まで retry する。`JobScheduler::call()` は `reply_rx.recv()` で同期待機するため、worker が gate 付き executor 内で park している間に送った command は release まで reply できず、reply を await してから gate を開く test は deadlock する。scheduler の欠陥ではなく test 記述の問題で、helper thread から command を送り gate release 後に reply を await する形に修正した（synvelis#518）。

cost model: `JobCostEstimate::default()` の placeholder を deterministic `estimate_cost`（recipe・scope・measured/derived inputs 由来、wall-clock/RNG 不使用）に置換した。caller-declared estimate との合成は要素ごと max（model floor）で、caller が過少申告しても floor を下回れず budget/grant gate の fail-closed 性を維持する。明示 estimate を渡す既存 caller 経路は温存する（synvelis#518）。`ruviform_exec::OfflineJob` は `mpsc::Receiver` を保持し `!Sync` であり、`RenderDispatch` が `Send + Sync` を要求するため workspace adapter が `JobScheduler` 状態を `Mutex` で包む。`JobSystem::headless()` が composition root として default fail-closed ports + scheduler を生成し CLI / MCP PoC の `GateDeps.render` に配線した（synvelis#518）。fail-closed 既定: executor 無し→typed permanent unsupported、settings 無し→typed unknown dependency、policy 未宣言→built-in 解決、scope→demand mapping 無し→inert。render 成功を捏造しない（synvelis#518）。

schema v14→v15: `render_policy` 永続化に伴い `SCHEMA_VERSION` を 15 に bump（serde-default optional field + identity migration step + `#[cfg(test)]` round-trip test の既存 pattern 準拠）。`SCHEMA_VERSION == 14` を assert する leased test pin 3 件（`cb01_variant.rs`/`md_a.rs`/`tim_02_smf.rs`）は coordinator 承認（msg id 記録）のもと pin 行のみ 14→15 に更新した。pin は schema bump のたびに意識的更新を強制する canary として機能する。実 `RenderExecutor`/settings/demand resolver の production 実装は未存在で、headless host は fail-closed typed rejection のみ返す（render 成功を主張しない）。IR status は `in-progress` のままで、mise 正規検証・prek・review-protocol は PR 前実施と記録されている（synvelis#518）。

### impl index と IR 文書衛生

`docs/impl/ir`・`docs/impl/exp` の管理 index は `doc-maintenance` の `index plan`/`index apply` で再生成可能にした（synvelis#365）。index の source 検証を落としていた ir 文書 44 件の front matter を impl-doc schema 準拠に機械的正規化（`type: "ir"`→`"impl"`、`status: "done"/"final"`→`"completed"`、`updated` を `YYYY-MM-DD` に正規化、`changes` 裸文字列→`{type,path}` object、欠落 `relations`/`metadata.experiments` 補完、list 形式 experiments→`adopted` 配列+`rejected: []`）し、README を generated-index marker 付き stub（手書きタイトル + Directory 行 + 空テーブル）に置き換えて owner sync（`sync_impl_index`）で全行再生成した。手書きタイトルは preamble として保持される（synvelis#365）。

schema 是正で未検出だった `missing-section` finding が新規可視化された（audit error 319、baseline 331 からは減少）。IR audit は schema 検証通過後にのみ本文 section 検査を行うため、front matter 修正が旧形式文書の本文不備を炙り出す副作用がある。本文変更は範囲外とし error 総数が baseline を下回ることをもって受入とした（synvelis#365）。既存 README の表は行リンクが `[file](file)` 形式で parser が要求する `[file](./file)` と非互換かつ手編集禁止の生成物のため、表を保持せず空テーブル stub から再生成させた。`mergeManagedIndex` が preamble（タイトル）と tail を保持するため手書き部分は失われない（synvelis#365）。

`docs/impl/ir` の missing-section 保持 60 文書は必須 `##` セクション 7 件（Summary / Context / Implementation / Related Experiments / Validation / Risks / Follow-ups）を全件に揃えて `missing-section` を 319→0 にした（synvelis#368）。既存の非正規見出し（`## 実装内容`/`## Verification`/`## 残件`/`## Remaining` 等）は内容を保持して改題・再配置し、欠落セクションは front matter relations が指す上流文書（spec/design/plan/task/test-spec）で事実確認してから記述した。大量文書補完のガード: Related Experiments は `metadata.experiments` が空かつ `docs/impl/exp/` に関連ログが無いことを確認した文書のみ「なし」と記述し、存在しないログへの参照を作らない。「定型文で埋めない」受入条件に対し空セクション・TODO マーカーの機械検査（60 文書・問題 0 件）と協調・敵対 2 系統の独立レビューで担保した。audit が要求しない非正規 `##` セクション（`## Decisions`/`## Notes`/日付付き追補）は削除せず保持した（synvelis#368）。

60 文書の本文補完は 11 の並列作業単位（prefix 別バッチ）に分割し各単位が front matter relations と上流文書を参照して記述、監査・検証は親側で一括実施する分担が機能した（synvelis#368）。残件として `missing-experiment-link` warning 18 件・`broken-relation-link` warning 36 件（いずれも front matter 由来）と exp jsonl `invalid-event-shape` error 52 件は未解消で別 Issue 候補であり、task status 表記揺れ（`in_progress` 等 3 件・`completed` 3 件）も範囲外として残存する（synvelis#365・#368）。

### 文書チェーン・leased-file・正直な記録の運用規律

doc chain の慣行: discovery → specs → designs → plans → tasks → test-specs → impl（ir/exp）→ evidence の順で文書を作り、`docs/**/README.md` の管理 index は Coordinator が統合時に再生成する。worker は index の編集・`index apply` 実行をしない（複数 slice で「別 worker 占有」「docs index 未同期は意図的」として明示）（synvelis#262・#274・#283・#286・#324・#516・#530）。leased-file 規律: lease 外ファイルの変更は scope 拡張 ask / coordinator 承認が必要である。実例: synvelis#530 で `rcp07_gate.rs`（lease 外）の構造体 literal 破損を避けるため variant/field 追加を棄却、synvelis#518 で `SCHEMA_VERSION` pin 3 件の更新に coordinator 承認（msg id 記録）、synvelis#286 で `synvelis-workspace/src/lib.rs` の root re-export を synvelis#518 占有範囲・ask 必須の違反として revert（compile 上不要と判明したのも理由）（synvelis#286・#518・#530）。

「正直な記録」の規律が繰り返し登場する: 未検証 lane は NotRun/Deferred/Unsupported として明記し、成功・証拠・対応を捏造しない。実験が無い slice では exp.jsonl に start/summary のみ記録して「no experiments」を明示する（synvelis#274）。外部 gate（synvelis#114/#267 の製品 E2E・3OS 実機・GUI 受入、Daily Full CI の `*-full`/`*-msrv`/`gw10`/`test-rt`/`check-app-*` lane、ruviform 側 gate ruviform#560・rf#692 等）は別 Issue/別 lane に帰属させる（synvelis#262・#274・#515・#516・#521・#530）。canonical 検証は mise タスク（`fmt-check`/`check`/`lint`/`test`/`build` を package 指定、`check-headless-deps`、`prek run --all-files`）で行う。Windows worktree では aggregate `mise run fmt-check` が環境起因の `os error 206` で fail することがあり、全 member の個別 `cargo fmt -p -- --check` pass をもって代替証拠とした先例がある（synvelis#286）。experiment log は `experiment_event.v1` schema の jsonl で start/hypothesis/observation/decision/change/validation/error/result/summary event を記録し、exp 文書の front matter `metadata.experiments.{adopted,rejected}` と対応させる（synvelis#262・#286・#514・#515・#516・#517・#518・#521・#530）。review-protocol（協調・敵対の独立レビュー）を PR 前に実施し blocker 0 を承認として扱う運用である。docs-only 変更では製品コード・テストの再実行は対象外（synvelis#283・#286・#365・#368・#514・#530）。

### fail-closed 契約と検証の型

`unsafe_code=forbid` と GUI/GPU/WebView 非含有の headless 制約下では、OS credential は `keyring` 等の safe wrapper、TLS は rustls、ファイル永続化は `StagedFile`/tmp+fsync+rename の既存 primitive に委譲し、unsafe FFI・独自 atomic rename/durability を再実装しない（synvelis#515・#517・#521）。schema migration の既存 pattern: serde-default optional field + identity migration step + `SCHEMA_VERSION` increment + `#[cfg(test)]` round-trip test。version bump 時は `SCHEMA_VERSION` を assert する既存 test pin の意識的更新が必要である（synvelis#518）。

fail-closed 契約テストの型: typed reject の一様性（ordinary op + stale pin）で表現不能な経路を証明し、境界の fail-closed（routing/external op）を別証拠とする。select-all fixture が新 kind を拾う副作用は apply-surface kind 絞り込み helper で fixture 側に反映し、apply 挙動自体は変えない（synvelis#283・#324）。fault fixture の再利用可能なカタログ: 空の policy store（全 capability denied の正直な golden）、forged session record（Human-kind invariant 再検証で fail-closed）、untrusted TLS certificate（`TlsVerification`/`Never`）、crash 済み job（明示 resume 必須）、queue 飽和（`QueueFull` は transient backpressure）、stale revision/digest pin、project-root 衝突（identical bundle copy）、restore source 不在（per-reference unavailable verdict）（synvelis#262・#283・#286・#515・#517・#518・#521）。oracle/契約テストの型: verdict truth table を oracle case として pin する（stateless/未 capture の全組合せ）、wire-level の replay 分類（同一 key+同一 payload→determined replay、異 payload→conflict）、capability floor の実効検証（宣言 metadata でなく wire permission + scoped effects）を pin test で固定する（synvelis#517・#521・#530）。

実バイナリ E2E の型: Windows headless で `synvelis-cli` 実行体を spawn し typed envelope と exit code を検証する。fixture は saved bundle の `project_dir` を要求し、live registry の関数名/arity を fixture に固定せず completion `detail` から導出する 2 段構成で結合を緩く保つ（synvelis#262・#274）。in-process fake server の型: 実 network を使わず `MockServer`/`MockTlsServer`（rustls `StreamOwned` accept loop）+ rcgen 生成 CA/leaf で TLS 経路を決定的に検証する。request I/O を `Read + Write` generic 化して plaintext/TLS で共有する（synvelis#521）。非同期 scheduler の test 記述の教訓: 同期 `call()` が reply を `recv()` で待つ設計では、worker が gate 内で park 中に送った command の reply を await してから gate を開くと deadlock する。helper thread から command を発行し gate release 後に reply を await する形にする（synvelis#518）。

### 境界・依存・永続化の規律

依存方向の規律: `synvelis-app` は gateway 型を名指しできず workspace facade 経由、`RoutingEngine`（player 側）は plan 実行に専念し selection を消費しない、`synvelis-script` は命名規則を知らない汎用 crate — 境界を越える知識は facade/adapter 層に閉じ込める（synvelis#262・#286・#514・#516）。content addressing の性質（同名・別 bytes は honest writer では起きない）を publish race の解決に使い、派生 receipt は staging 時点でなく publish 後の実在から再計算する型が render/branch pipeline で確立した（synvelis#517）。composition root での optional port 注入の型: `VariantPorts::standard(Option<&Path>)` のように durable/read-only boot と discovery で `Some`/`None` を分け、render dispatch は `UnavailableRenderDispatch` を default として CLI/MCP PoC に配線する。`JobSystem::headless()` も同様に default fail-closed ports + scheduler を一箇所で生成する（synvelis#517・#518）。adapter 登録の権威を一箇所に固定する型: facade 側で手動 install せず `VariantPorts::standard` を authoritative registration point とし、上流（synvelis#539/#542）で登録済みの adapter を再利用する。二重登録・stale 前提の install コードは除去対象である（synvelis#286）。

schema pin test を意図的 canary として使う型: `SCHEMA_VERSION` を assert する既存 test は bump のたびに fail して意識的更新を強制する仕組みとして機能する。pin 更新は coordinator 承認を得て pin 行のみ変更し、migration 本体と round-trip test は既存 pattern（serde-default + identity step）に従う（synvelis#518）。依存 pin 更新時の確認手順: ruviform git 依存の rev 更新は workspace 全 crate に一括で効くため差し戻し不可。更新が本当に必須かは旧 pin の git rev で対象 crate の存在を確認して判断する（`cbcc8343` には `ruviform-script-luau` が無かった）（synvelis#516）。

## 適用対象

この知識は Synvelis の CLI・gateway・workspace・render・project・library・script 系 crate の変更に適用される。headless CLI の実行配線と envelope/exit code 契約、script analyze の既存 seam 消費、variant diff gateway exercise と Difference Explorer app wiring、VCS semantic diff の分類規約と `DIFF_POLICY_VERSION` bump、render/branch pipeline の本番配線と `JournaledRuntimePublish` の `Pending` 契約、session engine selection の非消費判定と stateless `PluginStateDecl` 契約、OS credential session store と `keyring` vault、mlua interim から ruviform 製品 backend への anti-corruption adapter、Git LFS `https://` transport、production job system の actor scheduler、impl index/IR 文書衛生、およびそれらに共通する fail-closed・leased-file・正直な記録の運用規律が対象である。主に synvelis-cli・synvelis-automation・synvelis-gateway・synvelis-workspace・synvelis-render・synvelis-project・synvelis-library・synvelis-script・synvelis-script-luau・synvelis-app・synvelis-player の変更時に参照する。

## 出処

この知識は、headless CLI 実行配線・script analyze（synvelis#262・#274）、variant diff gateway exercise・Difference Explorer app wiring（synvelis#283・#286）、plugin_instances VCS semantic diff（synvelis#324）、impl index/IR 文書衛生（synvelis#365・#368）、session engine selection・stateless PluginStateDecl（synvelis#514・#530）、OS credential session（synvelis#515）、mlua→ruviform adapter（synvelis#516）、render/branch pipeline 本番配線（synvelis#517）、production job system（synvelis#518）、Git LFS HTTPS/TLS transport（synvelis#521）の各作業記録から抽出した。関連 Issue は synvelis#262・#274・#283・#286・#324・#365・#368・#514・#515・#516・#517・#518・#521・#530 である。
