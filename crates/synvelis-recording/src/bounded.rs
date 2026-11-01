//! 固定容量・非割当の有界コンテナ。
//!
//! `unsafe_code` が forbid のため `MaybeUninit` 固定配列は使えず、
//! 代わりに構築時に `Vec::with_capacity(capacity)` で確保した領域を
//! 再利用する。`push` は容量一杯で payload を返して失敗し、`clear` /
//! `compact` は長さだけを操作して再割当てしない。消費は `head` カーソル
//! による O(1) 前進で行い、RT パス上の memmove を避ける。

/// `push` が容量一杯で失敗したことを示す。payload を所有権付きで返す。
#[derive(Debug, PartialEq, Eq)]
pub struct BoundedFull<T>(pub T);

/// 容量固定の有界ベクタ。割当ては構築時の一度だけ。
#[derive(Debug)]
pub struct BoundedVec<T> {
    items: Vec<T>,
    head: usize,
}

impl<T> BoundedVec<T> {
    /// `capacity` 個の要素を保持できる領域を確保する（この呼出しでのみ割当て）。
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            head: 0,
        }
    }

    /// 論理要素数（消費済みを除く）。
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len() - self.head
    }

    /// 論理要素数が 0 か。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 確保容量。
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.items.capacity()
    }

    /// `head` カーソルが前進した分。
    #[must_use]
    pub fn consumed(&self) -> usize {
        self.head
    }

    /// 末尾に追加する。残り容量がなければ payload を `BoundedFull` に包んで返す。
    ///
    /// # Errors
    /// [`BoundedFull`]：残り容量なし。
    pub fn push(&mut self, value: T) -> Result<(), BoundedFull<T>> {
        if self.items.len() >= self.items.capacity() {
            return Err(BoundedFull(value));
        }
        self.items.push(value);
        Ok(())
    }

    /// 先頭から `n` 要素を消費済みにする（`head` を進めるだけ。所有権移動なし）。
    ///
    /// `n > len()` の場合は論理要素数までに留める。
    pub fn advance(&mut self, n: usize) {
        self.head = (self.head + n).min(self.items.len());
    }

    /// 論理先頭要素。
    #[must_use]
    pub fn front(&self) -> Option<&T> {
        self.items.get(self.head)
    }

    /// 論理 `index` 番目（消費済みを除いた順序）。
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&T> {
        self.items.get(self.head + index)
    }

    /// 消費済みを除く残り要素のスライス。
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        &self.items[self.head..]
    }

    /// 未消費要素を `head` 側へ詰め、`head` を 0 に戻す。
    ///
    /// 非 RT の schedule 再充填で使う。`Vec::drain` の memmove は
    /// 有界（≤ capacity）だが RT パスでは呼ばない。
    pub fn compact(&mut self) {
        if self.head > 0 {
            self.items.drain(..self.head);
            self.head = 0;
        }
    }

    /// 全要素を消去する（容量は保持。再割当てなし）。
    pub fn clear(&mut self) {
        self.items.clear();
        self.head = 0;
    }
}

impl<T> IntoIterator for BoundedVec<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        // 所有権で消費する際は消費済みスロットも含めて drain する。
        self.items.into_iter()
    }
}

/// `BoundedVec` の消費は `head` カーソルで行い、要素の部分取り出しは
/// `into_iter`/`clear`/`compact` を通す設計のため `pop`/`remove` は
/// 提供しない（誤った O(n) 消費や RT 上の move を型で防ぐ）。
#[cfg(test)]
mod tests {
    use super::{BoundedFull, BoundedVec};

    #[test]
    fn push_advances_and_caps() {
        let mut v = BoundedVec::with_capacity(2);
        v.push(1).unwrap();
        v.push(2).unwrap();
        assert_eq!(v.push(3), Err(BoundedFull(3)));
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn advance_consumes_without_move() {
        let mut v = BoundedVec::with_capacity(4);
        v.push(10).unwrap();
        v.push(20).unwrap();
        v.push(30).unwrap();
        v.advance(2);
        assert_eq!(v.front(), Some(&30));
        assert_eq!(v.as_slice(), &[30]);
        v.compact();
        assert_eq!(v.len(), 1);
        assert_eq!(v.consumed(), 0);
    }
}
