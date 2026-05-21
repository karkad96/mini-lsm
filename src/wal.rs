use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::{Record, RecordError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    Always,
    BatchOnly,
    Never,
}

pub struct Wal {
    file: BufWriter<File>,
    path: PathBuf,
    size: u64,
    sync_mode: SyncMode,
}

impl Wal {
    pub fn new(path: &str) -> std::io::Result<Self> {
        Self::with_sync_mode(path, SyncMode::Always)
    }

    pub fn with_sync_mode(path: &str, sync_mode: SyncMode) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        let size = file.metadata()?.len();
        Ok(Wal {
            file: BufWriter::new(file),
            path: PathBuf::from(path),
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

        self.size += data.len() as u64;
        self.file.write_all(&data)?;
        self.file.flush()?;

        if self.sync_mode == SyncMode::Always {
            self.file.get_ref().sync_all()?;
        }
        Ok(())
    }

    pub fn append_batch(&mut self, records: &[Record]) -> std::io::Result<()> {
        for record in records {
            let data = record.encode();
            self.size += data.len() as u64;
            self.file.write_all(&data)?;
        }

        self.file.flush()?;

        if self.sync_mode != SyncMode::Never {
            self.file.get_ref().sync_all()?;
        }
        Ok(())
    }

    pub fn iter(path: &str) -> Result<WalIter, RecordError> {
        let file = File::open(path)?;

        Ok(WalIter { reader: std::io::BufReader::new(file) })
    }

    pub fn recover(path: &str) -> Result<Vec<Record>, RecordError> {
        Self::iter(path)?.collect()
    }

    pub fn delete(self) -> std::io::Result<()> {
        drop(self.file);

        std::fs::remove_file(&self.path)
    }
}

pub struct WalIter {
    reader: std::io::BufReader<File>,
}

impl Iterator for WalIter {
    type Item = Result<Record, RecordError>;

    fn next(&mut self) -> Option<Self::Item> {
        match Record::decode(&mut self.reader) {
            Ok(Some(record)) => Some(Ok(record)),
            Ok(None) => None,
            Err(RecordError::Truncated) => None,
            Err(e) => Some(Err(e)),
        }
    }
}
