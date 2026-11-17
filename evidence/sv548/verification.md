# sv548 — strip-domain inner op lowering evidence

Issue: `scarecrow173/synvelis#548` / branch
`feature/548-strip-domain-lowering`（base: `origin/main`
`9155f204`、前提 PR `synvelis#547` merge 済み）
OS: Windows（mise managed toolchain）。cargo 直接実行は
補助証拠、canonical = mise。

## Lane 別 Pass/NotRun

| Lane | 判定 | 根拠 |
| --- | --- | --- |
| OS-L1 headless/fixture | **Pass** | 下記 mise 検証コマンド全 pass。`cb06_strip_fields_mask_gated_edit_wire_lowering`・`cb06_strip_fields_insert_ops_wire_lowering` が strip-domain inner op の positive mask-gated edit を headless で実測 |
| GUI-L2 | **NotRun** | GUI surface は対象外（headless gateway 契約のみ） |
| EXT-L3 実 render/RT・実 plugin | **NotRun** | 実機/実 PCM/実 RT は外部 gate（Issue が明示的に対象外） |
| 実 CLI/MCP/GUI E2E | **NotRun** | host 側 E2E は本 slice の検証範囲外 |
| MSRV lane | **NotRun** | ユーザー指定により実行しない |

## 実行した検証（全て exit 0）

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Pass（初回は新規コードの fmt 差分で fail → `cargo fmt --all` 適用後 pass） |
| `mise run check synvelis-gateway` | Pass |
| `mise run lint synvelis-gateway` | Pass |
| `mise run test synvelis-gateway` | Pass（33 test binaries 全 pass・0 failed。`cb06_strip_gateway` 4 件 — lifecycle・boundary/capabilities・mask-gated edit wire lowering・insert ops wire lowering を含む。`tests/prepare.rs` 21 件 — strip-domain prepare 3 件を含む） |
| `mise run check synvelis-project` | Pass（consume のみ・非回帰確認） |
| `mise run check-headless-deps` | Pass（12 headless crates に GUI framework deps 無し） |
| `mise exec -- prek run --all-files` | Pass |

## 補助証拠（canonical = mise）

- `mise run test synvelis-gateway -- cb06_strip_fields_mask_gated_edit_wire_lowering`:
  Pass — 狭 mask（`fader`+`mute`+`solo`）で set-mute/
  set-solo/param.set-base の 3 positive edit が candidate
  payload に commit（content revision 1→4・live revision
  +3・live fader base 不変）→ compare `verified:true`・
  `scope_digests_equal:false` → inserts 系
  `variant-unsupported`・master strip `variant-scope-violation`・
  `synvelis.clip.mute` `variant-unsupported` の負側 3 件・
  reject は revision 非消費。
- `mise run test synvelis-gateway -- cb06_strip_fields_insert_ops_wire_lowering`:
  Pass — 全 5 field mask で insert-processor/
  processor.set-binding の 2 positive edit（revision 1→3・
  live chain は seeded `fx` のみ・binding 不変）→ 非 strip
  op 負側維持。
- `mise run test synvelis-gateway -- prepare_strip` /
  `prepare_param_set_base`: Pass — ordinary prepare で
  set-mute/set-solo/param.set-base/insert-processor が
  lower・proposal retained・candidate が would-be 状態・
  committed project byte-identical。
- `mise run test synvelis-gateway -- prepare_strip_domain_args_fail_typed`:
  Pass — schema/type 違反・不正 binding・存在しない
  processor が typed reject・proposal 0 件。

## 変更サマリ

- `crates/synvelis-gateway/src/prepare_engine.rs`:
  `op_table` に `synvelis.strip.set-mute`/`set-solo`/
  `insert-processor`・`synvelis.param.set-base`・
  `synvelis.processor.set-binding` の descriptor + schema
  を追加（TargetContract は `Resource{Strip}`/
  `Resource{ParameterTarget}`/`Resource{Processor}`、
  effects は bool/insert が `[Create, Write]`・
  set-base/set-binding が `[Write]`）。
- `crates/synvelis-gateway/src/prepare.rs`:
  target helper 3 件・lower fn 5 件・`lower_for`/`op_name`/
  `op_facts` arms（reads + preconditions を正直に宣言、
  catch-all `unsupported` には落とさない）。
- `crates/synvelis-gateway/tests/cb06_strip_gateway.rs`:
  module doc 更新・op builder 追加・positive mask-gated
  edit 試験 2 件・負側維持。
- `crates/synvelis-gateway/tests/prepare.rs`:
  ordinary prepare 試験 3 件追加。
- doc chain: `docs/*/issue-548-strip-domain-lowering.md`
  （slug 固定 8 type）。cb-06 doc 3 件に追補。
- `evidence/sv283/acceptance-ledger.md`・
  `verification.md` に gateway positive 経路を追記。

## 設計判断の適用

- `op_facts` arms 必須（設計判断1）: 実施 — 登録 capability
  が ordinary prepare で常時 fail-closed になる半状態を
  回避。
- GW-04 op 表慣例（設計判断2）: `synvelis.<domain>.<op>`・
  version 1.0・TargetContract は書き込み先資産・effect
  bounds は first-use create を正直に反映。
- 「routing mask 拡張」= gateway op dispatch 拡張として
  実装。send/connection/sidechain/bus op の candidate 受理・
  scope entity removal は lease 外の別 slice。
- `doc-driven-dev-graph` route は既存 broken relation
  （`docs/impl/ir/cb-09-diff-explorer.md` → `evidence/sv286/`）
  と focus 未指定で `blocked` — リポジトリ側の既知 graph
  blocker であり本 slice の欠陥ではない。文書チェーンは
  lease 済みパスへ作成。

## NotRun / Deferred

- 実 OS provider・実 Plugin host・外部 Plugin GUI・実 RT・
  実 Render pipeline・実デバイス入力 pipeline・workspace/
  GPUI projection: NotRun（headless fixture のみ。
  Fake/Domain Oracle の PASS を実機 PASS に昇格しない）。
- Routing op（send/connection/sidechain/bus 編集）の候補
  受理: 引き続き `Unsupported` — `synvelis-project` の
  mask/adapter 変更を要し別 slice。
- scope entity removal（processor/target/lane/source）の
  候補編集: 引き続き `Unsupported` — 別 slice。
- strip 以外の domain（midi-clip・instrument-slot 等）の
  lowering 追加・positive 化: 別 Issue。
- `AddParameterTarget`・automation lane/region・modulation
  系の wire capability: 本 slice では見送り（schema が重く
  domain 検証責務も別）。
- docs index（`docs/**/README.md`）: Worker は編集・
  再生成・`index apply` を実行していない — Coordinator が
  統合時に再生成。
