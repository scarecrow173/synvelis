---
id: "KNOW-qwZQk6bPt7ryp6OiVZNgII"
type: "knowledge"
status: "draft"
lifecycle: "draft"
knowledge_kind: "operation"
title: "CI と検証プロセス"
created: "2026-11-19"
updated: "2026-11-19"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/83"
    - "https://github.com/scarecrow173/synvelis/issues/86"
    - "https://github.com/scarecrow173/synvelis/issues/115"
    - "https://github.com/scarecrow173/synvelis/issues/131"
    - "https://github.com/scarecrow173/synvelis/issues/145"
    - "https://github.com/scarecrow173/synvelis/issues/147"
    - "https://github.com/scarecrow173/synvelis/issues/148"
    - "https://github.com/scarecrow173/synvelis/issues/151"
    - "https://github.com/scarecrow173/synvelis/issues/154"
    - "https://github.com/scarecrow173/synvelis/issues/157"
    - "https://github.com/scarecrow173/synvelis/issues/166"
    - "https://github.com/scarecrow173/synvelis/issues/167"
    - "https://github.com/scarecrow173/synvelis/issues/177"
    - "https://github.com/scarecrow173/synvelis/issues/184"
    - "https://github.com/scarecrow173/synvelis/issues/190"
    - "https://github.com/scarecrow173/synvelis/issues/230"
    - "https://github.com/scarecrow173/synvelis/issues/245"
    - "https://github.com/scarecrow173/synvelis/issues/377"
    - "https://github.com/scarecrow173/ruviform/issues/238"
    - "https://github.com/scarecrow173/ruviform/issues/329"
    - "https://github.com/scarecrow173/ruviform/issues/330"
    - "https://github.com/scarecrow173/ruviform/issues/631"
  informs: []
  explains: []
  supersedes: []
---

# CI と検証プロセス

## 概要

Synvelis の CI は独立構築から始まり、verify・rt-verify・gw10-verify・msrv-verify・dep-guard・evidence の 6 lane に ui-verify を加えた構成を経て、最終的に `workflow_dispatch` 専用（manual-only、`expected_sha` input 付き）へ移行した。検証ロジックは workflow に書かず mise task へ委譲し、evidence 系 job が全 lane の結果を fail-closed に集約する。impact 分類による選択実行は Shadow Mode で偽陰性を監査した後、毎回全 lane を実行する Full CI へ方針転換した。コンパイルキャッシュ（sccache）、npm 依存の freshness 管理、flake の構造的修正、runner 運用、検証 oracle の設計には一貫した規約があり、未実施の検証は NotRun/NotRequired として理由付きで記録して実績と混同しない。

## 詳細

### CI workflow の lane 構成

`.github/workflows/ci.yml` は verify（3 OS matrix）・rt-verify・gw10-verify（3 OS）・msrv-verify・dep-guard・evidence の 6 lane で構成され、後に ui-verify（3 OS、synvelis#151）が追加された。全 lane は検証ロジックを workflow に書かず mise task へ委譲する。共通の job env は `MISE_ENABLE_TOOLS`・`CARGO_INCREMENTAL=0`・`CARGO_NET_GIT_FETCH_WITH_CLI=true`・`GIT_LFS_SKIP_SMUDGE=1` で、ui-verify のように job 単位で `MISE_ENABLE_TOOLS` を override して node を足す前例がある。

required contexts は 10 件で固定されており、job 名の変更・削除は ruleset 影響を伴うため不変とされた。新 lane を required contexts に直接追加せず、`evidence` 集約経由で fail-closed を実現する手法が取られた。Ruleset 変更を伴わずに済み、個別 required 化は Baseline 設計側の判断として defer した前例がある。

各 lane の役割は次のとおりである。dep-guard は `cargo fetch --locked`・manifest 表記ガード・単一 SHA ガードで Cargo.lock の ruviform 解決が単一 SHA であることを fail-closed に検証し、`ruviform_sha` を output する。rt-verify は `cargo test --workspace --locked rt_` の strict-prefix 非空ガードを持ち、`-- --list` で全 test binary を列挙して `rt_` テストを計数し、0 件は fail とする。この列挙 step は全 test binary のコンパイルを伴うため job 支配コスト（実測 56s）であり、本試験自体は同一 target dir の fingerprint 再利用で 4s に過ぎない。msrv-verify は rust@1.89.0 を別途 install して `test-msrv` を実行する。evidence lane は `needs` 全 lane・`if: always()` で走り、run_id/commit/ruviform SHA/OS/toolchain/lane 結果/NotRun 行を含む証跡テーブルを出力し、証跡ゼロを fail とする fail-closed 集約である。

OS-L1（native/gpui-test feature lane、synvelis#377）では、新規 job を作らず既存 verify/verify-macos leg に step を追加する方式を採り、`tools/ci-summary` の leg universe と gate 契約を不変に保った。feature lane 追加に伴い verify/gw10-verify の leg timeout は 40→60 分へ引き上げられ、Linux leg の gpui-test には `libxkbcommon-dev`・`libxkbcommon-x11-dev` の apt install が前提となった。

### evidence gate と impact 分類の進化

evidence gate の fail-closed 化は、`needs.*.result` aggregate だけでは `skipped` を見逃す fail-open 経路があったため、jobs API で leg 単位の Pass/Fail/NotRun/Skipped/Cancelled を照合する方式へ改修された（synvelis#148）。未知の job 名は unclassified violation として fail する。

選択実行（impact 分類による lane skip）はまず Shadow Mode で導入された。分類と期待集合の宣言のみ行い全 lane は実行し、would-skip leg の実結果と照合して偽陰性を監査する方式である（synvelis#148）。分類器は LANE_LEGS（job 名 universe）・PATH_RULES・CRATE_DEPS/CRATE_CLASSES・RT_OWNERS/GW_OWNERS・DECLARED_NOT_RUN を持ち、`classify`/`gate`/`lanes` subcommand を提供した。`classify` job は fetch-depth 0・node のみで `git diff base...head` を行い、base 未到達・push の zero `before` は tool 側 fallback で full CI に倒す。jobs API の収集には `actions: read` 権限が必要で、evidence job に job 単位で追加した（他 lane 不変）。

matrix lane が skip されると GitHub は `<lane> (${{ matrix.os }})` という placeholder job を jobs API に残し、これが unclassified violation → evidence fail を起こした（synvelis#157。PR #161 / run 41894049161 で「required 全緑・macOS queued でも evidence fail」として再現）。placeholder の修正では「同 lane の全 leg が report 上 scheduled===false かつ verdict=skipped かつ同 attempt で leg に置き換えられていない」の 3 条件でのみ正当と判定する。判定の正本は `expectation` ではなく `scheduled` フラグである（job-level `if:` は lanes 出力を消費するため）。placeholder の式内容照合は spec/design の Deferred どおり不採用で、名 prefix + lane 状態で十分 fail-closed と判断した。期待件数は `expected-jobs` サブコマンドが schedule 状態から算出し（13/14/11）、`lanes` が LANE_LEGS 全 name を覆わない report は形式不正として最大 universe 14 へ fallback する。under-expect（欠落見逃し）を構造的に排除する設計である。jobs API の収集は `--paginate` + slurp が必要で、`per_page=100` 非 paginate では `filter=all` の 100 件超過時に古い attempt 側で打ち切られる fail-open 余白があった。

後続の Full CI 化（synvelis#166）で方針は「毎回全 lane 実行」へ転換された。分類・期待集合・shadow・placeholder 受理の経路を削除した `tools/ci-summary.mjs` に fail-closed verdict 規則のみを移植し、旧 `ci-impact` 資産と関連 spec 3 件は削除・superseded 化された。ruleset required の `evidence` job は macOS 非 required の互換 gate として維持され、新設の `ci-summary` job が full 集約を担う。lane `if:`・`needs` による選択実行は撤去された。`evidence` job 失敗時の ci-summary は required として violation になり fail する（gate infra 障害を成功にしない意図どおりの挙動）。

`workflow_dispatch` の `expected_sha` 契約は「expected_sha == github.sha == checked-out HEAD」の 3 点一致を `verify-sha` サブコマンドで fail-closed 検証する。warning のみの比較では stale dispatch（ref が進んだ場合）を「別 workflow revision の検証結果を候補 SHA の Full CI 成功」と誤認し得るためである。全 lane の `commit:` 証跡は `git rev-parse HEAD`（実際に検証したコミット）を記録する。`CI_BASE_SHA` が空に解決されるケースは report context 記録のみで gate 判定に影響しない（ci-summary の mode 契約で検証済み）。

### ruviform 依存アクセスと vendor 同期

ruviform への git 依存アクセスは `ruviform-dep-access` composite action が解決する。anonymous HTTPS（public repo 前提で secret 未設定でも全 lane が動く）と deploy_key + host key fingerprint pin + `url.insteadOf` + `GIT_SSH_COMMAND` の 2 mode を持つ。secret は composite action の inputs 経由で受け取り `secrets` context を直接参照しない。両 mode が不可なら `::error::` + fail し、`always()` cleanup を各 lane に置く。

`Cargo.lock` の ruviform 解決 rev が変わる変更（pin 更新・`cargo update`・ブランチ切替）を取り込んだ後は `mise run sync-ui-deps` で gitignore された生成物 `ui/vendor/ruviform` を再同期する必要がある（synvelis#230、#115）。vendor は `.worktreeinclude` で Orca worktree へそのままコピーされるため、pin 更新後に作られた worktree では stale vendor が混入し `check-ruviform-rev` が REV/lock 不一致で fail する。この fail-closed は正しい検出であり、復旧は `sync-ui-deps` の再実行である。`sync-ui-deps` は `~/.cargo/git/checkouts` の cargo fetch 済み checkout を参照するため、ui-verify lane 内では `cargo fetch --locked` を先に実行する前提がある。`sync-ui-deps` を UI task の `depends` に入れる案は、fetch 前環境で lane が壊れるため採らなかった。

pin 更新の影響判断には、cargo git checkout の実コードを新旧 pin で byte 比較する手法が使える（synvelis#190）。`update_view` 経路の関数が byte 同一なら旧 pin の実機証拠を現行 pin の契約証拠として採用できると判断した前例がある。

### sccache とコンパイルキャッシュ

sccache v0.19.0 を GitHub Actions cache backend で verify・gw10-verify（各 3 OS）に導入した（synvelis#147）。導入 step は cargo cache restore 直後・検証 step 群の前後に「Set up sccache → env export → env verify → statistics」の 4 step を置く。`mozilla-actions/sccache-action` と `actions/github-script`（ACTIONS_RUNTIME_TOKEN/ACTIONS_RESULTS_URL の再 export）は SHA pin する。

`SCCACHE_GHA_VERSION` は lane 名を含めず `synvelis-v1-<RUNNER_OS>` の OS 別 namespace のみとし、同一 OS・同一 toolchain の lane 間で entry を共有する。sccache の cache key（compiler hash・全引数・ソース fingerprint）が feature/profile/toolchain 差を分離するため lane 間共有は安全で、warm の gw10 ubuntu 100% hit が verify 書込みを含む共有の実証になった。kill switch は repository variable `CI_SCCACHE_DISABLE=1` で全 sccache step を skip させ通常 compile へ fallback する設計である。cache 障害時も fail-open で全 lane green を維持し、namespace を `synvelis-v1` → `v2` に上げれば cache 全破棄も可能である。

env verify step は version・`RUSTC_WRAPPER`・backend 変数・credential 存在を確認する fail-closed で、cache 未接続なら失敗する。statistics step は `sccache --show-stats || true` で統計失敗で lane を落とさない。`RUSTC_WRAPPER` は GITHUB_ENV 経由で後続全 step に効くが、sccache は rustc wrapper であり cargo/test 実行・証跡には関与しない。hit しても check/clippy/test は全て実行される。

rt-verify/msrv-verify/ui-verify への sccache は段階展開として不導入（deferred であり不採用ではない）。MSRV は rust 1.89.0 と通常 1.99.0 の toolchain 成果物誤共有を構造的に排除するため、ui-verify は npm install/test/build 主体で Rust compile 対象が限定的なため（ruviform でも ui lane の hit 率は低く分散大）である。rt-verify は列挙コンパイル 56s が支配コストで、stage 2 では `synvelis-v1-Ubuntu` namespace で verify の test compile 単位と大部分を共有できる見込み（厳格 non-empty 選択の維持を条件）。

先行評価（ruviform #329/PR #342 で verify hit 47–62%・job 10–30% 削減・kill switch 検証済み）の結果は転用せず、OS/toolchain/profile/Ruviform SHA が異なる Synvelis で cold/warm 実測して採否を判定した。採用判定は verify・gw10-verify ともに採用（warm hit 75–100%、Before 中央値比で verify 全 OS の wall 改善）で、gw10 windows/macos の wall 微増は誤差範囲・critical path 外と判断し、OS 別条件分岐を入れず全 OS 統一で採用した。

### npm 依存の freshness 管理

npm 依存導入は freshness-checked `install-ui` task に集約する方式を採用した（ruviform #330 先例、synvelis#151）。`sources = ["ui/package-lock.json", "ui/package.json"]`・`outputs = ["ui/node_modules/.package-lock.json"]` で mise の freshness check を有効化し、`npm --prefix ui ci --no-audit --no-fund` を唯一所有させる。`setup-ui` は `depends = ["install-ui"]` の集約入口にして呼び出し側互換を保ち、npm 消費 task（check-ui/test-ui/build-ui/dev-ui/build-app）は `depends` で再利用する。

npm cache（actions/cache での node_modules 保存）は不採用とした。install-ui の freshness skip で十分であり、cache 破損時の復旧経路を増やさない判断である。install-ui freshness は node_modules の部分破損を検出しない既知の前提で、破損時は `npm ci` 手動再実行で復旧する。

### CI flake の診断と修正

TCP control port の flake は `free_port()`（bind→即 drop で「空き」を probe）→ child bind の TOCTOU が原因で、retry 増加や timeout 延長では「正しいサーバへの接続」という不変条件を回復できない（synvelis#154）。修正は `mcp-poc serve --control-port 0` で kernel が原子的に port を選択し、実 port を stderr の readiness 信号（`control-port=<N>`）で報告、bind 失敗は `fail()`（exit 1）にする方式で競合ウィンドウを構造的に除去した。テスト側の `control()` は報告済み port へ直接 connect し retry loop を除去した（旧実装は bind 失敗が silent return で「自分の listener が無い」ことすら区別不能だった）。port 選択プロトコルのような「選択→使用」の 2 段構成は、使用側（bind する側）が port を確定して報告する設計に変えるのが正しい境界修正である。silent return/bind 失敗の握り潰しも fail-closed 化が必要で、readiness 報告の上限超過は child stderr 末尾を含む正直な panic にする。

macOS runner 枯渇時の運用は、queued job を殺さず待機させ、queue が捌けてから `gh run rerun --job` で 1 件ずつ直列 retry する（synvelis#131）。run-level cancel や並列 rerun は他の queued job を巻き込んで殺す。queue 20–25 分で GitHub 側 auto-cancel（0 step 実行）された job は環境制約の NotRun であり、変更起因の失敗ではない。required leg が cancelled なら fail-closed の evidence gate は正しく failure になる。

テスト fixture が返す temp ルートは `fs::canonicalize` で canonical spelling に揃えるべきである（synvelis#184）。非 canonical 綴り（macOS `/var`、Windows 8.3 名・junction）では symlink 契約テストが成立せず、junction 経由 TEMP で同一 4 件の失敗を再現して原因クラスを確定した。canonical 化で否定系テスト（escape/unavailable 系）の検証経路が「真の逸脱」判定になり、むしろ検証として強くなる。

alloc 計測テストの warm-up は計測対象インスタンス自身に対して `Region` の外で行う（synvelis#245）。使い捨てインスタンスの warm-up は lazy-init を温めない。`SuppressingSink` の watermark 初期値に合わせ generation 0 の warm event を選ぶと watermark を進めず gate を通過する最小 warm emit になる。generation ≥1 の warm event だと計測窓の `ev(1)..` が stale-drop され queue 投入経路の計測が弱まる。undrainable 残留で空き slot が減る分は full-queue drop 分岐の追加カバレッジで相殺し、warm event を 1 件に限定するのは inner queue への drain 経路を公開しない sink では残留が不可避であり 1 件で first-touch をカバーできるためである。

単一の green run は flake 除去の証明にならない（synvelis#245）。macOS ローカル検証が不能な flake 修正は「契約内の対症的対処」として記録し、flaky 再発時に発生源の同定を別途行う運用とした。既存の flake（`gw10f_delegated_apply_commits_and_survives_restart_real` の Connection reset by peer）は sccache 導入とは無関係の scope 外失敗として rerun で対処し記録する運用だった（synvelis#147）。

### 検証手法と oracle の設計

`ruviform-audio-test` の scripted backend（`ScriptedBackend`/`DeviceSpec`/`PcmScenario`/`StreamScript` faults/`VirtualClock`）を `&dyn Device` 経路で `RealDeviceRig`/`InputSession` に接続すれば、fake deliver を介さず `pump_input_next`（backend pump + scenario fill + callback 発火の一体化）で deterministic PCM を production recording 経路へ供給できる（synvelis#115）。`RealDeviceRig::open` が既に `&dyn Device` を受け `ScriptedDeviceHandle`（Clone で state 共有）がそのまま使えるため `tests/common/mod.rs` への glue 追加は不要だった。delivery は `pump_input_next` が callback 発火を兼ねるため fake `deliver` とは別経路であり、共通化せず test file 内に閉じた helper とした。

検証の厳密さは journal `ChunkRecord`/`spans` で media byte を index → `pcm_reference_block`（絶対 stream frame・segment 単位）→ `pcm_assert_eq`（Sine は `AbsEpsilon(1e-5)`、他 pattern は `Exact`、ruviform `SINE_EPS` 規約踏襲）で行う。`verified_sha256` は `sha2` で独立再計算する。`PumpReport` の position/reported_frame_position 連続性・`clock_domain` 一致も断言する。末尾端数は `StreamFault::Frames` で当該 callback の frame 数を残りに一致させて厳密に駆動する。

RT 計測テストは別 test binary・単一 `rt_` prefix テスト・warm-up 済み `Region` の規約に従う。global allocator 統計と無関係の functional test は rt_alloc binary に置かない。`rt_` test を追加しない slice では `test-rt` は条件非該当として NotRun 記録する。fault 注入は commit→reload 越しに検証する。scenario gap は Xrun（`gap_frames`/`errors_fired` を `PumpReport` が報告）→ `FAULT_INPUT_DISCONTINUITY` → verified prefix = gap_start、`FramePosition` jump は `first_input_frame` 不連続で fail-stop、`ClockDiscontinuity` → `FAULT_EPOCH_DISCONTINUITY`、vanish（`StreamInvalidated`）→ `FAULT_INPUT_DISCONTINUITY` と対応づく。persistent open error は `InputBindError::Device` の defined rejection → clear 後 recovery で、persistent error は clear まで全 open を拒否し失敗 open は device slot を残さない（backend `open_input` の fault 評価順: persistent → once → DeviceBusy）。

test helper の拡張は既存経路の再利用を優先する。`rf628_record` を `pre_open` hook 付き wrapper 化するだけで open-fault recovery の record→commit→reload を本番 rig 構築経路のまま検証でき、private 関数の複製（~50 行・ドリフト温床）を要する手組み rig 案は不採用とした。既存 verdict を書き換えず gap を閉じる別 test として追加する方針も取った。`expect_err` は対象型が `Debug` 非実装だと使えないため `let...else` + panic にする（clippy `manual_let_else` 準拠）。rejection 未発生時に panic する設計にすれば silent pass を防げる。

parity test では、`DrawnPlan` が色を持たない（theme/fallback の plan が同一）ことは欠陥ではなく観測限界であり、gpui-test lane の parity 主張は `ResolvedTheme` 解決値と `ViewMount.theme` mount 受理の範囲に留めて「plan が同一であること」を negative test として assert するのが正直な記録になる（synvelis#177）。design token の canonical 語彙化は whole-value DTCG alias で `color.*`/`spacing.*` primitive を正本に保ち、第二の interpreter を作らない。`ViewPalette` は private のため parity は公開境界（`ResolvedTheme` accessors + `ViewMount.theme` mount）で証明し、upstream 変更は scope 外とした。provisional 値は文書・evidence で「design review 未確定」と明示して確定値と区別する。

verified import の境界では、`import_verified` は byte-integrity のみ検証して `Imported` で commit するため、decode 検証と `Verified` への state 前進は consumer 責務である（synvelis#86）。`open_blob_verified` が digest-kind 3 分岐（`Sha256`→`blobs/sha256/<hex>`+sha2 再計測・`Crc32Fold`→flat `blobs/<digest>`+crc fold・`Opaque`→`ResolveError::UnsupportedDigest` fail-closed）で検証済み `File` handle を返し、verify→open 間の path-swap ギャップを排除する単一権威である。`SetSourceState` は `Verified` 止めとした（`Committed` まで進めると 2 command = 2 undo step で旧経路の 1 command = 1 undo step と undo 粒度が変わるため。playback gate は `>= Verified`）。`Opaque` digest は fail-closed（未知 digest を検証済み content address として解釈しない。legacy CRC は byte-level 検証のみで強識別子へ昇格しない）。manifest replay では記録済み target が優先され request の target は無視されるため、clip 付き import は初回呼び出し時に target を渡す必要がある。

### 共有 CI・文書・検証の運用規約

共有 CI 実装（Ruviform 所有の evaluator）は Consumer 側で `ref:` pin した SHA の tarball/checkout を使い、mode/guard/pin/入口契約を `node --test` の契約 test で固定する（synvelis#167）。契約 test は YAML パーサ非依存の構造 assertion で書く。reconcile 入口は `schedule` + `workflow_dispatch` のみの別 workflow とし、既存の `workflow_run` 専用契約を変えない。共有 evaluator の `--reconcile`・guard・Issue 冪等は pin の中身に委譲し Consumer 側で再実装しない。tracker reconcile の新 workflow は `permissions: {}`・同一 concurrency group・job guard・15 分 timeout・最小権限・2 段 checkout + self-test + `tracker.mjs --reconcile --report` + 別名 artifact の構成で、GITHUB_TOKEN dispatch 完了の通知 gap を埋める。cron 未観測を手動 run の成功に混同しない。

文書チェーン規約は spec→design→plan→task→test-spec→impl/ir→impl/exp の順で文書を起こし、`docs/*/README.md` index は doc-maintenance（`plan --type all`/`apply`/`sync_impl_index`）で再生成し手編集しない（synvelis#115、#145、#167、#190）。検証の canonical は `mise run` で、`cargo test` 直接実行は補助証拠である。実行していない検証を PASS と書かず NotRun/NotRequired は理由付きで記録する。evidence 記録は `evidence/<issue>/` 配下に case matrix・ledger・検証記録を置き、実測 verdict で更新する。case matrix は lane 列ごとに正直に NotRun/NotRequired/Deferred を記録し、未観測を Pass にしない。

worktree 内作業の境界として、GitHub 書き込み（merge・Issue close・trigger 撤去・ruleset 変更）は coordinator の責務として worker スコープ外にし、実 CI run が取れない場合は jobs API 契約のローカル再生・合成 JSON で代替検証して「GitHub 実 run 未実施」を明記する（synvelis#166、#167、#145）。ruleset は read のみで変更しない。

依存方向の制約は `player → library → project`、`render → player` の非循環を保ち、blob 解決のような同一責務の重複実装は単一権威（`open_blob_verified`）へ集約する（synvelis#86）。review-protocol の指摘は全て修正してから進める実績がある（synvelis#157 で A-1〜B-6 の P2/P3 指摘を修正）が、一方で「本変更非因果の既存ギャップ」（attempt-masking 等）は規約どおり未修正で記録のみに留め、後続 issue 候補とした。task/plan の front matter status と IR の Validation 記録が一致しない文書が複数あり、「文書が一致しない」と記録してどちらかを正と断定しない扱いとした。ruviform 側の gap（`RunMetadata` JSON serializer 欠如・`InputPump` drift リスク等）は Synvelis 側で恒久 workaround を作らず、必要になれば ruviform Issue を起票する方針である（synvelis#115）。

## 適用対象

この知識は Synvelis の CI・検証・flake 対応・文書衛生の工程全体に影響する。`.github/workflows/ci.yml` と関連 workflow（tracker 系）、`tools/ci-summary.mjs`（旧 `tools/ci-impact.mjs` は削除済み）、`tools/ci-issue-tracker-reconcile.test.mjs`、mise task（install-ui/setup-ui/check-ui/test-ui/build-ui、test-rt/test-msrv/lint-actions、sync-ui-deps、check-ruviform-rev）、`ruviform-dep-access` composite action が直接の対象である。CI lane は workspace 全 crate をカバーし、個別には mcp-poc の control channel、synvelis-recording の oracle test、synvelis-theme と gpui-test lane、synvelis-project の verified import がこの知識体系の下で検証されている。

開発プロセス側では、evidence ledger・文書チェーン・review-protocol・worktree と coordinator の責務分担という運用規約が同じ知識に属する。将来 CI lane・検証 task・evidence 記録を追加・変更する作業は、この文書の契約（fail-closed・NotRun 正直記録・canonical は mise run）に従う必要がある。

## 出処

CI 基盤構築・regression gate・検証プロセス・flake 対応・文書衛生の作業群からの抽出。出所 Issue は synvelis#83（独立 CI 構築）・#86（verified import 配線）・#115（recording oracle）・#131（shadow CI 観測）・#145（ci.yml manual-only 化）・#147（sccache）・#148/#157/#166（impact 分類と evidence gate・Full CI 化）・#151（ui-verify lane）・#154/#184（CI flake 修正）・#167（tracker reconcile）・#177（design tokens parity）・#190（既実装検証記録）・#230（stale vendor 診断）・#245（macOS alloc flake）・#377（OS-L1 CI）である。Ruviform 側の関連 Issue は ruviform#238（l1_proxy_e2e flake、#241 で解決）・#329（sccache 先行評価、PR #342）・#330（install-ui 先例）・#631（design token provisional 値の review）で、Consumer 側 workflow の共有 CI evaluator pin を `73d51d8…` から Ruviform PR #380 merge 後の main（`eed590e0`）へ揃え、Consumer 版 reconcile workflow を新設する計画が synvelis#167 で策定済みである（観測済みの reconcile run 42183098205 の shared checkout は `eed590e` であった）。
