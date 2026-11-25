# Evidence — synvelis#127 VST3 TUID platform byte order（Linux/macOS CI 退行の調査と修正）

post-merge batch CI（run `45605002411`、main HEAD `63182927`）で
`tim03_real_abi_vst3_discovery_lifecycle_and_fail_closed_boundary` が
ubuntu-latest test-full・msrv-verify・verify-macos の全てで FAILED となった退行の
root cause 調査と修正の記録。CLAP 側は全 OS pass、Windows verify も pass だった。

## 実際の失敗内容（CI ログ実測）

失敗は descriptor count ではなく、`test_gain` の descriptor/class id 値の mismatch
だった（`crates/synvelis-app/tests/tim03_real_abi_process_oracle.rs:439`）。

| lane | left（実測値） | right（旧期待値） |
| --- | --- | --- |
| verify (ubuntu-latest) test-full | `6d4f8a213b7c9e05f1a24c68b9d37e51` | `218a4f6d7c3b059ef1a24c68b9d37e51` |
| verify-macos test-full | `6d4f8a213b7c9e05f1a24c68b9d37e51` | 同左 |
| msrv-verify test-msrv | `6d4f8a213b7c9e05f1a24c68b9d37e51` | 同左 |

scan 自体は全 OS で成功し、`test_gain` は name で発見できている（count mismatch や
artifact 未解決ではない）。CLAP は文字列 id のためプラットフォーム差を持たず pass。

## Root cause

fixture の class id は `ruviform-plugin-test-vst3/src/component.rs:226` で
`vst3::uid(0x6D4F8A21, 0x3B7C9E05, 0xF1A24C68, 0xB9D37E51)` として定義される
論理値 1 つだが、`vst3` binding crate（v0.3.0）の `uid()` constructor は
**設計上プラットフォーム依存**である。crate 自身の doc が明記する:

> Note that the byte order of the resulting value will differ between Windows and
> other platforms.

`vst3-0.3.0/src/support.rs` の cfg 分岐:

- `#[cfg(target_os = "windows")]`: COM GUID の mixed-endian memory layout。
  先頭 u32 を little-endian、続く 2 つの u16 フィールドを little-endian、
  末尾 8 bytes はそのまま。byte 列は
  `21 8A 4F 6D 7C 3B 05 9E F1 A2 4C 68 B9 D3 7E 51` → hex
  `218a4f6d7c3b059ef1a24c68b9d37e51`（Windows 実測と一致）。
- `#[cfg(not(target_os = "windows"))]`: Steinberg `INLINE_UID` 由来の
  big-endian order。byte 列は
  `6D 4F 8A 21 3B 7C 9E 05 F1 A2 4C 68 B9 D3 7E 51` → hex
  `6d4f8a213b7c9e05f1a24c68b9d37e51`（Linux/macOS 実測と一致）。

host scan（`ruviform-plugin-vst3` host）は raw TUID bytes を lowercase hex で render
するだけなので、同一論理 class でも rendered descriptor id はプラットフォームで
正当に分かれる。artifact discovery の欠陥でも、scan のプラットフォーム差異でも、
fixture binary の構成差でもない — **TUID byte order の設計上のプラットフォーム差**
であり、初版 test が Windows の値だけを全 OS の期待値にしていたのが退行の原因。

検証した除外仮説:

- descriptor count 差異説 → 否定。CI ログで scan は成功し count assertion には
  到達している（失敗は id 値 assertion）。fixture factory は `countClasses = 12`・
  category filter で 7 audio module classes を全プラットフォーム共通で返す。
- artifact discovery がプラットフォームで別バイナリを解決した説 → 否定。
  `test_gain` は name で発見できており lifecycle も通る。id の差は byte order の
  規則的な permutation であり、別 class/別 artifact では説明できない。

## 修正

`crates/synvelis-app/tests/tim03_real_abi_process_oracle.rs`:

- `EXPECTED_VST3_TEST_GAIN_ID` を `#[cfg(target_os = "windows")]` /
  `#[cfg(not(target_os = "windows"))]` で分岐し、両 layout の exact bytes を
  プラットフォーム条件付きで pin する。比較を緩める・正規化する・skip する
  方向は取らない（byte order の差異そのものが ABI の正直な記録対象）。
- 分岐根拠（`vst3::uid` のプラットフォーム依存設計・両 hex 値・論理 class の
  同一性）を test 内コメントと `factory_class_id_evidence` JSON 文字列に記録。
- spec/design/test-spec の id 期待値記述をプラットフォーム条件付きに更新。

## 検証

### ローカル（Windows x86_64、rustc 1.99.1、canonical = mise）

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Pass（exit 0） |
| `mise run check synvelis-app` | Pass（exit 0） |
| `mise run lint synvelis-app` | Pass（exit 0） |
| `mise run test synvelis-app` | Pass（exit 0。Windows cfg 側 id で oracle 2 tests pass） |
| `mise exec -- prek run --all-files` | Pass（exit 0） |

### 3OS CI（workflow_dispatch、branch ref）

workflow_dispatch: run `45613303160`（ref `scarecrow173/sv127-tim03-real-abi`、
expected_sha `460de2a286d6fd3443b354b4998579d3629543b9` = fix commit + 最新 main
`36b7da54` との merge）。**run 全体 conclusion: success**。

| lane | conclusion | oracle 実走確認（job log） |
| --- | --- | --- |
| verify (ubuntu-latest)（test-full 含む） | success | `tim03_real_abi_vst3/clap ... ok`（test-full・gpui lane 双方） |
| msrv-verify | success | `tim03_real_abi_vst3/clap ... ok` |
| verify-macos | success | `tim03_real_abi_vst3/clap ... ok` |
| verify (windows-latest) | success（rerun 後） | `tim03_real_abi_vst3/clap ... ok` |
| rt-verify / dep-guard / ui-verify / gw10-verify（3OS） / evidence / ci-summary | success | — |

退行時に FAILED だった 3 lane（ubuntu test-full・msrv-verify・verify-macos）は全て
green に回復し、VST3/CLAP 両 oracle test が各 lane で実走 pass したことを job log
で確認した。

#### verify (windows-latest) の初回 failure について（scope 外 flake の記録）

初回実行では verify (windows-latest) が 1 度 failure となったが、失敗内容は本修正と
無関係な sv#646 asset-library test `pending_commit_cancel_requested_still_replays`
（`crates/synvelis-app/tests/sv646_pending_commit_replay.rs:127`、
"import 25 manifest never reached committed" — import replay の polling timeout）で
あり、`tim03` ではない。変更面（VST3 期待値 const・docs）が library import replay に
因果的に影響し得ないこと、および `gh run rerun --failed` で**変更なしに success** へ
転じたことから、runner timing 由来の flake と判断し本 evidence に記録する
（sv#646 側の flake 自体は別領域の残件）。初回実行時点でも ubuntu test-full・
msrv-verify・verify-macos は success だった。

### 補足

- CI 検証対象 SHA `460de2a2` は fix commit + main merge のもの。本 evidence ファイルの
  追加は docs-only の follow-up であり、コード・テスト差分を変えないため 3OS 結果は
  流用できる（再検証が必要なコード変更は入っていない）。
- ローカル（Windows）では `mise run fmt-check` / `check synvelis-app` /
  `lint synvelis-app` / `test synvelis-app` が全て exit 0（oracle 2 tests pass）。
