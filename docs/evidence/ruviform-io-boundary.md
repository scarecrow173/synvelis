# Ruviform I/O 境界と StagedFile — 実行証拠

この文書は、Ruviform I/O 境界の確定と StagedFile 移行・HEAD CAS・実機検証・observability 導入に関して実施された検証と、その結果・判断を記録する。計画段階で未実施の検証は実績と混同しないよう明示する。

## SYN-IO-02 store StagedFile 移行（synvelis#84、PR#178）

`DirectoryBundleStore` の独自 atomic write を撤去し ruviform-io-file staged publish へ移管した。検証構造は契約テストで、snapshot=CreateNew・HEAD=Replace・FileAndDirectorySync・RequireAtomic で publish されることを pin した。結果は store publish 契約テスト 16 件を含む workspace 全 83 test binary・0 fail（Windows 実測、`mise run test --locked`）である。

merge 前レビューで Judge=BLOCKED の P1（Satisfied 衝突受理が snapshot 親 dir 耐久性を再確立しない）を検出・修復した。修復後の独立 Validator 再検証は CONFIRMED-FIXED、Judge は ACCEPT WITH NON-BLOCKING NOTES。修復前は新規 2 テストが RED（false success `Ok("r1.json")` を観測）、修復後 GREEN である。

決定的 fault 試験として、過去 mtime sentinel で「同一 byte 再 save が destination を物理更新する（publish pipeline 再実行の証拠）」こと、および Windows `FILE_SHARE_READ` 保持/unix read-only で再公開完遂不能時に Err・HEAD 非 repoint となることを pin した。実電源断は未実施（mechanism pin のみ）である。

残存 non-blocking として、≥3 同時 writer での read→Replace 間 TOCTOU（単一 writer モデル非目標）、unix 側 fault leg は修復前判別しない（RED 判別は Windows leg）が記録された。macOS/Linux 実測・実電源断・複数 process の真 HEAD CAS は NotRun/設計 deferred として記録されている。

## SYN-IO-02 HEAD CAS 実装（synvelis#330、PR#342 merge `d653062`）

`save_cas`/`HeadCasError`/`HEAD.lock` advisory mutex CAS を実装し、`head_cas` 契約テスト 8 件を追加した。pin した内容は、expected 一致 commit、stale/absent-assert conflict（publish 前判定で snapshot/HEAD/orphan 不変）、同一 root 2 store 同時 save_cas の single-winner+typed Conflict、InMemoryStore parity、Unsupported fallback、stale lock の rename-aside break、fresh lock 待機→解放後 commit、snapshot publish 失敗時の lock 解放と retry 回復、controller moved-HEAD conflict（saved 非前進）である。

canonical 検証は全 pass: fmt-check/check/lint/test（206 test binary・0 fail）/test-rt/check-app-native/lint-app-native/test-msrv/build。`Cargo.lock` 不変（ruviform 解決 SHA `fd1d4d5f…`、ruviform 側変更なし）である。

NotRun 記録は、実 2-process（当時は後続 slice へ委譲、後に実測済み）、macOS/Linux、電源断、非協調 writer 排他、stale 閾値超過 hold の residual race。journal restore の `save_cas` が Unsupported を返す場合は loud failure を選択した（実経路では発生しない）。

## SYN-IO-02 HEAD CAS 実 2-process E2E（synvelis#330）

self re-exec 方式の 2-process テスト `two_process_save_cas_single_winner_typed_conflict` を追加し、3 round・各 round で現在 HEAD を expected として 2 子 process を spawn した。Windows 実機（Windows 11 MINGW64、rustc/cargo 1.99.1、base `82d27ac`）で exactly one winner + loser typed `Conflict` + 最終 HEAD=勝者 + 敗者 snapshot 非 publish + `HEAD.lock`/stale 残滓なしを pin した。`head_cas` suite は 9 tests 全 pass、同 test を 5 回連続実行して flake なしを確認した。

canonical 検証（fmt-check/check synvelis-project/lint/test synvelis-project）は全 exit 0。macOS/Linux は portable code のため CI 実走見込みとして NotRun 記録（merge 後対象 SHA の CI 証拠で確認）である。

round 1 は expected=None（absent assert）、以降は前 round 勝者 HEAD を expected とする moved-head assert で非空 store 上の CAS も exercise した。revision は round・子ごとに一意採番して snapshot 名衝突を回避した。

## SYN-IO-02 Windows NTFS 実機 Case lane（synvelis#84）

`real_fs_probe`（約 800 行・全 `#[ignore]`）を新設し、`SV84-REAL-01`（kill-mid-publish→reopen 不変条件・5 trigger window）、`SV84-REAL-02`（crash 後 `save_cas` retry 回復・stale `HEAD.lock` 実 break）、`SV84-REAL-03`（Failed publish・CAS `Conflict` で HEAD 非進行+retry 回復）を実装した。

`SYNVELIS_REALFS_ROOT` を NTFS temp（`C:`）に固定し（workspace `G:` は ReFS/DevDrive のため）、`mise run test-project-real-fs` が exit 0・3 passed（20.93s）であった。対象 SHA `dbf0820`、Windows 11 Pro 10.0.26300・NTFS・rustc 1.99.1・ruviform rev `fd1d4d5f`。`CASE=` 観測行を evidence report に転記し、OS-L1 残ゲート表の Windows NTFS 実機 Case 行を更新した。power-loss・Indeterminate は NotRun 維持。ソースコード差分ゼロ・`Cargo.lock` 変更なしである。

実装中にレビューで検出・修正した構造的欠陥が二つある。`#[ignore]` test の子 spawn に `--ignored` flag が無いと libtest が 0 tests run で clean exit し kill window を永久 miss する問題と、kill_round を同一 bundle で retry すると missed window で commit 済み HEAD が expected をずらし Conflict→clean exit 連鎖で panic する問題である。後者は attempt 毎に fresh bundle に変更して解消した。

`Satisfied` collision を retry 回復の観測点に利用した。payload を seed/len で deterministic にし、crash writer と byte 同一の retry が orphan `r2.json` を `SnapshotCollision::Satisfied`→Replace 再 publish で採用する実経路を踏ませた（byte が違えば Conflict で回復しない）。

## SYN-IO-02 OS-L1 evidence 照合（synvelis#84、コード変更なし）

対象 SHA `c0b000b3`（main HEAD・PR#380 merge）の ci.yml run が収集時点で未実施のため、祖先 `7c206289` の covering run 43428177865（全 14 job success、workflow_dispatch 手動 run）を採用した。`git diff --stat` で差分が PR#379（pre-commit 設定）と PR#380（VCS-06 test/docs/evidence）のみ、synvelis-project のソースと syn-io-02 fixture 3 file にゼロであることを確認してから適用した。

leg 別結果は、ubuntu/windows/macos/msrv で head_cas 8・store_publish 16・controller_store 11 全て `ok`。rt-verify は `rt_` filter で 0 selected のため RT 証拠に計上しない。

Deferred/NotRun の正直な分類は、対象 SHA 自体の run、Windows NTFS 実機 Case（後の slice で実測済み）、物理電断、Network/Cloud FS、実 GUI/Device/Plugin、AudioLens consumer。upstream 委譲先は ruviform#305/#294/#306、synvelis#28/#294、audio-lens#143 である。

リスク明記として、nearest-SHA 証拠は「関連 code が byte 同一」の照合に基づくため、`c0b000b3` 自体の後続 run が fail すれば遡って無効である。windows-latest leg の Pass は Issue #84「まず Windows 実機」条件を満たさない。

## SYN-RF305 render/player WAV Export StagedFile 移行（synvelis#82、ruviform#305/PR391+RF407）

旧 `ruviform_media_io::{StagedOutput,PublishOptions}` を `StagedFile`/`SealedFile`/`PublishReceipt`/`PublishFailure` 契約へ移行した。render/player の export 周辺モジュール（spec/export/verify/l2_e2e/pc_b）を更新し、`Cargo.lock` 全 ruviform crate を候補 SHA `27698cb1`（RF main `5d48298b` + staged-file cutover + RF407 bounded bridge `4c675193` + docs）へ統一した。`3473a964→27698cb1` の差分は文書 index のみ 7 ファイルでコード差分ゼロを独立確認した。

初期移行で混入した encoded bytes 全量 RAM 保持の退行（root Validator が P1 CONFIRMED。例: 48k stereo f32・2h で旧 ~2.575GiB →新 ~5.15GiB、コード評価）は、SV 独自 proxy を完全撤去して RF407 `with_borrowed_demuxer` bridge 消費で構造的に解消した。in-flight encoded byte は方向あたり ≤64KiB に bounded。decoded planes の O(output) 保持は移行前契約由来で対象外である。

最終 head `27698cb1` の実測は、fmt-check/check/lint pass、`mise run test` 89 test binary・0 fail、test-rt 0 fail、check-headless-deps pass（9 crates）、check-ruviform-rev pass（26 sources・REV match）、check/lint-app-native pass、l2_e2e 17/17・x_e2e 5/5・pc_b 7/7 全 pass。旧 API 名（`StagedOutput`・旧 `PublishOptions`・`open_boxed_muxer`）の workspace 検索ゼロである。

`test-msrv` は `cargo-platform@0.3.3`（rustc 1.91 要求）が SV175 `2c0711e` 以降の lockfile に存在するため fail — main 既存破壊で本変更非起因として coordinator へ報告済みである。

独立レビュー 2 ラウンド完走。第1ラウンド（移行 head）は Reviewer A/B とも ACCEPT（non-blocking notes）、Validator 12/12 CONFIRMED・0 REJECTED、Judge ACCEPT WITH NON-BLOCKING NOTES・P0/P1 ゼロ。第2ラウンド（bridge 消費後）は Reviewer A/B とも P0/P1 ゼロ、Validator 6/6 adjudicated（B の F2 指摘は誤同定とし却下）、Judge ACCEPT・merge-eligible・条件なし。follow-up 候補は custom verify hook 未カバー経路、synvelis-recording の手組 staged publish（ともに P2・範囲外）である。

publish 失敗注入は destination directory で実 OS adapter の genuine `PublishFailure`（`NotPublished`）を発生させ、`ExportStage::Publish`+effect 保全+temp 残滓なしを pin した（fabricated error なし）。`PublishEffect::Indeterminate` の Consumer 側決定的注入は `PublishFailure` 非公開 constructor と RF mock の `#[cfg(test)]` internal により生成不能で、typed 分類経路は `NotPublished` leg で pin した。

## SYN-OBS-01 Host logging + Source catalog（synvelis#81）

`synvelis-app` に `ruviform-observability` `logging` feature と `tracing` 直接依存を追加し、`logging` モジュール（`LogSettings`/`CliOverrides`/`EnvOverrides`/`parse_args`/`resolve`/`to_logging_config`/`InstallState`/`HostLog`/`install`）を新設した。`HostLog` が global subscriber・guards・health・degraded を process 単位で一元所有し、bin で install→`attach_logging`→main 末尾まで保持して順序 shutdown する構造である。

実測（Windows・rust 1.99.0 pinned）は fmt/check/lint/test/test-rt/check-headless-deps 全 pass。`cargo test -p synvelis-app` で logging 11 件+service_ops 13 件+asset_check 10 件 pass。`cargo tree` で tracing stack が logging feature 経由のみ、player 等 RT 系 crate は tracing 非依存を確認した。

logging テスト新設 11 件は、parse/resolve 優先順位・OFF 既定・degraded best-effort sink・scoped `with_default` 経由の file sink 実書き込み・global install once-only 契約・source catalog 整合。`syn.app.info` の logging セクション形状（未 install 時 `off`/空配列）を service_ops に追加検証した。

NotRun は、実機 Windows WebView2 ウィンドウでの起動・描画（headless 検証のみ）、worker による `test-msrv` 未実行。upstream#289 collector・#291/#292/#293 は open のまま deferred である。

判断として、`set_global_default` の 1 回制約から global install を叩くテストを 1 件に集約し、`AlreadyInstalled` は非エラー化、app_info logging セクションは常時出力、env 読取は bin 境界に限定（library は `std::env` を読まない）、`SinkDegraded` 非 Clone のため `take_degraded` で所有権移譲（Clone derive を upstream 改善候補として記録）とした。

## canonical temp fixture（synvelis#184、in-progress）

`synvelis-library` の symlink テストで `std::env::temp_dir()` の非 canonical 綴りが ruviform-io-file の canonical component 契約と不一致となり失敗する問題に対し、`TempTree::new` で `create_dir_all` 後に `fs::canonicalize` したパスを保持する修正が計画された。junction 経由の非 canonical TEMP/TMP での `--test local` 全件 pass を検証条件とし、修正前の同一 4 件 FAILED は対照実験として記録済み。タスク status は in-progress であり完了実績ではない。

## 文書チェーン運用の実績

SYN-IO-01（synvelis#82）は docs のみの変更として discovery→spec→adr→design→test-spec→plan→task の文書チェーンを整備し、task1（境界 Owner・失敗写像・Backup/Import 設計）と task2（検証計画・gate 証拠・文書チェーン完結）は status=done。全 Golden/Negative/Crash case は計画時点で NotRun と明示された。

各実装 slice は spec/design/plan/task/test-spec/impl-ir の 6 文書チェーン + managed index 再生成で記録され、IR に実行した検証コマンド・結果・NotRun 項目・follow-up を集約する運用が一貫している。ただし証拠記録のみの slice（OS-L1 evidence 照合）では完全チェーンを重複作成せず IR + evidence report に集約する例外運用が取られた（synvelis#84）。

## 文書状態の注意（食い違い・未更新箇所）

HEAD CAS の task 文書は status=done・Done When 全項 `[x]` だが、末尾の Verification チェックリストに mise コマンドの `[ ]` が残っている。IR には全コマンド pass の実測記録があり、文書のチェックボックス未更新と読める（synvelis#330）。

SYN-OBS-01 の task1〜6 はいずれも status=done だが、Done When のチェックボックスが全て `[ ]` のまま。IR は全 lane pass と task6 の status 更新を完了条件に含むため、完了自体は IR 記録で裏付けられる（synvelis#81）。

store StagedFile 移行の IR は Summary で契約テスト「16 tests」と記す一方、変更ファイル一覧では「新規・14 tests」と記載する。同一文書内の P1 修復節を読むと初期 14 件 + 修復で追加した fault 試験 2 件 = 16 件で整合するが、一覧側の数値は修復前のまま残っている（synvelis#84）。

HEAD CAS の plan 文書は Implementation Sequence のコード項目が `[x]`、mise 検証項目が `[ ]` のまま。IR で全検証 pass が記録されており、plan は最終検証前の状態で止まっている（synvelis#330）。また plan/IR の ruviform 解決 SHA は slice ごとに異なる（store 移行期 `49de02cc`、HEAD CAS/Windows lane `fd1d4d5f`、RF305 `27698cb1`）— 時系列の異なる pin であり矛盾ではないが、文書をまたぐ際は各 slice の記載値をそのまま使う必要がある。

## 出処

synvelis#82（SYN-IO-01 境界設計・SYN-RF305 WAV Export 移行）、synvelis#84（SYN-IO-02 store StagedFile 移行・実機検証・OS-L1 照合）、synvelis#330（HEAD CAS 実装・実 2-process E2E）、synvelis#81（SYN-OBS-01 Host logging・Source catalog）、synvelis#184（canonical temp fixture、in-progress）、ruviform#288/#305/#291/#292/#293（upstream 側の primitive・後続 Issue）。
