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
