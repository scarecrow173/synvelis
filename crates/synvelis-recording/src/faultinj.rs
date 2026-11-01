//! 決定的 Fault Injection。staging/writer の I/O 経路を
//! [`StorageWriter`] trait 境界で包み、Partial/Short Write・Disk
//! Full・Sync 失敗・Journal torn tail をスクリプト化して注入する。
//!
//! 公開モジュール（cfg(test) に閉じない）: `recording-poc` の crash
//! モードや将来のジョブ層検証がそのまま利用できるようにするため。
//! 注入は全て決定的で、時刻・乱数・実 OS fault に依存しない。

use std::{
    fs::File,
    io::{self, Write},
    path::Path,
};

/// staging が要求する最小の永続化書込み口。`write_all` は常に
/// 「全部書くかエラーか」（短 write を成功扱いしない）。
pub trait StorageWriter {
    /// `buf` を末尾へ全て書く。
    ///
    /// # Errors
    /// 途中失敗を含む任意の `io::Error`。
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()>;
    /// これまでの内容を durable に同期する。
    ///
    /// # Errors
    /// sync 失敗。write 成功とは別種に扱う。
    fn sync_data(&mut self) -> io::Result<()>;
}

impl StorageWriter for File {
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        Write::write_all(self, buf)
    }
    fn sync_data(&mut self) -> io::Result<()> {
        File::sync_data(self)
    }
}

/// 注入する故障の種類。
#[derive(Clone, Debug)]
pub enum Inject {
    /// `call` 回目（0 起点）の `write_all` で全バイトを書かずエラー
    /// （partial write / disk full 相当。`StorageFull` kind を使う）。
    FailWrite {
        /// 失敗させる write 呼出し番号。
        call: u64,
    },
    /// `call` 回目の `write_all` で先頭 `bytes` だけを実際に書き、
    /// その後エラーを返す（short write の途中失敗を再現）。
    ShortWrite {
        /// 失敗させる write 呼出し番号。
        call: u64,
        /// 実際に書く byte 数。
        bytes: usize,
    },
    /// `call` 回目の `sync_data` を失敗させる。
    FailSync {
        /// 失敗させる sync 呼出し番号。
        call: u64,
    },
    /// 累積書込み byte が `limit` を超える以後の `write_all` を、
    /// limit までの分だけ書いてエラーにする（torn tail 用）。
    ByteLimit {
        /// 許容する累積 byte 数。
        limit: u64,
    },
}

/// [`File`] を包む決定的 fault injector。
pub struct FaultWriter {
    inner: File,
    injects: Vec<Inject>,
    write_calls: u64,
    sync_calls: u64,
    written: u64,
}

impl FaultWriter {
    /// `inner` を包む。注入は `inject` で追加する。
    /// `written`（`ByteLimit` の判定基準）は既存 file 長から開始し、
    /// 非空 file を wrap しても注入累計がずれないようにする。
    #[must_use]
    pub fn new(inner: File) -> Self {
        let written = inner.metadata().map_or(0, |m| m.len());
        Self {
            inner,
            injects: Vec::new(),
            write_calls: 0,
            sync_calls: 0,
            written,
        }
    }

    /// 注入を追加する（同種複数可・評価は登録順）。
    pub fn inject(&mut self, inj: Inject) {
        self.injects.push(inj);
    }

    /// 内側の File への参照（読み戻し・sync 用）。
    #[must_use]
    pub fn inner(&self) -> &File {
        &self.inner
    }
}

impl StorageWriter for FaultWriter {
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        let call = self.write_calls;
        self.write_calls = self.write_calls.saturating_add(1);
        for inj in &self.injects {
            match *inj {
                Inject::FailWrite { call: c } if c == call => {
                    return Err(io::Error::new(
                        io::ErrorKind::StorageFull,
                        "injected write failure",
                    ));
                }
                Inject::ShortWrite { call: c, bytes } if c == call => {
                    let n = bytes.min(buf.len());
                    if n > 0 {
                        Write::write_all(&mut self.inner, &buf[..n])?;
                        self.written = self.written.saturating_add(n as u64);
                    }
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "injected short write",
                    ));
                }
                Inject::ByteLimit { limit } => {
                    if self.written >= limit {
                        return Err(io::Error::new(
                            io::ErrorKind::StorageFull,
                            "injected byte limit",
                        ));
                    }
                    if self.written.saturating_add(buf.len() as u64) > limit {
                        let n = usize::try_from(limit - self.written).unwrap_or(0);
                        Write::write_all(&mut self.inner, &buf[..n])?;
                        self.written = self.written.saturating_add(n as u64);
                        return Err(io::Error::new(
                            io::ErrorKind::StorageFull,
                            "injected byte limit (torn)",
                        ));
                    }
                }
                _ => {}
            }
        }
        Write::write_all(&mut self.inner, buf)?;
        self.written = self.written.saturating_add(buf.len() as u64);
        Ok(())
    }

    fn sync_data(&mut self) -> io::Result<()> {
        let call = self.sync_calls;
        self.sync_calls = self.sync_calls.saturating_add(1);
        for inj in &self.injects {
            if let Inject::FailSync { call: c } = *inj
                && c == call
            {
                return Err(io::Error::other("injected sync failure"));
            }
        }
        self.inner.sync_data()
    }
}

/// ファイル `path` の `offset` バイトを `mask` で XOR する（journal
/// checksum 破損の注入。テスト側 util）。
///
/// # Errors
/// open/read/write の失敗。
pub fn corrupt_byte(path: &Path, offset: u64, mask: u8) -> io::Result<()> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?;
    f.seek(SeekFrom::Start(offset))?;
    let mut b = [0u8; 1];
    f.read_exact(&mut b)?;
    b[0] ^= mask;
    f.seek(SeekFrom::Start(offset))?;
    Write::write_all(&mut f, &b)?;
    f.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn tmp_file(name: &str) -> (std::path::PathBuf, File) {
        let dir =
            std::env::temp_dir().join(format!("vsc-faultinj-{}-{}", std::process::id(), name));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        let f = File::create(&p).unwrap();
        (p, f)
    }

    #[test]
    fn fail_write_injects_storage_full() {
        let (_path, file) = tmp_file("fw.bin");
        let mut w = FaultWriter::new(file);
        w.inject(Inject::FailWrite { call: 1 });
        w.write_all(b"aa").unwrap();
        let err = w.write_all(b"bb").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::StorageFull);
    }

    #[test]
    fn short_write_lands_partial_then_fails() {
        let (path, file) = tmp_file("sw.bin");
        let mut w = FaultWriter::new(file);
        w.inject(Inject::ShortWrite { call: 0, bytes: 3 });
        let err = w.write_all(b"abcdef").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::WriteZero);
        drop(w);
        let mut s = String::new();
        File::open(&path).unwrap().read_to_string(&mut s).unwrap();
        assert_eq!(s, "abc");
    }

    #[test]
    fn byte_limit_tears_tail() {
        let (path, file) = tmp_file("bl.bin");
        let mut w = FaultWriter::new(file);
        w.inject(Inject::ByteLimit { limit: 5 });
        w.write_all(b"aaa").unwrap();
        assert!(w.write_all(b"bbbb").is_err()); // 2 bytes land, then err
        assert!(w.write_all(b"c").is_err());
        drop(w);
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 5);
    }

    #[test]
    fn fail_sync_injected() {
        let (_path, file) = tmp_file("fs.bin");
        let mut w = FaultWriter::new(file);
        w.inject(Inject::FailSync { call: 0 });
        w.write_all(b"aa").unwrap();
        assert!(w.sync_data().is_err());
        assert!(w.sync_data().is_ok());
    }
}
