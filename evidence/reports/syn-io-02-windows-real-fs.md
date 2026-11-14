# SYN-IO-02 Windows NTFS Real-FS Evidence — SV84-REAL-* opt-in lane (synvelis#84)

Issue #84「まず Windows 実機」行の残件 slice の実測記録。
`crates/synvelis-project/tests/real_fs_probe.rs`（全 test
`#[ignore]`、opt-in lane `mise run test-project-real-fs`、
CI 対象外）が、実 Windows ファイルシステム上で
`DirectoryBundleStore` の Snapshot `CreateNew` publish →
HEAD CAS/`Replace` publish を実 OS process kill
（`Child::kill` = `TerminateProcess` 相当）で中断し、
reopen/Recovery/Failed を Case ID `SV84-REAL-*` 付きで
pin する。

**process kill は OS の dirty page を失わせない
（power-loss ではない）** — 真の power-loss と
`PublishEffect::Indeterminate` の非破壊的誘発は自動化不能
であり、本書でも `NotRun` のまま正直に残す。CI/headless
fixture の Pass を本 lane の実測に読み替えない
（`syn-io-02-os-l1-evidence.md` 残ゲート表の原則どおり）。

## Anchor — 対象 SHA と実装状態

- **対象 SHA**: `dbf0820566694434d6f926173be4ca76d9c31286`
  （branch `feat/84-syn-io-02-windows-real-fs`、base =
  `origin/main` `c1d485a15207f7cf0c9bad58846ad42105f29d80`）。
  `crates/*/src/**` 差分ゼロ — store 契約は main 統合済み
  実装（PR #178 `b527e0e` StagedFile 移行・PR #342 `d653062`
  HEAD CAS）をそのまま実 FS で観測する。
- **Cargo.lock ruviform 解決 rev**:
  `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`（変更なし）。
- **doc chain**: `docs/{specs,designs,plans,tasks,
  test-specs,impl/ir}/syn-io-02-windows-real-fs.md`。

## 実行環境（実測・2026-11-15）

| 項目 | 値 |
| --- | --- |
| OS | Microsoft Windows 11 Pro 10.0.26300（win32） |
| FS | NTFS — bundle root を `SYNVELIS_REALFS_ROOT=C:\Users\akiyoshi\AppData\Local\Temp\synvelis-realfs-ntfs` に固定（`C:` volume = NTFS を `Get-Volume` で確認。workspace の `G:` は ReFS/DevDrive のため対象外） |
| toolchain | rustc 1.99.1 (58cf8f9c3 2026-10-13) / cargo 1.99.1（mise 管理） |
| command | `SYNVELIS_REALFS_ROOT=... mise run test-project-real-fs` → `cargo test -p synvelis-project --locked --test real_fs_probe -- --ignored --test-threads=1 --nocapture` |
| exit code | 0 — `test result: ok. 3 passed; 0 failed`（20.93s） |

## Case ID × Requirement × Result

| Case ID | Requirement（Issue #84「まず Windows 実機」行） | Result | Expected / Observed |
| --- | --- | --- | --- |
| SV84-REAL-01 | Crash→Recovery: Snapshot `CreateNew` publish 中の process kill → reopen で旧 HEAD 保持 or 完結した新 HEAD + orphan 保存。HEAD 誤進行・torn HEAD なし | **Pass** | 期待: HEAD は seed `r1.json` か完結 `r2.json` のみ、orphan は byte 完結。実測: 5 round 全て kill 成立（`status=Some(1)`）・reopen で `head=r1.json`・`head_raw` 一致・orphan `r2.json` は 16,777,216 byte 完結を byte 検証・`HEAD.lock.stale-*` 0 件（下記実測行） |
| SV84-REAL-02 | Crash→Recovery + `save_cas` retry: kill 後の retry が stale `HEAD.lock` を実 stale path（`HEAD_LOCK_STALE_AFTER`=20s）で break し一貫 HEAD へ回復 | **Pass** | 期待: retry `save_cas` が commit し HEAD が正確に 1 回だけ進む。実測: `observed_head=r1.json` + `lock_leftover=true` → retry は stale lock を待機 break して `head=r2.json`・`lock_cleared=true`・`HEAD.lock*` 残滓 0（約 20s の stale 待機を含む 20.93s lane） |
| SV84-REAL-03 | Failed で HEAD 誤進行なし: publish 失敗（immutable name の foreign occupant）と stale-expected CAS `Conflict` が HEAD を進めず、clean retry が回復 | **Pass** | 期待: `HeadCasError::Store`/`Conflict` で HEAD 不変・foreign byte 非破壊・retry で 1 回だけ commit。実測: foreign publish → `Store(... destination already exists (os error 183) ...)`・`head` は `r1.json` のまま・foreign byte 保持・stale CAS → typed `Conflict{expected=r9.json, actual=r1.json}`・`r3.json` 非生成・retry → `head=r2.json` |
| Indeterminate（`PublishEffect::Indeterminate` での HEAD 誤進行なし） | ambiguous OS error の非破壊的誘発が必要 | **NotRun** | 実 NTFS 上で Indeterminate を安全に誘発する手段がない（ruviform-io-file fault-injection 責務）。consumer 側は effect/phase/durability の分類転写を `store_publish` fixture で pin 済み（`syn-io-02-os-l1-evidence.md`） |
| 真の power-loss（電源断相当） | crash→durability の電断形 | **NotRun** | 非破壊的に自動化不能。process kill は dirty page を保持するため代替不可（RCD-02 `test-recording-real-fs` の実測と同じ結論） |

## SV84-REAL-01 実測行（kill-mid-publish → reopen）

trigger は親 process が bundle dir を spin-poll し、子が
publish window に入った瞬間に `Child::kill`。各 round は
fresh seeded bundle（seed = `r1.json`）で独立。

| round | op | trigger | kill | reopen 観測 |
| --- | --- | --- | --- | --- |
| cas-lock | `save_cas` | `HEAD.lock` 出現 | `status=Some(1)` | `head=r1.json` `snap_exists=false` `lock_leftover=true` `temp_residue=0` |
| cas-snaptemp | `save_cas` | `r2.json.rvf-tmp-*` 出現 | `status=Some(1)` | `head=r1.json` `snap_exists=false` `lock_leftover=true` `temp_residue=1` |
| cas-snapdone | `save_cas` | `r2.json` rename 済み | `status=Some(1)` | `head=r1.json` `snap_exists=true snap_len=16777216` `lock_leftover=true` `temp_residue=0` |
| save-snaptemp | `save` | `r2.json.rvf-tmp-*` 出現 | `status=Some(1)` | `head=r1.json` `snap_exists=false` `lock_leftover=false` `temp_residue=1` |
| save-snapdone | `save` | `r2.json` rename 済み | `status=Some(1)` | `head=r1.json` `snap_exists=true snap_len=16777216` `lock_leftover=false` `temp_residue=0` |

全 round で assert した不変条件: `head()` は seed/crash
snapshot 名のいずれかのみ・raw `HEAD` 内容と一致（torn
HEAD なし）・HEAD が crash snapshot を指す場合は
`read_snapshot` が crashed writer の byte と完全一致・
旧 HEAD 保持時の orphan `r2.json` は存在すれば byte 完結
（`CreateNew` atomic rename の裏付け）・`HEAD.lock.stale-*`
0 件・`save` は `HEAD.lock` を取らない。temp residue
（`*.rvf-tmp-*`）は観測値として記録し、store が回復データを
自動削除しない契約と整合。

## SV84-REAL-02/03 実測行

- `SV84-REAL-02`: kill 後 `observed_head=r1.json`・
  `lock_leftover_before_retry=true` → fresh store で
  `save_cas(expected=r1.json, rev=2)` retry → stale lock
  break（`created_ns` 基準 20s）を経て `head=r2.json`・
  `lock_cleared=true`・`HEAD.lock*` 残滓 0。crash writer
  と byte 同一の payload のため `Satisfied` collision 経路で
  orphan `r2.json` を再 publish 採用。
- `SV84-REAL-03`: `snapshots/r2.json` に foreign occupant を
  置いて `save_cas` → `HeadCasError::Store`（publish
  `DestinationExists` → `SnapshotCollision::Conflict` →
  `Error::Store`、os error 183 = `ERROR_ALREADY_EXISTS`）。
  HEAD は `r1.json` のまま・`HEAD.lock` 解放済み・foreign
  byte 非破壊。stale expected `r9.json` の CAS は publish
  前に typed `Conflict`（`r3.json` 非生成）。foreign 除去後
  の同一 CAS retry は `head=r2.json` へ 1 回だけ commit。

## Requirement×Result 直交記録（POLICIES.md）

| requirement | environment_kind | result |
| --- | --- | --- |
| Snapshot `CreateNew` → HEAD CAS/`Replace` publish の kill-mid-flight → reopen/Recovery | 実 Windows 11 / NTFS（process kill） | Pass（SV84-REAL-01/02） |
| Failed（publish 失敗・CAS `Conflict`）で HEAD 誤進行なし + retry 回復 | 実 Windows 11 / NTFS | Pass（SV84-REAL-03） |
| `Indeterminate` publish で HEAD 誤進行なし | 実 Windows 11 / NTFS | NotRun（非破壊的誘発不能） |
| 真の power-loss durability | 実 Windows 11 / NTFS | NotRun（自動化不能・process kill は代替不可） |
| headless fixture（write-back/二重 publish/旧 writer 上書き/orphan 禁止） | CI fixture | Pass（`syn-io-02-os-l1-evidence.md` 別記録。本書の Pass に転用しない） |

## 本書が成立させるもの・させないもの

- **成立**: 実 NTFS 上での Snapshot `CreateNew`/HEAD
  `Replace` publish の process-kill 中断 → reopen で
  「旧 HEAD or 完結新 HEAD + orphan 保存」不変条件、
  stale `HEAD.lock` の実 break を伴う `save_cas` retry
  回復、Failed/`Conflict` での HEAD 非進行と retry
  回復 — 全て Case ID・実測値付きで Pass。
- **不成立（主張しない）**: 真の power-loss durability、
  `Indeterminate` effect の実 FS 再現、directory 全体の
  atomic transaction、Network/Cloud FS、macOS/Linux 実機
  （portable code のため CI leg が被覆 — 別証拠）、
  Import↔Save 結合 Case（sv#86/#91 責務・範囲外）。
