# sv#516 binding-fit 照合表 — synvelis-script port ↔ ruviform-script 中立契約

Date: 2026-11-17。Slice: interim `synvelis-script-luau`(mlua) →
`ruviform-script`/`ruviform-script-luau` port 交換判定。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/516
（OPEN・`Refs` のみ・Close しない）。
Branch: `feat/516-mlua-adapter-decision`（worktree `sv516-mlua-adapter`）。

比較対象:

- synvelis 側: `crates/synvelis-script/src/backend.rs`（923 行 —
  consumer-side port mirror）+ `crates/synvelis-script-luau/src/lib.rs`
  （627 行 — interim mlua 実装、main `acc0198c` 時点）。
- ruviform 側: `crates/ruviform-script/src/{session,value,budget,host,
  id,registry,capability,canonical,codec,analyzer}.rs` +
  `crates/ruviform-script-luau/src/{runtime,analyzer,ffi}.rs`。
- ruviform 解決 rev: `84a68f25d5afd1b1f201018d76894d25b122f150`
  （main HEAD・PR #734 まで。旧 pin `cbcc8343` には
  `ruviform-script-luau` crate 自体が存在しない — runtime backend
  `dce45712`/PR #729 以降が必須）。

凡例: **適合** = そのまま写像可能 / **差分** = 写像は可能だが語彙・
形状・分類が異なる（adapter で吸収）/ **不足** = 中立契約側に
表現手段がなく synvelis 側で保持するしかない / **N/A** = 当該
consumer 経路では不使用。

## 1. Trait / session 署名

| 項目 | synvelis port (`backend.rs`) | ruviform (`session.rs`) | 判定 |
| --- | --- | --- | --- |
| backend trait | `ScriptBackend: fmt::Debug`、`open_session(&self, &SessionConfig)`（by-ref） | `ScriptBackend`（Debug bound なし）、`open_session(&self, SessionConfig)`（by-value） | 差分 — adapter が by-ref を保持し by-value へ変換 |
| session trait | `evaluate(&mut self)`、`session_id() -> &str`、`cancel(&mut self)`、`close(&mut self)` | `evaluate(&mut self, Option<&str>)`、`session_id() -> SessionId`、`cancel()` default→`cancel_token()`、`cancel_token()`、`close()` | 差分 — adapter が `evaluate(None)` 固定・`luau-mlua-<n>` 文字列 id を維持・cancel を token へ転送 |
| entry point | 引数自体なし（概念なし） | `evaluate(entry)` — 製品 backend は `Unsupported` で拒否 | 適合 — synvelis は entry を使わない |
| cancel | `cancel(&mut self)` のみ（token 非公開） | `cancel()` + `cancel_token() -> CancelToken`（clone 共有） | 差分 — adapter の `cancel` は inner `cancel()`（= token.cancel()）へ転送。consumer は token を触らないため公開面不変 |
| session 回数 | evaluate-once（2 回目 `ContractViolation`） | evaluate-once（2 回目 `Internal`） | 差分 — adapter が自前 flag で `ContractViolation` を維持 |
| close 後 evaluate | `Backend{"session is closed"}` | `Internal{"evaluate after close"}` | 差分 — adapter が自前 flag で `Backend` を維持 |

## 2. SessionConfig / ScriptSource

| 項目 | synvelis | ruviform | 判定 |
| --- | --- | --- | --- |
| config fields | `budget`, `source`, `artifact_kinds` | `budget`, `host_bindings`, `artifact_kinds`, `source`, `placement_hint`, `resume` | 差分 — adapter は `host_bindings=empty()`・`placement_hint=None`・`resume=None` を固定送出 |
| source fields | `text`, `language`, `backend_hint`, `digest`, `revision` | 同 + `schema_version`（`ScriptSource::new` が自動 stamp） | 適合 — adapter が `rf::ScriptSource::new` で構成 |
| digest 形式 | `sha256:<hex>`（裸 SHA-256・algorithm tag） | `sha256:v1:<hex>`（canonical form commit） | 差分 — adapter が open 時に synvelis 形式を検証し、rf へは `ScriptSource::new` の自動 digest を渡す。artifact provenance は synvelis 形式を維持 |
| language id | `ScriptLanguageId(pub String)`、`::luau()` = `"luau"` | `ScriptLanguageId` newtype、`luau_language()` = `"luau"` | 適合 |
| revision | `SourceRevision(pub u64)` | `SourceRevision(u64)` + `new`/`get` | 適合 |

## 3. ExecutionBudget / cancel

| 項目 | synvelis | ruviform | 判定 |
| --- | --- | --- | --- |
| fields | `max_instructions`/`max_wall_time`/`max_memory_bytes`/`max_output_bytes`/`max_host_calls`（全 `Option`） | 同一 5 field・全 `Option` | 適合 — 1:1 転記 |
| requested kinds | `requested_kinds() -> BTreeSet<BudgetKind>` | なし（backend が個別判定） | 適合 — adapter が requested_kinds で fail-closed 事前検証 |
| BudgetKind | `Instruction`/`WallClock`/`Memory`/`Output`/`HostCall` | `Instructions`/`WallTime`/`MemoryBytes`/`OutputBytes`/`HostCalls` | 差分 — 相互変換表を adapter に保持 |
| instruction budget | 拒否（`UnsupportedBudget{Instruction}`） | 拒否（`UnsupportedBudget{Instructions}`） | 適合 |
| host-call budget | 拒否（`UnsupportedBudget{HostCall}` — enforced 集合外） | **受理（vacuous — host call 経路が存在しない）** | **差分・adapter で保持** — synvelis port は「要求された次元は enforce 必須」なので adapter が `HostCall` を事前拒否。rf の vacuous accept をそのまま通すと port 契約を弱める |
| wall clock | Luau interrupt deadline | 同（interrupt callback deadline） | 適合 |
| memory | `Lua::set_memory_limit`（0・overflow は拒否） | 同（`validate_budget` + `set_memory_limit`） | 適合 |
| output | 直列化 JSON size 検査 | canonical 直列化 size 検査 | 適合（計測対象の byte 表現は異なるが同じ「出力上限」意味論 — rf は canonical 形で計測） |
| cancel granularity | `Safepoint` を広告 | `Cooperative` を広告 | 差分 — adapter descriptor は `Safepoint` を維持（interrupt safepoint 観測は実態として正しい）。consumer は `cancel_granularity` を resolve 条件に使っていない |
| CancelToken | なし（session 内部 flag のみ） | `CancelToken`（Arc<AtomicBool> clone 共有） | 差分 — adapter は公開しない（port に token accessor がない）。内部で inner token を経由 |

## 4. ScriptValue

| 項目 | synvelis | ruviform | 判定 |
| --- | --- | --- | --- |
| variants | Absent/Null/Bool/Integer(i64)/Number(f64)/String/Bytes/Array/Record/**Time(i64)**/**ResourceId(String)** | Absent/Null/Bool/Integer(i64)/Number(f64)/String/Bytes/Array/Record/**Time{domain,timebase,count}**/**ResourceId{domain,kind,value}** | 差分 — Time/ResourceId は domain 修飾型。Luau lift は両者とも生成しないため実害なし。adapter は出現時 `ContractViolation` で fail-closed（synvelis の flat 形へは lossy 変換しない） |
| 非有限数 | lift 時 `ValueOutOfRange` で拒否 | `Number(f64)` として保持（canonical は決定的 tag record を出力） | 差分 — adapter が変換時に `!is_finite` → `ValueOutOfRange` で interim と同じ拒否を維持。artifact 直下の非 record payload は rf が `Internal` で先に拒否（分類差は §6 参照） |
| NaN/±inf artifact | `ValueOutOfRange` | `Internal{"artifact payload must be a record"}`（Number は Record でない） | 差分 — record 内包なら adapter が `ValueOutOfRange` を維持。bare scalar artifact は `Internal`（rf の record 必須契約が先に効く）。テストは record 内包形に更新 |
| lift 規則 | Nil→Null、整数 f64→Integer、table→record/array、mixed/sparse→`ContractViolation` | 同一規則（同一 lift ロジックを上流が所有） | 適合 — ただし分類は `Internal`（§6） |
| JSON 変換 | `to_json`/`from_json`（serde_json 経由・Absent record field 省略・Bytes/Time/ResourceId は JSON 不可） | `canonical_value`/`materialize`（tag record codec） | 差分 — adapter は `to_json`→`digest_of` で synvelis canonical digest を再計算。payload decode（`from_script_value`/serde_json）は consumer 側不変 |

## 5. ScriptArtifact / provenance

| 項目 | synvelis | ruviform | 判定 |
| --- | --- | --- | --- |
| payload 型 | `ScriptValue`（返り値全体をそのまま保持 — scalar も可） | `BTreeMap<String, ScriptValue>`（**Record 必須** — 非 Record は `Internal`） | 差分 — 製品契約の方が厳格。synvelis consumer は常に spec record を期待するため実害なし。adapter は record map → `ScriptValue::Record` で包み戻す |
| digest | `DefinitionDigest` = `digest_of(to_json(payload))` の裸 hex | `DefinitionDigest` = `"sha256:v1:<hex>"`（canonical_value commit） | 差分 — adapter が synvelis canonical で再計算（interim と同一式）。consumer は digest を検証していない（importer は `source_digest` のみ pin） |
| source provenance | `ArtifactSource{language, backend_id, source_digest, source_revision}` | `ArtifactSource{source_digest, source_revision}`（language なし） | 差分 — adapter が session 開時の synvelis source を保持し同形で再構成 |
| produced_by | `ProducedBy{backend_id, impl_version}` | `ProducedBy{backend, version: ScriptBackendVersion{impl_version, contract_version}}` | 差分 — adapter は `impl_version` に**製品 backend の descriptor 値**（`"0.1.0+luau<rev>"` 系）を記録 — 実際に VM を走らせた実装を正直に名乗る |
| schema_version | `SCRIPT_ARTIFACT_SCHEMA_VERSION = 1` | `ARTIFACT_SCHEMA_VERSION = 1` | 適合 |

## 6. Error vocabulary

| 項目 | synvelis `ScriptErrorKind` | ruviform `ScriptErrorKind` | 判定 |
| --- | --- | --- | --- |
| closed set | Parse/Compile/Type/Runtime/**BudgetExceeded{kind}**/Cancelled/Unsupported{what}/Host{detail}/Backend{detail}/Internal{detail}/**ValueOutOfRange**/**ContractViolation{detail}** | Parse/Compile/Type/Runtime/**BudgetExceeded**（payload なし）/Cancelled/Unsupported/Host/Backend/Internal | 差分 — 中立契約は `Copy` unit enum で payload を持たない。`BudgetExceeded` の次元情報・`ValueOutOfRange`・`ContractViolation` は synvelis 独自 |
| BudgetExceeded kind | `BudgetKind`（今回 `Option<BudgetKind>` へ — 製品 error が次元を運ばないため） | なし | **port 変更（lease 内 `backend.rs`）** — 単一 armed 次元なら `Some`、複数 or 0 なら `None`（推測しない） |
| lifted-shape 違反 | `ContractViolation{detail}` | `Internal` | 差分 — adapter は `Internal`→`Internal` で 1:1 写像（backend bug を script 違反に偽装しない）。`ContractViolation` は binding 自身が検出する違反（domain 修飾値・未知 variant・2 回目 evaluate・close 後 evaluate 以外の port 契約違反）に限定 |
| evaluate-after-close | `Backend{"session is closed"}` | `Internal{"evaluate after close"}` | 差分 — adapter が `Backend` を維持 |
| 2 回目 evaluate | `ContractViolation{"session already evaluated"}` | `Internal{"session already evaluated"}` | 差分 — adapter が `ContractViolation` を維持 |
| entry 指定 | （概念なし） | `Unsupported` | N/A — adapter は常に `None` |
| error context | `position`/`backend_id`/`session_id` field | `range: Option<SourceRange>` のみ | 差分 — adapter が backend_id/session_id を付与（interim と同じ）・range.start→position 変換 |
| SessionError | UnsupportedBudget/UnsupportedArtifact/SourceDigestMismatch/SourceTooLarge/LanguageMismatch/IsolationViolation/ResourceExhausted/Backend | UnsupportedBudget/InvalidHostSurface/SourceDigestMismatch/SourceTooLarge/LanguageMismatch/IsolationViolation/ResourceExhausted/Backend | 差分 — `UnsupportedArtifact`・`InvalidHostSurface` は片側のみ。adapter が open 前検証で `UnsupportedArtifact` を保持・`InvalidHostSurface` は empty surface 固定で到達不能 |
| 失敗順序 | language→size→digest→budgets→artifact→VM→memory値→wall overflow | language→host→size→digest→resume→schema→budget値→artifact→VM→memory値 | 概ね適合 — adapter が synvelis 順序を先に検証してから委譲。唯一の差: VM 生成失敗 vs budget 値拒否の優先順位（interim は VM 生成→memory/wall 検査、製品は budget 検査→VM 生成）。VM 生成失敗は実質到達不能な pathological case で、製品側の「検証を VM 生成より前に」という順序はより健全 — 記録のみ |

## 7. BackendCapabilities / descriptor / registry

| 項目 | synvelis | ruviform | 判定 |
| --- | --- | --- | --- |
| descriptor fields | `id`/`language`/`version`/`capabilities` | `backend_id`/`language`/`version`/`capabilities` | 差分 — adapter が synvelis descriptor を保持（値は interim と同一: id=`luau-mlua`、contract_version=1、enforced={WallClock,Memory,Output}、max_source=1MiB、artifact={WF,AR}、send/sync=false、owner_thread_only、cancel=Safepoint、native_integer=false） |
| capabilities fields | `enforced_budgets`/`max_source_bytes`/`artifact_kinds`/`send`/`sync`/`owner_thread_only`/`cancel_granularity`/`native_integer` | `artifact_kinds`/`capability_ids`/`owner_thread_only`/`send`/`sync`/`shared_vm_state`/`cancel_granularity` | 差分 — 語彙が異なる（rf は `shared_vm_state`・`capability_ids`、synvelis は `enforced_budgets`・`max_source_bytes`・`native_integer`）。adapter は synvelis descriptor をそのまま広告 — 実態と整合（enforced budgets・1MiB・owner-thread・Safepoint は全て真） |
| registry | `ScriptBackendRegistry`（`Arc<dyn ScriptBackend>`、BTreeMap、独自 `ScriptRequirement`/`ResolveError`） | `ScriptBackendRegistry`（`Box<dyn ScriptBackend>`、BTreeMap、`ScriptRequirement`/`ResolveError` 別語彙） | **N/A（consumer 未使用）** — synvelis consumer は registry を経由せず `LuauBackend` を直接注入（importer の `Arc<dyn ScriptBackend>`・CLI の直接構築）。registry 語彙差は現行 consumer に影響しない。将来 registry 解決を consumer が必要とした場合の移行条件として記録 |
| version | `ScriptBackendVersion{impl_version, contract_version}` | 同形 | 適合 |

## 8. Analyzer port（参考 — 本 slice では交換しない）

| 項目 | synvelis `analyzer.rs` port | ruviform `analyzer.rs` + `LuauAnalyzer` | 判定 |
| --- | --- | --- | --- |
| 実装 | `LuauPortAnalyzer`（synvelis-automation/src/analyzer.rs — interim port 実装、native bridge なし） | `LuauAnalyzer`（vendored Luau.Analysis 0.740 + C ABI shim — **native bridge**） | **別 gate** — sv#274（SV-SCRIPT-04）が公式 Luau.Analysis 接続を担当。本 slice は runtime port のみ交換。analyzer の製品 backend への切替は #274 の判断に委ねる |
| 語彙 | `AnalyzeRequest`/`AnalyzeResult`/`Diagnostic`/`Completion`/`AnalyzerError`（`StaleResult`/`Unavailable`/`Unsupported`） | 同名語彙・別 crate 型 | 差分（未接続） — 移行時は adapter が必要になる見込み。現時点では consumer（`synvelis-automation/src/analyzer.rs`）が interim port を消費中で回帰なし |

## 9. 製品 backend の非目標（ruviform lib.rs 明記）

- host binding 実行 seam — 中立契約に handler 経路なし（記述子検証・受理のみ）。synvelis は `SessionConfig` に host surface がなく `WorkflowScriptBuilder` が authority-free — **現行 consumer 要件と一致**。
- named entry points — `evaluate(entry)` は `Unsupported`。synvelis は entry を使わない。
- evaluate 再実行 — single-shot（両実装同じ）。
- Resume VM 復元 — `resume` は digest pin のみ（VM 状態復元ではない）。synvelis は `resume=None` 固定。
- `max_instructions` — `UnsupportedBudget`（mlua に API なし）。synvelis も拒否 — **一致**。
- 3-OS 実機受入 — Windows `x86_64` のみ実測（ruviform 側記録）。macOS/Linux は NotRun — consumer gate（synvelis#516・#267）へ委譲。本 slice も Windows のみ実測。

## 10. 判定サマリ

| 分類 | 件数（概算） | 代表例 |
| --- | --- | --- |
| 適合（そのまま写像） | ~15 | budget 5 field、ArtifactKind、artifact schema_version、language/revision id、lift 規則、sandbox globals、single-shot、owner-thread-only |
| 差分（adapter で吸収） | ~18 | trait 署名、digest 形式（`sha256:` vs `sha256:v1:`）、SessionConfig 追加 field、ScriptErrorKind payload、Internal vs ContractViolation、artifact payload Record 必須、impl_version provenance、registry 語彙 |
| 不足（中立契約に表現なし → synvelis 側で保持） | ~4 | `BudgetExceeded` 次元属性（→ `Option<BudgetKind>`）、`ValueOutOfRange`、`ContractViolation`、`UnsupportedArtifact` SessionError、source digest `sha256:` タグ検証、multi-budget 時の next 元不明 |
| N/A（consumer 未使用） | ~3 | registry 解決、analyzer port、placement_hint/resume/host_bindings |

**結論**: 直接 crate 交換は不可能（trait・error・digest・config が
全面的に異なる）だが、**薄い binding（anti-corruption adapter）で
synvelis port 公開面を維持したまま製品 backend を消費する分岐が
成立する**。consumer（`synvelis-cli`/`synvelis-automation`）は
変更不要 — 実測で確認済み（automation 全 test green・cli check 通過）。
interim mlua 実装の重複（sandbox globals・lift・budget・digest・
artifact provenance）は上流 `ruviform-script-luau` に一元化される。
