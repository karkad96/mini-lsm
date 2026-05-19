use std::io::{self, Read};

use crc32fast::Hasher;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Op {
    Put,
    Delete,
}

impl Op {
    fn to_byte(&self) -> u8 {
        match self {
            Op::Put => 0,
            Op::Delete => 1,
        }
    }

    fn from_byte(b: u8) -> Result<Op, RecordError> {
        match b {
            0 => Ok(Op::Put),
            1 => Ok(Op::Delete),
            other => Err(RecordError::InvalidOp(other)),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Record {
    pub op: Op,
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum RecordError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("invalid op byte: {0}")]
    InvalidOp(u8),
    #[error("crc mismatch: expected {expected:#x}, got {actual:#x}")]
    CrcMismatch { expected: u32, actual: u32 },
    #[error("unexpected end of stream (truncated record)")]
    Truncated,
}

impl Record {
    pub fn encode(&self) -> Vec<u8> {
        let key_len = self.key.len() as u32;
        let value_len = self.value.len() as u32;

        let mut buf = Vec::with_capacity(1 + 4 + self.key.len() + 4 + self.value.len() + 4);
        buf.push(self.op.to_byte());
        buf.extend_from_slice(&key_len.to_le_bytes());
        buf.extend_from_slice(&self.key);
        buf.extend_from_slice(&value_len.to_le_bytes());
        buf.extend_from_slice(&self.value);

        let mut hasher = Hasher::new();
        hasher.update(&buf);
        let crc = hasher.finalize();
        buf.extend_from_slice(&crc.to_le_bytes());

        buf
    }

    pub fn decode<R: Read>(reader: &mut R) -> Result<Option<Record>, RecordError> {
        let mut op_buf = [0u8; 1];
        match reader.read(&mut op_buf)? {
            0 => return Ok(None),
            1 => {}
            _ => unreachable!(),
        }

        let op = Op::from_byte(op_buf[0])?;
        let key_len = read_u32(reader)? as usize;
        let key = read_exact_vec(reader, key_len)?;
        let value_len = read_u32(reader)? as usize;
        let value = read_exact_vec(reader, value_len)?;
        let stored_crc = read_u32(reader)?;

        let mut hasher = Hasher::new();
        hasher.update(&op_buf);
        hasher.update(&(key_len as u32).to_le_bytes());
        hasher.update(&key);
        hasher.update(&(value_len as u32).to_le_bytes());
        hasher.update(&value);
        let actual_crc = hasher.finalize();

        if stored_crc != actual_crc {
            return Err(RecordError::CrcMismatch {
                expected: stored_crc,
                actual: actual_crc,
            });
        }

        Ok(Some(Record { op, key, value }))
    }
}

fn read_u32<R: Read>(reader: &mut R) -> Result<u32, RecordError> {
    let mut buf = [0u8; 4];
    read_exact_or_truncated(reader, &mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_exact_vec<R: Read>(reader: &mut R, n: usize) -> Result<Vec<u8>, RecordError> {
    let mut buf = vec![0u8; n];
    read_exact_or_truncated(reader, &mut buf)?;
    Ok(buf)
}

fn read_exact_or_truncated<R: Read>(reader: &mut R, buf: &mut [u8]) -> Result<(), RecordError> {
    match reader.read_exact(buf) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Err(RecordError::Truncated),
        Err(e) => Err(RecordError::Io(e)),
    }
}
