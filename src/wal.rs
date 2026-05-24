use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write, BufReader};
use std::path::{Path, PathBuf};

use crate::{Record, RecordError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    FsyncAlways,
    FsyncOnBatch,
    FsyncNever,
}

pub struct Wal {
    file: File,
    path: PathBuf,
    size: u64,
    sync_mode: SyncMode,
}

impl Wal {
    pub fn new<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        Self::with_sync_mode(path, SyncMode::FsyncAlways)
    }

    pub fn with_sync_mode<P: AsRef<Path>>(path: P, sync_mode: SyncMode) -> std::io::Result<Self> {
        let path = path.as_ref();
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        let size = file.metadata()?.len();
        Ok(Wal {
            file,
            path: path.to_path_buf(),
            size,
            sync_mode,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn append(&mut self, record: &Record) -> std::io::Result<()> {
        let data = record.encode();

        self.file.write_all(&data)?;
        self.size += data.len() as u64;

        if self.sync_mode == SyncMode::FsyncAlways {
            self.file.sync_data()?;
        }
        Ok(())
    }

    pub fn append_batch(&mut self, records: &[Record]) -> std::io::Result<()> {
        let mut buf = Vec::new();
        for record in records {
            buf.extend(record.encode());
        }

        self.file.write_all(&buf)?;
        self.size += buf.len() as u64;

        if self.sync_mode != SyncMode::FsyncNever {
            self.file.sync_data()?;
        }
        Ok(())
    }

    pub fn recover<P: AsRef<Path>>(path: P) -> Result<Vec<Record>, RecordError> {
        let mut out = Vec::new();
        for item in WalIter::open(path)? {
            match item {
                Ok(record) => out.push(record),
                Err(RecordError::Truncated) => {
                    // log here: tracing::warn!("WAL truncated, stopping replay");
                    break;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }

    pub fn delete(self) -> std::io::Result<()> {
        std::fs::remove_file(&self.path)
    }
}

pub struct WalIter {
    reader: std::io::BufReader<File>,
}

impl WalIter {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, RecordError> {
        let file = File::open(path)?;
        Ok(WalIter { reader: BufReader::new(file) })
    }
}

impl Iterator for WalIter {
    type Item = Result<Record, RecordError>;

    fn next(&mut self) -> Option<Self::Item> {
        match Record::decode(&mut self.reader) {
            Ok(Some(record)) => Some(Ok(record)),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }
    }
}
