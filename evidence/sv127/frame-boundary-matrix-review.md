# TIM-03 frame/boundary matrix gate-fix review evidence

Date: 2026-11-27
Branch: `feat/127-frame-boundary-matrix`
PR: `#688`

## Reviewer / validator split

| role | actor | evidence |
| --- | --- | --- |
| implementer | dispatched worker | gate-fix diff in `crates/synvelis-player/tests/tim03_request_plan.rs`, `tim03_process_boundary.rs`, `tim03_loop_boundary.rs` |
| reviewer | independent read-only review subagent (`Focused gate review`) | qualitative blocker review below; no write access and no test execution |
| validator | canonical mise lanes executed by the implementer after the fix | `frame-boundary-matrix/gate-fix4-canonical.log` and `gate-fix4-results.tsv` |

This is review-protocol-equivalent evidence for this slice: review and validation are separate activities, and the reviewer did not edit the implementation.

## Reviewer result

Reviewer verdict: **blockers: 0**.

Reviewer findings:

1. `tim03_req_simultaneous_loop_seek_tempo_param` covers valid W×B cells with fresh instances, exact split lengths, folded post-seek position 7, exactly-once param delivery, and process-start-relative offsets.
2. Emitted rows expose boundary kinds, combined-case inputs, exact process lengths, param positions, source-sequence ordering, and same-param last-wins detail.
3. No `#[allow(clippy::type_complexity)]` remains; the tuple return was decomposed behind the named `PlayingSurface` type alias.

Reviewer minor caveats, with disposition:

| caveat | disposition |
| --- | --- |
| combined-row tie-rule text is static consumer evidence inferred from outcomes, not a direct scheduler-internal probe | Accepted for this slice. The upstream tie rule is consumer evidence; exact observable effects are single coalesced boundary, post-seek folded position 7, and exactly-once param delivery. |
| tempo-map installation is exercised, but broader tempo conversion behavior is not asserted | Accepted. Full dynamic tempo edit matrix is explicitly out of scope. |
| same-param last-wins is inferred from delivery order rather than final plugin-state readback | Accepted. The recording-plugin event order is the contract-level observable used by the fake plugin oracle. |
| `absolute_position` under loops is folded transport position, not engine-frame absolute | Accepted and evidence wording treats frame offset as process-start-relative; no media-frame absolute claim is made. |

## Blocker disposition

| gate blocker | disposition |
| --- | --- |
| simultaneous ordering was only a 256-frame representative | Fixed. `tim03_req_simultaneous_loop_seek_tempo_param` now runs valid W=100/128/256 × B=0/1/100/last cells with exact process lengths, post-seek position 7, tempo-map plan install with explicit PPQN 480, and exactly-once same-position param frame-offset/source-order evidence. W=100/B=100 remains invalid/out-of-window and is not clamped. |
| JSONL rows lacked discriminating fields | Fixed. Rows now carry boundary kinds, combined-case flags, simultaneous command order, tie-rule consumer evidence, exact process lengths, param positions, process-start-relative frame offsets, source-sequence order, observed delivery order, and same-param last-wins detail. |
| review-protocol-equivalent evidence missing | Fixed by this document, canonical validation logs, PR body update, and PR comment. |
| `#[allow(clippy::type_complexity)]` lint hiding | Fixed. Both allows were removed; the return type uses the explicit `PlayingSurface` alias and `mise run lint synvelis-player` passes without suppression. |

Remaining review blockers: **0**.

## Follow-up static review disposition

A second read-only static reviewer inspected an intermediate diff and raised four concerns. Disposition after the final fix:

| concern | final disposition |
| --- | --- |
| exact loop-multiple seek target / potential upstream multi-wrap defect | The final matrix uses `Seek(307)` over loop `[0,300)` and observes post-collapse position 7, so it is not an exact multiple and does not collapse to the natural-wrap baseline 0. The pinned upstream scheduler completes the matrix in the canonical lane; no upstream defect was reproduced in this slice. Legacy representative exact-multiple tests remain regression coverage, not the matrix oracle. |
| ordering and tempo evidence were static labels | The final oracle asserts observable exact lengths, folded post-seek position 7, exactly-once ordered param delivery, and uses plan-owned `with_musical_transport` with explicit PPQN 480 and asserts call-start bpm_milli=120000/ppqn=480 rather than tempo-map-only negative musical state. Tie-rule text remains consumer semantics, but the assertions are behavioral. |
| source-sequence / last-wins not in combined matrix | Fixed. The combined plan now carries three same-position events with source sequences `[3,1,2]`, duplicate `ParamId(7)`, expected delivery order `[(9,0.2),(7,0.25),(7,0.75)]`, and same-param final value 0.75. |
| WrapSim masks upstream modulo behavior | Not used as the combined wired oracle. The wired matrix executes the real scheduler/node path. No Synvelis workaround or upstream semantic change was added. |

Reviewer static notes about the intermediate implementation are therefore either fixed or explicitly bounded. Remaining blockers after final validation: **0**.
