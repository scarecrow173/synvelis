---
id: "ADR-ODnVCNLDT5mtpswKxVDw9R"
type: "adr"
status: "accepted"
title: "VS-B SPSCプリミティブ・Gate Schedule表現・Fault通知・Chunk所有権"
created: "2026-11-02"
updated: "2026-11-02"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/14"
    - "https://github.com/scarecrow173/synvelis/issues/23"
    - "https://github.com/scarecrow173/synvelis/issues/12"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/adr/vs-a-workspace-ruviform-git-poc-stable-id.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vs-b-rt-capture-mock-input-buffer-pool-spsc.md"
    - "docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vs-b-rt-capture-mock-input-buffer-pool-spsc.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/187"
    - "https://github.com/scarecrow173/ruviform/issues/190"
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# VS-B SPSCプリミティブ・Gate Schedule表現・Fault通知・Chunk所有権

## Context and Problem Statement

VS-B（Synvelis #14）は Mock Input→Gate→RT Capture→事前確保 SPSC Handoff
の経路を実装する。RT 契約（非 Block・非割当・Heap-owned Chunk を RT で
Drop しない・Fail-stop）を満たすために、(1) SPSC プリミティブの選定、
(2) Gate Schedule の有界表現、(3) Fault 通知経路、(4) 失敗 `try_push`
返却 Chunk の扱い、(5) `CaptureSpan` の格納場所を固定する必要がある。
Queue 容量・Chunk サイズ・Span 上限などの実数値は PoC 測定後に確定する
ため、本 ADR は構造と所有権のみを決定する。

## Decision Drivers

* #12 §17.2/§18.1-3: `ruviform-rt::channel` は既存の有界 SPSC であり、
  `try_push` は Full 時に payload を返す。Queue-full policy と Drop/Owner
  契約は利用側が設計する。
* #12 §18.1-5: Queue Full/Free 枯渇/Span 容量超過は Atomic Fault/Fail-stop。
  失敗 `try_push` が返す Heap-owned Chunk を RT で Drop せず、固定 Slot
  等で非 RT teardown まで保持。Fault 通知を満杯のデータ Queue に依存
  させない。
* #12 §17.1-5: 有限 Lookahead Schedule を非 RT で準備。無期限 Loop 全周を
  事前展開せず、補充不能は安全停止。
* VS-A ADR: Ruviform は既定ブランチ追従の git 依存・lockfile に resolved
  revision を記録。Ruviform に DAW 固有意味論を持ち込まない。
* RT 契約: callback 内で alloc/dealloc/lock/OS I/O/panic/無界ループ不可。

## Considered Options

SPSC プリミティブ:

* Q-1（採用）: `ruviform-rt` の `channel<T>`/`Producer`/`Consumer` を
  git 依存で追加利用。rtrb ベース・容量固定・`try_push` が
  `TryPushError::Full(value)` で payload を返す。`TryPushError` は
  `#[non_exhaustive]` で現行 `Full` のみ。
* Q-2: Synvelis 独自のリングバッファ SPSC を実装。
* Q-3: `std::sync::mpsc` / crossbeam 等の MPMC/MPMC 系チャネル。

Gate Schedule 表現:

* G-1（採用）: 固定容量の rolling Lookahead Window。`GateSpan`（半開
  `[session_in, session_out)` + `pass_index` + Project 区間）の有界列と
  `scheduled_until` ウォーターマークを持ち、非 RT が順序補充する。
  RT は Window 先頭から消費し、`scheduled_until` 超過は Fault で安全停止。
* G-2: Session 全体の Gate 領域を Preflight で全展開する固定配列。
* G-3: 可変長 `Vec` で必要時に追加（容量非有界）。

Fault 通知:

* F-1（採用）: `Arc<AtomicU32>` 相当の共有 Fault Latch（bit 立て + 読み
  出し）を RT/非 RT が共有。データ Queue とは独立。
* F-2: `filled` Queue に Fault メッセージを流す in-band 通知。

失敗 `try_push` 返却 Chunk:

* O-1（採用）: Engine 内の固定 `retained` スロットへ move して保持し、
  非 RT shutdown/retire で回収・Drop。RT 上では move のみ（Box の
  dealloc を起こさない）。
* O-2: RT 上で Drop（その場で dealloc）。
* O-3: 別の retire 専用 SPSC へ再 push（`ruviform-rt` の
  `retire_channel`/`Retirer`/`DropCollector` という既存プリミティブも
  あるが、full 時は payload を返す方針の別 Queue が増える）。

`CaptureSpan` 格納:

* S-1（採用）: 各 `CaptureChunk` 内の固定容量配列（per-chunk 有界 Span
  列）。Block 途中の複数境界を 1 Chunk の複数 Span で表現し、Span 容量
  超過は Fault。
* S-2: Span だけを別 SPSC で Writer へ送る side-channel。

## Decision Outcome

Chosen option: "Q-1 + G-1 + F-1 + O-1 + S-1"。

* Q-1: 合意済み候補であり、payload 返却型 `try_push` が「Chunk を RT で
  Drop しない」契約の実装要件と一致する。独自 SPSC（Q-2）は同等機能の
  重複実装になり検証コストが増える。`std::sync::mpsc`（Q-3）は
  Producer/Consumer 単一所有・容量有界・非 Block try_push の契約を型で
  保証できないため RT 経路に使わない。
* G-1: 「無期限 Loop 全周を事前展開しない」合意を正直に表現できる唯一の
  形。G-2 は有限セッションでは動くが、停止時刻未定の録音を表現できず、
  Lookahead 補充と補充不能時安全停止の故障 Gate を検証できない。G-3 は
  RT 上の再割当リスクと容量無界を招く。
* F-1: Full のデータ Queue 自体が通知経路だと、通知が必要なまさにその
  状況で Fault を届けられない。Atomic Latch は RT で待たず・割当てず・
  順序不要に fail-stop を伝播できる。
* O-1: `try_push` が返した `Box` 由来 Chunk を RT で Drop すると dealloc
  が発生し契約違反。`Option<CaptureChunk>` の固定スロットへの move は
  割当ても dealloc も起こさない。Fail-stop 後は push を再試行しない
  （Fault は単一方向ラッチ）ので、retained は最大 1 個で足り、
  「所有権を失わない」が型と総数照合で検証できる。O-3 の
  `retire_channel` は正当な上流プリミティブだが、Fail-stop で retained
  が 1 個に限定される本 Gate では、もう 1 本の Queue とその満杯時の
  payload 返却ポリシーを持ち込む必要がない（YAGNI。汎用的な retire
  需要は VS-C 以降で再評価）。
* S-1: Span を Chunk と同じ所有権単位で運ぶと、Queue/所有権/故障経路が
  増えず、Writer が受け取る 1 メッセージで Block の全保存区間が完結
  する。S-2 は欠落時の Span/PCM 照合を別途復元する必要があり、故障
  Gate を複雑化する。

### Consequences

* Good: `ruviform-rt` 追加で SPSC の正当性・wait-free 性を上流の契約に
  委譲できる。既定ブランチ追従 + lockfile の運用（VS-A ADR）をそのまま
  踏襲する。
* Good: Fault Latch が単一方向（set のみ）なので、Fail-stop 状態の遷移が
  決定的で、RT 側の後処理が「有界 no-op」に簡約される。
* Good: Lookahead Window により、補充不能・Span 超過・Queue 系とは独立
  した第3の fail-stop 経路（schedule underrun）を試験できる。
* Bad: `ruviform-rt` は `arc-swap`/`rtrb` を依存に持ち、依存グラフが
  増える（lockfile で固定・追跡）。
* Bad: rolling Lookahead は非 RT スケジューラの補充タイミングを呼び出し
  側（PoC ドライバ/将来の Coordinator）の責務にする。補充を忘れると
  録音が Fault 停止する = 設計どおりの安全側失敗。
* Bad: retained スロット容量 1 は「最初の Fault で停止する」設計に依存
  する。複数 Fault 許容設計へ変える場合はスロット数と回収照合の見直し
  が必要（現行は Fail-stop で 1 個で充足）。
* Bad: per-chunk Span 配列は、1 Block が極端に多くの Gate 境界を跨ぐ場合
  に容量超過 Fault となる。容量は注入可能で、PoC 測定後に実数値を確定
  する（Deferred）。

### Confirmation

* `mise run check`/`lint`/`test` が緑で、`Cargo.lock` に `ruviform-rt`
  の git resolved revision が記録される。
* Queue Full/Free 枯渇/Span 容量超過/Schedule 補充不能/Epoch 断絶の各
  故障で、RT 側が非 Blocking に Fault Latch を立てて Fail-stop し、
  counting allocator で alloc/dealloc 0 件を実測できる。
* 失敗 `try_push` 返却 Chunk が退役時総数照合に含まれる。
* 有限 Lookahead で 3 周 Loop を完走できる（全周事前展開に依存しない）。

## Implementation Plan

* Affected paths:
  * `Cargo.toml`（workspace 依存に `ruviform-rt` git 追加）、`Cargo.lock`
    （再生成・追跡）。
  * `crates/synvelis-recording/` — `plan`/`clock`/`gate`/`capture`/
    `handoff`/`engine`/`mock` 等の新モジュール。
  * `crates/recording-poc/` — VS-B 決定的シナリオと試験。
* Patterns to follow:
  * `try_push`/`try_pop` のみ使用し、Full 時は返却 payload を固定
    スロットへ退避。Queue メソッド以外で producer/consumer を共有しない
    （`!Sync` の単一所有を維持）。
  * Gate 評価・Span 書込み・PCM copy は全て事前確保メモリ上の有界操作。
  * Fault Latch は `set` のみ（clear は退役時の非 RT 再初期化のみ）。
* Patterns to avoid:
  * RT callback 内の `Box` 生成/破棄、`Vec` push、`Option` 以外の可変長
    操作、lock、println/ファイル I/O。
  * `std::sync::mpsc`・`pop_latest`（返却 Chunk の drop 方針が合わない）。
  * データ Queue に Fault を載せる設計、Full Queue を前提にした回収。
  * Loop 全周分の Gate 事前展開。
* Dependencies or configuration:
  * `ruviform-rt = { git = "https://github.com/scarecrow173/ruviform.git" }`
    を workspace 依存に追加（`ruviform-media-core` と同一戦略）。

## Verification

* [ ] `mise run fmt-check`/`check`/`lint`/`test`/`build` が全緑。
* [ ] Fault 各経路で counting allocator が RT callback 中の alloc/dealloc
  を 0 件と計測する。
* [ ] Fail-stop 後の callback が有界 no-op（SessionFrame のみ進行）である
  ことを試験する。
* [ ] 停止・退役時に pool 総数（filled 滞留 + free 滞留 + writer 保持 +
  engine retained）が一致する。

## Pros and Cons of the Options

### Option Q-1 (ruviform-rt::channel)

* Good: 合意済み・検証済み・payload 返却で所有権制御ができる。
* Bad: 上流 API 変更を lockfile 更新で取り込む運用負荷が残る。

### Option G-1 (rolling lookahead window)

* Good: 無期限 Loop と補充不能の安全停止を正直に表現。有限シナリオでは
  事前に足りる分だけ補充すれば全展開と同等に動く。
* Bad: 補充責務が呼び出し側に移る（設計どおりの安全側失敗）。

### Option F-1 (atomic fault latch)

* Good: Full Queue と独立・非 Block・順序不要・単一方向で決定的。
* Bad: Fault 詳細（最初の原因）の保持は bit 数で限定される（診断情報は
  非 RT が Chunk header/Validity から補う）。

### Option O-1 (fixed retained slot)

* Good: RT 上は move のみ・容量 1 で充足・総数照合が単純。
* Bad: 「最初の Fault で停止」設計に結合（fail-stop 合意と一致）。

### Option S-1 (per-chunk span array)

* Good: 所有権・故障・照合の単位が Chunk に一元化される。
* Bad: Span 数は Block あたり有界で、極端な境界密度は容量超過 Fault
  （安全側失敗として受容）。

## More Information

* [Synvelis #14](https://github.com/scarecrow173/synvelis/issues/14) —
  実装・故障 Gate と終了条件。
* [Synvelis #12 §17-18](https://github.com/scarecrow173/synvelis/issues/12) —
  3 層分離・bounded handoff・Fail-stop・VS-01 仕様の合意。
* [Ruviform #187](https://github.com/scarecrow173/ruviform/issues/187) /
  [#190](https://github.com/scarecrow173/ruviform/issues/190) —
  実 Input/Span Scheduler（本 Gate の前提外）。
* `docs/adr/vs-a-workspace-ruviform-git-poc-stable-id.md` —
  Ruviform git 依存 + lockfile 運用の継承元。
