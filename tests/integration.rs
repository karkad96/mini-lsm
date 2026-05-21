use mini_lsm::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn put_round_trip() {
        let r = Record {
            op: Op::Put,
            key: b"hello".to_vec(),
            value: b"world".to_vec(),
        };
        let bytes = r.encode();
        let mut cursor = Cursor::new(bytes);
        let decoded = Record::decode(&mut cursor).unwrap().unwrap();
        assert_eq!(decoded, r);
    }

    #[test]
    fn delete_round_trip() {
        let r = Record {
            op: Op::Delete,
            key: b"gone".to_vec(),
            value: vec![],
        };
        let bytes = r.encode();
        let decoded = Record::decode(&mut Cursor::new(bytes)).unwrap().unwrap();
        assert_eq!(decoded, r);
    }

    #[test]
    fn multiple_records_in_stream() {
        let r1 = Record {
            op: Op::Put,
            key: b"a".to_vec(),
            value: b"1".to_vec(),
        };
        let r2 = Record {
            op: Op::Put,
            key: b"b".to_vec(),
            value: b"2".to_vec(),
        };
        let mut bytes = r1.encode();
        bytes.extend(r2.encode());

        let mut cursor = Cursor::new(bytes);
        assert_eq!(Record::decode(&mut cursor).unwrap().unwrap(), r1);
        assert_eq!(Record::decode(&mut cursor).unwrap().unwrap(), r2);
        assert!(Record::decode(&mut cursor).unwrap().is_none()); // clean EOF
    }

    #[test]
    fn corrupted_crc_is_detected() {
        let r = Record {
            op: Op::Put,
            key: b"k".to_vec(),
            value: b"v".to_vec(),
        };
        let mut bytes = r.encode();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;

        let result = Record::decode(&mut Cursor::new(bytes));
        assert!(matches!(result, Err(RecordError::CrcMismatch { .. })));
    }

    #[test]
    fn truncated_record_is_detected() {
        let r = Record {
            op: Op::Put,
            key: b"k".to_vec(),
            value: b"v".to_vec(),
        };
        let mut bytes = r.encode();
        bytes.truncate(bytes.len() - 3);

        let result = Record::decode(&mut Cursor::new(bytes));
        assert!(matches!(result, Err(RecordError::Truncated)));
    }

    #[test]
    fn empty_stream_is_clean_eof() {
        let bytes: Vec<u8> = vec![];
        let result = Record::decode(&mut Cursor::new(bytes)).unwrap();
        assert!(result.is_none());
    }
}

#[cfg(test)]
mod wal_tests {
    use mini_lsm::{Op, Record, RecordError, Wal};
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn tmp_path() -> String {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir()
            .join(format!("wal_test_{}_{}", std::process::id(), id))
            .to_str()
            .unwrap()
            .to_owned()
    }

    fn put(key: &[u8], value: &[u8]) -> Record {
        Record { op: Op::Put, key: key.to_vec(), value: value.to_vec() }
    }

    fn del(key: &[u8]) -> Record {
        Record { op: Op::Delete, key: key.to_vec(), value: vec![] }
    }

    #[test]
    fn recover_empty_file() {
        let path = tmp_path();
        Wal::new(&path).unwrap();
        assert!(Wal::recover(&path).unwrap().is_empty());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn append_single_put_and_recover() {
        let path = tmp_path();
        let mut wal = Wal::new(&path).unwrap();
        let rec = put(b"hello", b"world");
        wal.append(&rec).unwrap();
        assert_eq!(Wal::recover(&path).unwrap(), vec![rec]);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn append_single_delete_and_recover() {
        let path = tmp_path();
        let mut wal = Wal::new(&path).unwrap();
        let rec = del(b"gone");
        wal.append(&rec).unwrap();
        assert_eq!(Wal::recover(&path).unwrap(), vec![rec]);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn append_multiple_records_preserves_order() {
        let path = tmp_path();
        let mut wal = Wal::new(&path).unwrap();
        let recs = vec![put(b"a", b"1"), put(b"b", b"2"), del(b"a"), put(b"c", b"3")];
        for r in &recs {
            wal.append(r).unwrap();
        }
        assert_eq!(Wal::recover(&path).unwrap(), recs);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn append_empty_key_and_value() {
        let path = tmp_path();
        let mut wal = Wal::new(&path).unwrap();
        let rec = put(b"", b"");
        wal.append(&rec).unwrap();
        assert_eq!(Wal::recover(&path).unwrap(), vec![rec]);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn append_large_value() {
        let path = tmp_path();
        let mut wal = Wal::new(&path).unwrap();
        let rec = put(b"big", &vec![0xABu8; 64 * 1024]);
        wal.append(&rec).unwrap();
        assert_eq!(Wal::recover(&path).unwrap(), vec![rec]);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn append_batch_empty_slice() {
        let path = tmp_path();
        let mut wal = Wal::new(&path).unwrap();
        wal.append_batch(&[]).unwrap();
        assert!(Wal::recover(&path).unwrap().is_empty());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn append_batch_recovers_all_records() {
        let path = tmp_path();
        let mut wal = Wal::new(&path).unwrap();
        let recs = vec![put(b"x", b"1"), del(b"y"), put(b"z", b"3")];
        wal.append_batch(&recs).unwrap();
        assert_eq!(Wal::recover(&path).unwrap(), recs);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn append_batch_equivalent_to_sequential_appends() {
        let path_batch = tmp_path();
        let path_seq = tmp_path();
        let recs = vec![put(b"k1", b"v1"), put(b"k2", b"v2"), del(b"k1")];

        Wal::new(&path_batch).unwrap().append_batch(&recs).unwrap();

        let mut wal = Wal::new(&path_seq).unwrap();
        for r in &recs {
            wal.append(r).unwrap();
        }

        assert_eq!(
            Wal::recover(&path_batch).unwrap(),
            Wal::recover(&path_seq).unwrap(),
        );
        std::fs::remove_file(&path_batch).ok();
        std::fs::remove_file(&path_seq).ok();
    }

    #[test]
    fn records_survive_wal_reopen() {
        let path = tmp_path();
        {
            let mut wal = Wal::new(&path).unwrap();
            wal.append(&put(b"key", b"val")).unwrap();
        }
        {
            let mut wal = Wal::new(&path).unwrap();
            wal.append(&del(b"key")).unwrap();
        }
        assert_eq!(
            Wal::recover(&path).unwrap(),
            vec![put(b"key", b"val"), del(b"key")],
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn recover_truncated_last_record_is_silently_skipped() {
        let path = tmp_path();
        let mut wal = Wal::new(&path).unwrap();
        wal.append(&put(b"good", b"record")).unwrap();
        drop(wal);

        // Partial record: op=Put, key_len=3 (4 bytes), then EOF mid-key
        let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(&[0x00, 0x03, 0x00, 0x00, 0x00]).unwrap();

        assert_eq!(Wal::recover(&path).unwrap(), vec![put(b"good", b"record")]);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn recover_crc_mismatch_returns_error() {
        let path = tmp_path();
        let mut wal = Wal::new(&path).unwrap();
        wal.append(&put(b"key", b"val")).unwrap();
        drop(wal);

        let mut data = std::fs::read(&path).unwrap();
        *data.last_mut().unwrap() ^= 0xFF;
        std::fs::write(&path, &data).unwrap();

        assert!(matches!(
            Wal::recover(&path),
            Err(RecordError::CrcMismatch { .. })
        ));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn delete_removes_file() {
        let path = tmp_path();
        let wal = Wal::new(&path).unwrap();
        wal.delete().unwrap();
        assert!(!std::path::Path::new(&path).exists());
    }

    #[test]
    fn recover_after_delete_returns_error() {
        let path = tmp_path();
        let wal = Wal::new(&path).unwrap();
        wal.delete().unwrap();
        assert!(Wal::recover(&path).is_err());
    }
}
