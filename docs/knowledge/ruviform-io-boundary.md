---
id: "KNOW-THV0stIt0zazZzBLmiGtbd"
type: "knowledge"
status: "draft"
lifecycle: "draft"
knowledge_kind: "architecture"
title: "Ruviform I/O 境界と StagedFile"
created: "2026-11-19"
updated: "2026-11-19"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/81"
    - "https://github.com/scarecrow173/synvelis/issues/82"
    - "https://github.com/scarecrow173/synvelis/issues/84"
    - "https://github.com/scarecrow173/synvelis/issues/184"
    - "https://github.com/scarecrow173/synvelis/issues/330"
    - "https://github.com/scarecrow173/ruviform/issues/288"
    - "https://github.com/scarecrow173/ruviform/issues/296"
    - "https://github.com/scarecrow173/ruviform/issues/305"
    - "https://github.com/scarecrow173/ruviform/issues/306"
  informs: []
  explains: []
  supersedes: []
---

# Ruviform I/O 境界と StagedFile

## 概要

Synvelis の永続化とエクスポートの I/O は、Ruviform の staged publish primitive（`StagedFile`/`SealedFile`/`PublishOptions`/`PublishFailure`/`PublishReceipt`）を境界として使い、プロジェクトの意味論である HEAD・ExpectedRevision・衝突分類・crash 後の再開は Synvelis 側が所有する、という責務分担が SYN-IO-01 で確定した。この境界のもとで `DirectoryBundleStore` は手組みの atomic write を撤去して staged publish へ移行し、HEAD の compare-and-swap は `HEAD.lock` advisory mutex による `save_cas` port として実装された。あわせて、実 2-process 検証・実機 FS kill probe・CI 証拠の照合という検証手法と、observability の Host 所有 logging の配置が確立された。

## 詳細

### I/O 境界の責務分担

ExpectedRevision・HEAD・immutable Snapshot・外部変更検出・Crash 後 Reopen の Owner はすべて Synvelis 側に固定され、Ruviform は staged publish の primitive のみを提供する（synvelis#82、ruviform#288）。HEAD CAS も Synvelis 責務と決定され、Ruviform 側は変更しない。

Backup/Import archive の初期採用範囲は、ZIP コンテナ・Stored/Deflate・limits/path/link/manifest 方針を ADR で決定し、最終 format の凍結は実装 Issue へ委譲する方針が取られた（synvelis#82）。

consumer 固有の意味論は維持し、旧 API 互換層や恒久 SHA 固定には依存しない。Ruviform 依存は既定ブランチ追従の git 依存で、解決 revision は `Cargo.lock` に記録する（synvelis#82）。

OS 固有の Indeterminate/DirSync 失敗注入は ruviform-io-file 側の fault-injection 責務とし、consumer 側は二つの契約を決定的に pin する分担が合意された（synvelis#84、ruviform#296/#306 へ委譲）。一つは「任意の publish 失敗で HEAD を進めない・失敗分類を伝播する」こと、もう一つは「同一 byte 衝突受理は勝者 byte の完全 publish 再実行で耐久性を再確立し、完遂不能なら Err・HEAD 不変」である。

### StagedFile publish と snapshot 衝突分類

`DirectoryBundleStore` の手組み `.tmp`+`sync_all`+`fs::rename`（`write_atomic`）は撤去され、ruviform-io-file の staged publish helper へ移管された。snapshot は `PublishMode::CreateNew`、HEAD は `Replace` で、両者とも `Durability::FileAndDirectorySync` + `RequireAtomic` を要求する（synvelis#84、PR#178）。

`PublishFailure` から `Error::Store` への写像は、effect/phase/durability/operation id/temp path をメッセージへ転写する。`Indeterminate` の残滓 temp は自動削除せず、`NotPublished`/`Published` は best-effort cleanup とする。

同一 revision 衝突は `snapshot_collision` で 3 分類される。byte 完全一致は `Satisfied`（冪等受理）、envelope が `id_watermark` のみ異なる同一論理 snapshot は単調増方向なら `Refresh`（Replace 再公開）、低位方向なら `Satisfied`、それ以外は `Conflict`（既存 byte・HEAD 不変で拒否）である。`InMemoryStore` も同一分類器に整合させ、filesystem 無しで port parity を再現する（synvelis#84）。

ここには設計上の欠陥修正が含まれる。「内容の冪等性」を「耐久性の達成」と同一視して衝突受理をスキップすると、初回 publish が DirSync 失敗で Err を返した後の同一 byte 再送で、snapshot 親 dir の耐久性がどこでも再確立されない。受理経路は「勝者 byte の完全 publish 再実行」に修正された。`PublishReceipt` の Ok が要求した全保証の達成を意味するため、受理が dir sync 証拠を必ず再確立する。公開 API に standalone dirsync が無いため、完遂 publish が唯一の再確立経路である（synvelis#84、merge 前レビュー Judge=BLOCKED 由来の P1 修復）。

非空 bundle への `project.new` 等の silent clobber は、明示拒否へ変わる意図的な強化（behavior change）として記録された（synvelis#84）。

### HEAD CAS（expected-head store port）

`ProjectStore` には additive な `save_cas(project_id, expected_head, revision, bytes)` port と typed `HeadCasError`（`Conflict{expected,actual}`/`Unsupported`/`Store`）が追加された。default 実装は `Unsupported` を返し、unconditional `save` への silent degrade を禁止すると同時に、範囲外の Fake 実装を破壊しない（synvelis#330、PR#342）。

`DirectoryBundleStore` の CAS は `HEAD.lock` advisory mutex で実装される。`StagedFile`+`CreateNew`+`BestEffort` で atomic に取得し、内容は `created_ns`+一意 token とする。stale（20 秒超過・parse/read 不能）は `fs::rename` の一意 aside 名への atomic 退避で break し、fresh は 10ms poll・30 秒で busy timeout とする。critical section 内で HEAD 読取→expected 比較→snapshot publish→pre-publish fencing→HEAD publish→post-publish fencing を行う（synvelis#330）。

post-publish fencing は、pre-check と HEAD rename の間に lock が break される residual window で `Ok` を返さないための仕組みで、indeterminate 相当の `HeadCasError::Store` を返して `head()` 再読込へ委ねる（成功誤報なし）。`HEAD_LOCK_STALE_AFTER`=20s 超過 hold の residual race は物理的余地が残り、kernel file lock 化は ruviform 境界上の別 Issue 事項とされた（synvelis#330）。

advisory lock は非協調 writer（`save` 直接・外部 HEAD 書換え）を排他しない。検出は次回 CAS の conflict で行われる設計限界として明記されている（synvelis#330）。

`ProjectController::save` は `saved.head` を expected に供給し、`Unsupported` のみ legacy `save` へ fallback する。`synvelis-gateway` の journal replay restore は fallback 無しの loud failure とし、load→restore 間の HEAD 移動を silent rewind しない非対称な設計が選択された（synvelis#330）。

`HEAD.lock` は既存 bundle に存在しない新規 file であり、旧 version との混在では `save`（lock 非取得）は影響を受けず、`save_cas` 同士のみが排他される（synvelis#330）。

### 実 2-process / 実機 FS の検証手法

test binary の self re-exec 方式では、env guard（`SYNVELIS_*`）で worker 分岐し、親 process が `current_exe()` + test fn 名 + `--exact --nocapture`（`#[ignore]` test では `--ignored` も必須）で子を spawn する。go-file barrier で同時開始を合わせ、結果は stdout の prefix 行 marker protocol で受け取る。typed outcome 以外（marker 欠落・非 0 終了・timeout）はすべて test failure とする。専用 fixture bin 追加（crate 構成変更）と shell orchestrator（`mise run test` 正規経路から外れる）は棄却された（synvelis#330）。先例は synvelis-library の import テストである。

go-file barrier は best-effort の同時試行だが、`HEAD.lock` 直列化により結果（exactly one winner）は timing 非依存で成立するため十分である。timeout は 60 秒で hang を failure に変換する（synvelis#330）。

実機 kill probe（`SV84-REAL-*`）では、publish 途中の trigger（HEAD.lock 取得・snapshot temp・snapshot rename 完了）を spin-poll で観測して `Child::kill`（TerminateProcess 相当）で殺し、reopen して store 契約の不変条件を assert する。Windows timer granularity（約 15ms）より細かい publish window を sleep で取りこぼさないよう `exists()` spin + 一定回数毎の deadline 判定を使う。kill_round は attempt 毎に fresh seeded bundle を作り、missed window を pass に数えない構造にする（synvelis#84）。

真の power-loss（電源断相当）は process kill では代替不能（dirty page を保持するため）で、非破壊的に自動化できず NotRun のまま残す判断が取られた。`PublishEffect::Indeterminate` の実 FS 誘発も ambiguous OS error が必要で非破壊的に作れず NotRun とされた（synvelis#84）。

opt-in real-FS lane は `#[ignore]` test + 専用 mise タスク（`test-recording-real-fs` と同型 + `--nocapture` で CASE 行を証拠転記可能に）の組合せで、既定 `cargo test`/CI に混入しない。workspace が ReFS/DevDrive の環境では `SYNVELIS_REALFS_ROOT` で bundle を NTFS temp へ固定し FS 種別を証拠に記録する（synvelis#84）。

### 証拠・CI 運用

対象 SHA の CI run が未実施の場合、祖先 SHA の covering run を「nearest-SHA 証拠」として採用してよいが、`git diff` で対象 SHA との差分が関連 code にゼロであることを確認し、run が対象 SHA のものでないことも併記する（synvelis#84）。ローカル/中間証拠を統合受入と混同しない（POLICIES §5）。

CI leg の解釈では、ubuntu/macos leg の通常 FS での test pass のみを OS-L1 Pass の根拠とし、windows-latest leg は「CI 証拠」として区別する。GitHub hosted runner は「まず Windows 実機」条件（実 FS 状態遷移・Crash→Recovery）の代替にならない（synvelis#84）。`rt_` filter で 0 selected の rt-verify leg は RT 証拠に計上しない。

「Orphan の禁止」契約の正確な記述は、conflict は publish 前判定で orphan を生成しないが、crash window の orphan snapshot は recovery データとして保持・Satisfied 再 save で再採用する、というものである。「orphan を残さない」とは記録しない（synvelis#84）。

証拠記録のみの slice では完全な doc chain を重複作成せず、IR + evidence report に集約する（「IR を正とし重複時は IR に集約」の共通契約）（synvelis#84）。

### Observability（SYN-OBS-01）

`ruviform-observability` の `logging` feature を synvelis-app のみが有効化し、`tracing` 直接依存も synvelis-app のみに限定する。他 crate（RT lane）の依存木に tracing が入らないことを `cargo tree` で確認し、`check-headless-deps` lane が GUI/GPU 依存の混入を守る（synvelis#81）。

設定解決は lib 内 pure fn（CLI>env>既定の優先順）と bin 境界の env 読取に分離し、library 側から `std::env` を読まない。`--log`/`--log-file`/`--log-format` と `SYNVELIS_LOG*` env で制御し、既定は OFF。設定不正は exit code 4 で起動失敗とし、静かに無効化しない（synvelis#81）。

global subscriber（`set_global_default`）はプロセスで 1 回しか成功せず解除不能。global install 検証は 1 テストに集約し、他は scoped `with_default` dispatch でカバーする。2 度目の install は `AlreadyInstalled`（非エラー）として自前 sink を畳む（synvelis#81）。

tracing event の機密分類規約として、event field にパス・秘密情報・生データ・op params/body を載せない。RT callback では tracing/alloc/lock 禁止の doc-comment を明記する（synvelis#81）。

`SinkDegraded` が `Clone` でないため `HostLog::take_degraded` で所有権移譲にした。upstream への `Clone` derive 要望は改善候補として記録された（synvelis#81 → ruviform 側 follow-up 候補）。

Collector/Diagnostics/Debug/Host Runner（ruviform#289/#291/#292/#293）は upstream 待ちで本実装に含めず、Source catalog は registrar のないデータとして保持し registrar 所有は upstream に残す（synvelis#81）。

`syn.app.info` の logging セクションは常時出力（未 install 時も `off` を明示）し、GUI が state で分岐できるよう「無いキー」にしない（synvelis#81）。

### Upstream（Ruviform）連携と pin 運用

Ruviform 依存は既定ブランチ追従の git 依存で恒久 SHA pin は禁止。候補 SHA で検証する場合も `Cargo.lock` の全 ruviform crate を同一 SHA に揃え、`check-ruviform-rev` で一致を確認する（synvelis#82）。RF305 では候補 SHA 間の差分が文書 index のみでコード差分ゼロであることを `git diff --stat` で独立確認してから証拠を採用した。

`Cargo.lock` の ruviform 解決 rev を更新した場合は `sync-ruviform-ui` で UI 用 vendor コピーを再同期し、`check-ruviform-rev` で lock rev との REV 一致を確認する運用が RF305 slice で実施された（synvelis#82）。

consumer 先行互換パターンでは、consumer 側の移行を先に完了させて upstream PR の merge 前提とする。RF305 は ruviform PR391 merge 前提の Consumer 互換を先に確立し、RF407（bounded bridge）着地後に SV 側 workaround（`ReadProxy`/scoped-thread/channel proxy）を完全撤去して upstream bridge を消費する形へ置換した。upstream に正本 primitive が後から提供された場合、consumer 側 workaround は残さず撤去する（synvelis#82、ruviform#305/RF407）。

`PublishFailure` は非公開 constructor のため、consumer 側の失敗注入は実 OS adapter 経由（destination directory 等）の genuine `PublishFailure` のみ使用し、`Indeterminate` 注入は RF mock 側の責務とする。fabricated error は使わない（synvelis#82）。

RF407 の `with_borrowed_demuxer` は借用・`!Send` な readback handle を `'static` demuxer へ接続する正本 bounded bridge で、in-flight encoded bytes を方向あたり ≤64KiB に bound する。`MediaSource: 'static` と borrowed `!Send` の境界は RF 側が担い、SV 側で raw handle 漏れ・unsafe lifetime 延長・temp path reopen・独自 parser を行わない（synvelis#82）。

`PublishOptions` 写像の忠実性として、`replace_existing`→`PublishMode::{Replace,CreateNew}`、`sync_before_publish:true`→`Durability::FileSync`（dir-sync 非追加・旧 `File::sync_all`+rename と同順序）、`sync_before_publish:false`→`BestEffort` とする（synvelis#82）。

新旧 `RenderOutput` 契約の非互換時は両立実装（Facade）を作らず、lock を候補 SHA へ揃えて一括更新し、解除条件は upstream merge 後の再解決とする（synvelis#82）。

`cargo-platform@0.3.3` が rustc 1.91 を要求し MSRV 1.89 の `test-msrv` を破壊する main 既存破壊が観測された。混入経路は synvelis-app dev-dep→ruviform-ui-test→cargo_metadata で、SV175 の lockfile 変更由来。本変更非起因として coordinator へ報告し、修正は SV175 所有範囲（依存 pin または代替）と切り分けた（synvelis#82）。

decoded planes の O(output) 保持は移行前からの `expect_pcm` 契約由来で、encoded bytes 全量 RAM 保持の退行のみが修正対象とされた。増分 callback 化は test 契約変更を伴うため別途検討事項である（synvelis#82）。

### テスト fixture / パス綴り

`std::env::temp_dir()` は非 canonical 綴り（macOS `/var`、Windows 8.3 名・junction）を返し得るため、absolute symlink target を作るテストは `create_dir_all` 後に `fs::canonicalize` したパスを保持しないと ruviform-io-file の canonical component 契約と綴りが一致しない。この修正タスクは作成時点で in-progress であり、完了実績ではない（synvelis#184）。

### 運用・プロセスの約束事

canonical 検証は mise task 経由で行う: `fmt-check`/`check`/`lint`/`test`（`--locked`）/`test-rt`/`check-headless-deps`/`check-app-native`/`lint-app-native`/`test-msrv`/`build`/`check-ruviform-rev`。`cargo test` 単発は補助証拠として位置づけ、canonical は mise とする（synvelis#330、synvelis#82 等）。

`Cargo.lock` は手編集せず、`mise run check` や `cargo update -p <dep> --precise <sha>` 等で再生成する。docs の managed index は手編集せず、対応する sync ツールの apply で再生成する。

独立レビューは review-protocol に従い、Reviewer A（協調）/B（敵対）→ Validator → Judge を独立 subagent で実施し、finding を相互共有しない。Judge 判定は ACCEPT / ACCEPT WITH NON-BLOCKING NOTES / BLOCKED。merge 前レビューで BLOCKED が出た場合は修復後に独立 Validator が再検証する（synvelis#84、synvelis#82）。

実装記録（IR）では検証済みと未検証（NotRun/Unsupported/外部 gate）を正直に分離して記録し、推測値を実測として書かない。evidence 転記は実測後に行う（synvelis#84、synvelis#330）。

SYN-IO-01 は docs のみの変更で、実装は synvelis#84/#86 と ruviform#288 へ委譲され、実機 FS crash/durability 実測は NotRun と明示する方針だった（synvelis#82）。検証計画は Golden G-01..07・Negative N-01..10・Crash C-01..07 と gate 証拠 GATE-SV-01..09 を定義したが、計画時点で全 case NotRun である（synvelis#82）。

## 適用対象

Project 永続化（`DirectoryBundleStore`・`ProjectStore`・`ProjectController`・journal replay restore）、render/player の WAV Export（StagedFile 移行済み）、synvelis-app の Host logging、synvelis-library の temp fixture、CI/evidence 運用に影響・説明する知識である。RT lane の依存管理（tracing 非混入）と MSRV 検証、Ruviform 依存 pin と vendor 同期の運用にも関係する。

## 出処

SYN-IO-01 境界設計（synvelis#82、実装委譲先 synvelis#84/#86・ruviform#288）、SYN-IO-02 store StagedFile 移行・HEAD CAS・実機検証（synvelis#84、synvelis#330）、SYN-RF305 render/player WAV Export 移行（synvelis#82、ruviform#305）、SYN-OBS-01 Host logging・Source catalog（synvelis#81）、synvelis-library canonical temp fixture 修正（synvelis#184、in-progress）からの抽出。
