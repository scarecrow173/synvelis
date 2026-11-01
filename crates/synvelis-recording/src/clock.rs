//! RT コールバックに届く入力ブロック記述。
//!
//! real backend では `ClockEpochId` と device input frame 系列が
//! コールバックごとに提供される。`first_input_frame` は 0 起点とは
//! 限らないため、engine は最初のブロックで baseline を採種し、
//! 以降の Epoch 変化・frame 不連続を Fault として検出する
//! （不連続後に連続を捏造しない）。

use synvelis_project::ClockEpochId;

/// 1 回の入力コールバックに相当するブロック記述。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct InputBlock {
    /// このブロックを生成した Clock Epoch。
    pub clock_epoch: ClockEpochId,
    /// ブロック先頭の device input frame 番号。
    pub first_input_frame: u64,
    /// ブロックの frame 数（`> 0`）。
    pub frames: u32,
}
