//! IEEE CRC32（polynomial 0xEDB8_8320、reflected）。in-tree 実装で
//! 新規外部依存を持ち込まない。staging journal の record frame と
//! PCM byte の整合検証に使う（暗号学的強度は主張しない）。

use serde::{Deserialize, Serialize};

const fn build_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0usize;
    while i < 256 {
        #[allow(clippy::cast_possible_truncation)]
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        table[i] = c;
        i += 1;
    }
    table
}

static TABLE: [u32; 256] = build_table();

/// `bytes` の CRC32（初期 0・最終 XOR 込みの標準値）。
#[must_use]
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in bytes {
        c = TABLE[((c ^ u32::from(b)) & 0xFF) as usize] ^ (c >> 8);
    }
    !c
}

/// 逐次 CRC32。`update` を繰り返して `finish` で確定する。
#[derive(Clone, Debug)]
pub struct Crc32 {
    state: u32,
}

impl Crc32 {
    /// 初期状態。
    #[must_use]
    pub fn new() -> Self {
        Self { state: !0 }
    }

    /// `bytes` を畳み込む。
    pub fn update(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.state = TABLE[((self.state ^ u32::from(b)) & 0xFF) as usize] ^ (self.state >> 8);
        }
    }

    /// 確定値（標準 CRC32）。
    #[must_use]
    pub fn finish(&self) -> u32 {
        !self.state
    }
}

impl Default for Crc32 {
    fn default() -> Self {
        Self::new()
    }
}

/// 全 stream digest の fold: `acc = acc*K ^ v`（決定的・衝突耐性は
/// 主張しない。chunk CRC を順に畳んで media 全体の整合を表す）。
pub const DIGEST_FOLD_K: u64 = 0x9E37_79B9_7F4A_7C15;

/// `digest` に `v` を 1 回畳む。
#[must_use]
pub const fn digest_fold(digest: u64, v: u64) -> u64 {
    digest.wrapping_mul(DIGEST_FOLD_K) ^ v
}

/// CRC32 fold digest の **integrity 用途**を型で分離する newtype。
///
/// journal record 整合・commit 冪等 hash・`finalized.json` の
/// `digest` hex に使う非暗号学的な integrity 値であり、#86 の
/// 検証済み SHA-256 content identity（Asset/Blob）ではない。
/// 用途混同を防ぐため生の `u64` とは別型にする。journal wire 上の
/// `digest_fold` field は互換のため `u64` のまま（境界で本型へ
/// 変換する）。
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct IntegrityDigest(pub u64);

impl IntegrityDigest {
    /// fold の初期値（畳み込み前）。
    pub const ZERO: Self = Self(0);

    /// `v` を 1 回畳んだ次の値。
    #[must_use]
    pub const fn fold(self, v: u64) -> Self {
        Self(digest_fold(self.0, v))
    }

    /// 固定幅 lowercase hex（`finalized.json` の `digest` 表現と同一）。
    #[must_use]
    pub fn to_hex(self) -> String {
        format!("{:016x}", self.0)
    }
}

impl From<u64> for IntegrityDigest {
    fn from(v: u64) -> Self {
        Self(v)
    }
}

impl From<IntegrityDigest> for u64 {
    fn from(d: IntegrityDigest) -> Self {
        d.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_vectors() {
        // 標準ベクタ: "123456789" -> 0xCBF43926, "" -> 0
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(
            crc32(b"The quick brown fox jumps over the lazy dog"),
            0x414F_A339
        );
    }

    #[test]
    fn incremental_matches_oneshot() {
        let mut c = Crc32::new();
        c.update(b"1234");
        c.update(b"56789");
        assert_eq!(c.finish(), crc32(b"123456789"));
    }

    #[test]
    fn fold_is_deterministic() {
        let d = digest_fold(0, 1);
        assert_eq!(d, digest_fold(0, 1));
        assert_ne!(digest_fold(d, 2), d);
    }
}
