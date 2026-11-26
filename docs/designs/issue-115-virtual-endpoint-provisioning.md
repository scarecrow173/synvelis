---
id: "DESIGN-sv115VirtualEndpointProvisioning"
type: "design"
status: "approved"
title: "Issue #115 RCD-01 Virtual Endpoint Provisioning Consumer Orchestration 設計"
created: "2026-11-25"
updated: "2026-11-26"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  changes:
    added:
      - type: "file"
        path: "docs/designs/issue-115-virtual-endpoint-provisioning.md"
      - type: "file"
        path: ".github/workflows/virtual-endpoint-consumer-acceptance.yml"
      - type: "file"
        path: "evidence/sv115/issue-115-virtual-endpoint-consumer-workflow-implementation.md"
    modified:
      - type: "file"
        path: "crates/synvelis-recording/tests/virtual_endpoint.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-115-virtual-endpoint-provisioning.md"
  implemented-by: []
  depends-on:
    - "docs/specs/issue-115-virtual-endpoint.md"
    - "docs/designs/issue-115-virtual-endpoint.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/issue-115-virtual-endpoint-provisioning-ownership.md"
    - "docs/test-specs/issue-115-virtual-endpoint-provisioning.md"
    - "https://github.com/scarecrow173/ruviform/blob/main/docs/designs/virtual-audio-endpoint-provisioning-interface.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-115-virtual-endpoint-provisioning.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/issue-115-virtual-endpoint-provisioning.md"
  references:
    - ".github/actions/ruviform-dep-access/action.yml"
    - ".github/workflows/ci.yml"
    - "evidence/sv115/issue-115-virtual-endpoint-verification.md"
    - "https://github.com/scarecrow173/ruviform/blob/main/.github/workflows/virtual-audio-driver-probe.yml"
    - "https://github.com/scarecrow173/ruviform/issues/762"
  defers: []
  deferred-by: []
---

# Issue #115 RCD-01 Virtual Endpoint Provisioning Consumer Orchestration 設計

> 2026-11-26 follow-up gate note: 初回 provisioned manual run `45797309944`
> attempt `1` は consumer Fail だった。本文中の「merge 後 follow-up gate」という
> 記述は implementation 時点の計画として読み、run 観測と acceptance 転記は
> `evidence/sv115/issue-115-virtual-endpoint-provisioned-consumer-run.md` を参照する。
> 設計上の ownership、oracle、provisioning 責務分界は run 結果で変更しない。

> Cross-repo link note: Ruviform 側文書への `blob/main` URL は、対応 PR merge 後の
> canonical location を示す。proposal 時点の main にはまだ存在しておらず、両 PR の統合後に
> 実在させる意図のリンクである。

## 1. 設計入力の再確認

本設計は 2026-11-25 に作成され、2026-11-26 の implementation 時点で次の現状態を再確認した。

- Synvelis `origin/main` は `a24ff229d5084a6a44890ff3a2a06dc940cab0c9`。
  PR #654 の consumer virtual endpoint harness は merged 済みで、lane は
  `mise run test-recording-virtual-endpoint`。
- 現行開発 host の `evidence/sv115/issue-115-virtual-endpoint-verification.md`
  は endpoint pair 不在のため `NotRun — NOT_PROVISIONED`。lane exit 0 は
  Pass ではない。
- Synvelis `Cargo.lock` の Ruviform resolved revision は
  `87a7ba8ff3fbe6d4502bea27d152d7f74c105899`。manifest は default branch
  追従で、CI が single resolved SHA を guard する。この revision は
  provision/unprovision script pair と shared module を含む。
- Ruviform `origin/main` は `71f8bd4da047699eeb3ead781ff19e8a21320816`。
  `virtual-audio-driver-probe.yml` は `workflow_dispatch` 専用・
  `windows-2025` x64 単一 leg・timeout 90 分で、build → catalog → 2 日
  失効の self-signed cert → Root/TrustedPublisher 登録 → sign verify →
  `pnputil` staging → root-enumerated MEDIA devnode → endpoint/MIDI probe →
  stream roundtrip → wiring/e2e/fault cases を同一 job に内包する。現行 manual
  probe では cleanup 後に artifact upload が行われるが、この順序は本設計の
  consumer workflow には持ち込まず、後述する通り consumer evidence upload は
  cleanup より前に置く。
- 同 workflow は `bcdedit` も reboot も行わない。`testsigning=Yes` と
  SecureBoot OFF は GitHub-hosted runner の出荷状態として扱う。artifact
  retention は 30 日、権限は `contents: read`。
- Ruviform #762 は open のまま。Ruviform #977/#978/#979/#1001 は closed。
- Synvelis/Ruviform はいずれも public。`.github/actions/ruviform-dep-access`
  は anonymous HTTPS と `CROSS_REPO_DEPLOY_KEY` fallback を既に持つ。

これらの事実は設計根拠である。転記値と最新 main が異なる場合は最新 main を
優先し、implementation slice 着手時にもう一度確認する。

## 2. 所有権境界

所有権は次の通り分離する。

| owner | 提供する責務 | 提供しない責務 |
| --- | --- | --- |
| Ruviform | driver source、driver build/catalog/test-sign、driver store staging、root-enumerated MEDIA devnode、microphone consent provisioning、endpoint/MIDI probe、provision state、idempotent cleanup、primitive の失敗 vocabulary | Synvelis checkout、consumer target SHA、consumer test lane、Recorder oracle、consumer acceptance verdict、`evidence/sv115/` の解釈 |
| Synvelis | target SHA の選択と検証、Ruviform lock revision の解決、同一 job orchestration、`mise run test-recording-virtual-endpoint`、independent sample/byte/clock/latency/recovery/journal oracle、RunMetadata 解釈、evidence 転記 | driver build/sign/install ロジックの複写、devnode/consent/cleanup の恒久実装、Ruviform driver の内部仕様変更 |

この境界では、Ruviform は「OS 上に再現可能な endpoint 環境を作り、確実に
畳む」ことを所有し、Synvelis は「その環境で製品経路が受入条件を満たすか」を
所有する。判定の権威は consumer harness の terminal RunMetadata であり、
provisioning state はその実行条件を証明する side evidence である。

### 不適案

install ロジックを Synvelis workflow へ複写すると、driver store、cert、devnode、
consent、cleanup の知識が Ruviform の実装変更から乖離する。Ruviform workflow
へ Synvelis checkout と test lane を持ち込むと、共通基盤が特定 consumer の
受入方法へ依存し、依存方向が逆転する。両方とも複製または逆依存によって
同一の不変条件を二箇所で保守することになるため採用しない。

reusable workflow も採用しない。`workflow_call` の callee は別 job として
実行され、job ごとに runner instance が分かれる。devnode と endpoint は
provision した ephemeral runner の OS 状態なので、別 job から参照できない。
callee 内で consumer test まで実行する設計は、今度は Ruviform が Synvelis
checkout を知る逆依存になる。

signed package artifact も採用しない。現行 probe workflow の self-signed cert
は作成から 2 日で失効し、artifact retention は 30 日である。cross-repo
artifact download には token/権限と trust boundary が増え、package に使われた
cert・source revision・signature の再検証を consumer 側で恒久運用しなければ
ならない。短命 cert と期限つき artifact は、再現可能な acceptance interface
ではなく debug 用の一次成果物に適する。

## 3. Ruviform primitive interface の採用形状

初期 interface は PowerShell script pair とする。

- `tools/virtual-audio-driver/Provision-RvfVirtualAudioEndpoint.ps1`
- `tools/virtual-audio-driver/Unprovision-RvfVirtualAudioEndpoint.ps1`

script pair は GitHub Actions 外からも直接実行でき、同一 job の任意の step
から呼べる。composite action は GitHub Actions 内で step 群を隠蔽できるが、
初期契約としては script pair の方が小さく、runner 外での再現確認と manual
probe からの段階的移行に適する。composite action は後で薄い wrapper として
追加してよいが、interface の正本は state JSON と script pair に置く。

詳細な入力・出力・失敗 vocabulary・cleanup・必要権限は Ruviform 側
`docs/designs/virtual-audio-endpoint-provisioning-interface.md` を正本とする。
Synvelis はその state JSON を parse するだけで、provisioning stage を再実装
しない。

## 4. Synvelis manual workflow の conceptual design

implementation slice が追加した workflow は `workflow_dispatch` 専用の
`.github/workflows/virtual-endpoint-consumer-acceptance.yml` である。
required gate・ci-summary・ruleset には接続しない。runner は `windows-2025`
単一 leg、job timeout は初期値 120 分を上限とし、実測後に短縮を検討する。

### 4.1 step 順序

1. Synvelis を checkout し、`git rev-parse HEAD` を `target_sha` として記録
   する。manual input `expected_sha` がある場合は一致を要求し、不一致なら
   harness 実行前に失敗させる。
2. `permissions` は workflow 直下で空または `contents: read` に限定し、
   repository write token を渡さない。
3. mise/Rust toolchain を復元し、依存アクセス用に既存
   `.github/actions/ruviform-dep-access` を実行する。
4. `Cargo.lock` から single resolved Ruviform revision を抽出し、
   `ruviform_lock_rev` とする。複数 hit・零 hit・ malformed source は失敗。
5. Ruviform source を `ruviform_lock_rev` で `$RUNNER_TEMP` 配下へ checkout
   する。public 期は anonymous HTTPS、private 復帰時は既存 action が設定する
   read-only deploy key fallback を使う。checkout は Synvelis worktree を汚さない。
6. provision script を同一 job 内で実行する。入力には source root、expected
   source revision、workspace/evidence/state path、arch=x64、configuration、
   WDK install 可否、mic consent 可否、timeout、ephemeral cleanup mode を
   明示する。
7. provision state JSON を parse し、terminal status が `ok` で endpoint pair
   候補が記録されている場合だけ consumer lane へ進む。`ok` 以外は test を
   実行せず、provisioning evidence を upload して job を失敗させる。
8. `SYNVELIS_RUN_REPO=scarecrow173/synvelis` と
   `SYNVELIS_RUN_SHA=<checkout HEAD>` を env 注入して
   `mise run test-recording-virtual-endpoint` を実行する。Ruviform lock rev
   と actual provisioning source rev は harness の evidence 記録へ入れるか、
   少なくとも artifact manifest と evidence 転記で terminal RunMetadata と
   同一 run に紐づける。harness 側に env 記録の小さな拡張が必要なら、
   implementation slice で oracle を変えずに provenance だけを追加する。
9. stdout/stderr、terminal RunMetadata JSON、provision state、driver package
   manifest、runner/tool versions を consumer evidence artifact として upload
   する。artifact 名は `sv115-virtual-endpoint-consumer-<run_id>-<attempt>`、
   retention は初期 90 日とする。この upload は cleanup より前に置き、
   `if: always()` で実行する。cleanup state はまだ存在しないため、この
   artifact の内容に含めない。
10. cleanup script を `if: always()` で実行し、partial state を含めて
    devnode/driver/cert/consent/service を cleanup contract に従って畳む。
11. cleanup state、cleanup log、residue/leftover record を別 artifact
    `sv115-virtual-endpoint-cleanup-<run_id>-<attempt>` として
    `if: always()` で upload する。cleanup が失敗または hang しても consumer
    evidence artifact は step 9 で確定済みであり、test evidence の有無は
    cleanup 成否に依存しない。cleanup 失敗は job の運用失敗として扱うが、
    consumer harness の Pass/Fail/NotRun と混同しない。

test step が非零終了しても後続 step が走る必要があるため、metadata 収集、
consumer evidence upload、cleanup、cleanup artifact upload はすべて
`if: always()` にする。consumer evidence upload を cleanup より後ろへ移しては
ならない。workflow は terminal metadata を parse し、`pass` だけを success、
`fail` は failure、`not-run` は acceptance 未達として failure または explicit
neutral 相当の運用失敗に分類する。GitHub には neutral conclusion がないため、
job conclusion は運用上の実行可否を表すだけであり、acceptance verdict の権威
にはしない。

### 4.2 SHA の対応関係

証拠では少なくとも 4 種の SHA を区別する。

| 名前 | 意味 | 記録先 |
| --- | --- | --- |
| `github_sha` | workflow dispatch ref の SHA。workflow 定義の出所 | artifact manifest |
| `target_sha` / `SYNVELIS_RUN_SHA` | checkout 後検証済みの Synvelis 対象 SHA | RunMetadata `sha` |
| `ruviform_lock_rev` | Synvelis `Cargo.lock` の resolved Ruviform revision | provisioning record / artifact manifest / evidence |
| `ruviform_checkout_rev` | provisioning script 実行に使った actual source revision | provision state JSON / artifact manifest |

初期運用では `github_sha` と `target_sha` の一致を要求する。`github_sha` は
workflow 定義の出所であり、任意 SHA の code を別 workflow 定義で実行すると
証拠の対象が曖昧になるためである。`ruviform_lock_rev` と
`ruviform_checkout_rev` が一致しない run は acceptance evidence に使えない。

## 5. source pin 方針

採用案は Synvelis `Cargo.lock` resolved revision への pin である。

理由は、consumer harness が link する Ruviform crate と provisioning に使う
driver source を同じ revision に揃えられるからである。manifest が default
branch 追従でも、Synvelis の再現性契約は lockfile の single resolved SHA が
guard している。provisioning source だけ main 追従にすると、driver/endpoint
挙動と consumer 依存 revision がずれ、Pass/Fail の原因切り分けができない。

この決定には制約がある。primitive extraction が Ruviform main に入っても、
Synvelis `Cargo.lock` が古い revision を指す間は consumer workflow から新
primitive を使えない。初期導入では、primitive merge 後に Synvelis lockfile を
通常の依存更新として前進させる必要がある。この更新は workflow 実装へ混ぜず、
差分と検証が追える独立した依存更新として扱う。

rollback は、 Synvelis `Cargo.lock` を既知の revision へ戻し、その revision
で consumer workflow を再実行することで行う。primitive の bugfix が緊急に
必要な場合だけ manual input による override を検討できるが、override 版で
得た結果は acceptance evidence に使わない。例外運用を常設しない。

## 6. RunMetadata と evidence 運用

判定責任は次の通り分ける。

- harness: terminal RunMetadata を生成し、`pass`/`fail`/`not-run`、oracle id、
  repo/SHA、environment/level、evidence reference を記録する。
- workflow: harness を実行条件どおり起動し、metadata を収集し、provisioning
  provenance を consumer evidence artifact に、cleanup record を cleanup
  artifact に紐づける。harness 出力を書き換えない。
- evidence 解釈者: artifact の raw metadata を読み、acceptance verdict を
  `evidence/sv115/` へ転記する。job green を Pass に転用しない。

`evidence/sv115/` への転記は CI からの direct commit ではなく、通常の PR で
行う。転記責任は manual run を承認した Coordinator またはその結果を扱う
後続 Worker とし、run 完了後 artifact retention 内で、acceptance 判定を更新
する最初の evidence PR に含める。CI artifact が raw evidence の一次保存先、
repository evidence が人間がレビューできる canonical summary とする。
artifact retention は有限なので、Pass/Fail の判定に使った terminal
RunMetadata JSON 本体、CI run URL、OS/driver/cert/provisioning record、
Ruviform lock/checkout revision、artifact 名、artifact retention を転記時に
残す。cleanup 結果は取得できた場合に併記するが、欠損しても consumer verdict
を変えない。RunMetadata が小さい場合は JSON を evidence 配下へそのまま配置し、
summary Markdown から参照する。

初回 provisioned run は Fail になる可能性がある。Fail を隠して再実行だけを
記録せず、失敗 run の artifact と metadata も残す。NotRun は endpoint 未
provisioning の記録であり、provision state が `ok` の run で `NOT_PROVISIONED`
が出た場合は provisioning readiness と consumer 判定の矛盾として acceptance
不可にする。

## 7. open question の決定

| 論点 | 決定 | 根拠と残リスク |
| --- | --- | --- |
| script pair か composite action か | 初期は script pair | direct execution、同一 job step、manual probe からの段階移行に適する。composite action は後で wrapper 追加可。残リスクは呼び出し側が state JSON parse を持つこと。 |
| manual-only か定期 opt-in か | 初回は manual-only | WDK install/build を含み長時間で、初回 provisioned run の flake 切り分けが済んでいない。required gate 非接続を維持する。定期化は複数回の安定 run、実行時間、retry 方針、rf#762 の整理後に再検討する。 |
| source pin は lock rev か main 追従か | `Cargo.lock` resolved rev | consumer crate と driver source の revision を一致させ、再現性を保つ。残リスクは lock 更新まで新 primitive を使えないこと。rollback は lock 差し戻し。 |
| evidence を repo commit するか artifact link のみか | CI artifact + 通常 PR で repo 転記 | CI write token を増やさず、artifact 期限切れ後も canonical metadata を残せる。残リスクは転記忘れ。implementation slice の受入手順に転記を含める。 |
| rf#762 を supersede するか | narrow/partial supersede。close はしない | Ruviform 独自 driver を GitHub-hosted runner で使う経路は same-job ephemeral provisioning で代替できる。external driver、license、self-hosted/persistent runner、external hardware の検討は残る。本 slice では Issue を更新しない。 |
| `CROSS_REPO_DEPLOY_KEY` か anonymous か | public 期は anonymous HTTPS を第一経路、既存 action の fallback を使う | 両 repo は public。最小権限では secret を使わない。private 復帰時は既存 `ruviform-dep-access` の read-only deploy key fallback を使う。checkout と cargo dep access の両方で同じ rewrite を利用できる。 |
| sv#115 close 条件の整理 | 本 slice は provisioning 後の virtual EXT-L3 実走のみ。残件は後続 Issue 分割で扱う | GUI Option B、fault injection、Punch/Monitor/Arm 全 case、macOS/Linux/physical の扱いを同じ acceptance vocabulary で再整理する必要がある。本設計はそれらを Required に格上げしない。 |

## 8. 後続 implementation slice 分割

### Slice A: Ruviform primitive extraction

目的は、manual probe workflow に埋め込まれた provisioning 手順を reusable
script pair へ抽出し、state JSON contract を実装することである。

予定 file list:

- 追加 `tools/virtual-audio-driver/Provision-RvfVirtualAudioEndpoint.ps1`
- 追加 `tools/virtual-audio-driver/Unprovision-RvfVirtualAudioEndpoint.ps1`
- 必要なら追加 `tools/virtual-audio-driver/RvfVirtualAudioProvisioning.psm1`
- 必要なら変更 `tools/testsigned-driver/probe-stage.ps1`（既存 helper の再利用で足りるなら変更しない）
- 変更 `tools/virtual-audio-driver/README.md`
- 段階移行で変更 `.github/workflows/virtual-audio-driver-probe.yml`
- 関連 docs/test/evidence（implementation slice で決定）

検証は prek/actionlint、script の静的確認、manual probe workflow の parity run
で行う。移行は 2 段階とし、まず inline workflow を残したまま script を追加
して state JSON を確認し、次に manual probe の該当 stage を script 呼び出しへ
置き換える。artifact 名・stage vocabulary・fault cases の実行可能性を落とした
場合は rollback する。

### Slice B: Synvelis consumer workflow

目的は、同一 `windows-2025` job 内で provisioned endpoint を使って consumer
EXT-L3 harness を実行し、artifact/evidence 運用を確立することである。

予定 file list:

- 追加 `.github/workflows/virtual-endpoint-consumer-acceptance.yml`
- 必要なら変更 `crates/synvelis-recording/tests/virtual_endpoint.rs`（Ruviform
  revision provenance を RunMetadata evidence に追加する場合のみ。oracle は変更しない）
- 追加 `evidence/sv115/issue-115-virtual-endpoint-consumer-workflow-implementation.md`（implementation validation。provisioned run 転記は実走後の通常 PR）
- 更新 `docs/specs/issue-115-virtual-endpoint-provisioning.md`
- 更新 `docs/designs/issue-115-virtual-endpoint-provisioning.md`
- 更新 `docs/test-specs/issue-115-virtual-endpoint-provisioning.md`
- 更新 `docs/adr/issue-115-virtual-endpoint-provisioning-ownership.md`
- primitive が lock revision に無い場合は、独立した `Cargo.lock` 依存更新 PR

検証は actionlint、prek、workflow syntax/permissions の静的確認、canonical
local harness lane で行う。新規 workflow file は default branch に入るまで
manual dispatch できないため、provisioned manual run と artifact 確認は merge 後
follow-up gate とする。初期 manual run では Pass を前提にしない。Fail または
infrastructure error が出た場合は evidence に残し、consumer acceptance の
達成とは別に切り分ける。

rollback は manual-only であることを利用する。不具合が見つかった場合は、まず
GitHub Actions の workflow disable または手動 dispatch 停止で新規実行を止め、
修正または revert PR で workflow ファイルを main から取り除く。manual trigger
以外を持たないため、disable/revert 以外の緊急遮断は不要である。harness
provenance 変更だけが問題なら workflow を残して test file の変更だけを revert
し、lockfile 更新が問題なら独立した dependency rollback PR で戻す。consumer
workflow rollback は既存 harness lane `mise run test-recording-virtual-endpoint`
や merged 済み harness oracle を削除する意味ではない。

依存順は Slice A → lockfile 更新（必要な場合）→ Slice B。Synvelis workflow
を先に作っても primitive が lock revision に存在しなければ実行できないため、
この順序を逆にしない。

## 9. sv#115 close へ向けた残件マップ

| 領域 | 現行扱い | 後続候補 |
| --- | --- | --- |
| provisioned virtual WASAPI EXT-L3 | 本設計の対象。Required のまま | Slice A/B implementation と provisioned run evidence |
| Mock/L1 byte/sample/journal oracle | 既存 harness と並行する独立 Required 領域 | 現状の検証継続。本設計では変更しない |
| Punch/Loop/Arm/Monitor・frame boundary | broad Issue の残件 | Synvelis 側後続 Issue で case matrix を分割 |
| fault injection/device loss/queue full | fake/injected fault と driver-level fault を混同しない | Ruviform fault-case 成果物と consumer 側 fault oracle の対応付け |
| GUI Option B | 本設計の対象外 | GUI 受入 Issue で Required/Deferred を決定 |
| macOS/Linux L1 | 既存 CI native/build/headless の範囲 | 既存 CI 証拠の同 SHA 確認。virtual endpoint 化は本設計の対象外 |
| physical Audient/USB 特性 | 通常必須から外す方針を維持 | 未保証として記録。必要時だけ別 Issue |

この表は close 判定ではない。sv#115 を close する前に、broad Issue の
acceptance vocabulary を Required/Deferred/NotApplicable/NotRun/Pass/Fail で
再整理する必要がある。

## 10. 主要リスクと対応

- WDK install/build が長時間になる。初期 manual-only・timeout 120 分で観測し、
  cache や tool preinstall の最適化は実測後に検討する。
- mic consent provisioning は registry 変更と `AudioEndpointBuilder`/`Audiosrv`
  restart を伴う。ephemeral GitHub-hosted runner 限定とし、before 値を state に
  記録して cleanup で復元する。
- cleanup が失敗すると runner へ residue が残る。runner は ephemeral だが、
  cleanup state を artifact に残し、consumer verdict と分離して運用失敗として
  扱う。
- harness crash で terminal metadata が欠けることがある。その run は Pass/Fail
  evidence にせず、acceptance 上は未達として扱う。crash log は artifact に残す。
- source checkout・lock revision・RunMetadata SHA の不一致は証拠を無効にする。
  workflow は mismatch を fail-closed で検出する。
