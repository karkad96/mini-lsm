use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use crate::{Record, RecordError};

pub struct Wal {
    file: BufWriter<File>,
    path: PathBuf,
}

impl Wal {
    pub fn new(path: &str) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(Wal {
            file: BufWriter::new(file),
            path: PathBuf::from(path),
        })
    }

    pub fn append(&mut self, record: &Record) -> std::io::Result<()> {
        let data = record.encode();
        self.file.write_all(&data)?;
        self.file.flush()?;
        self.file.get_ref().sync_all()?;
        Ok(())
    }

    pub fn append_batch(&mut self, records: &[Record]) -> std::io::Result<()> {
        for record in records {
            let data = record.encode();
            self.file.write_all(&data)?;
        }
        self.file.flush()?;
        self.file.get_ref().sync_all()?;
        Ok(())
    }

    pub fn recover(path: &str) -> Result<Vec<Record>, RecordError> {
        let file = File::open(path)?;
        let mut records = Vec::new();
        let mut reader = std::io::BufReader::new(file);
        loop {
            match Record::decode(&mut reader) {
                Ok(Some(record)) => records.push(record),
                Ok(None) => break,
                Err(RecordError::Truncated) => break,
                Err(e) => return Err(e),
            }
        }
        Ok(records)
    }

    pub fn delete(self) -> std::io::Result<()> {
        drop(self.file);
        std::fs::remove_file(&self.path)
    }

}