use mini_lsm::{Op, Record, RecordError, SyncMode, Wal, WalIter};
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

#[test]
fn path_matches_constructor_argument() {
    let path = tmp_path();
    let wal = Wal::new(&path).unwrap();
    assert_eq!(wal.path().to_str().unwrap(), path);
    std::fs::remove_file(&path).ok();
}

#[test]
fn size_is_zero_on_empty_wal() {
    let path = tmp_path();
    let wal = Wal::new(&path).unwrap();
    assert_eq!(wal.size(), 0);
    std::fs::remove_file(&path).ok();
}

#[test]
fn size_grows_after_append() {
    let path = tmp_path();
    let mut wal = Wal::new(&path).unwrap();
    assert_eq!(wal.size(), 0);
    wal.append(&put(b"k", b"v")).unwrap();
    assert!(wal.size() > 0);
    let size_after_one = wal.size();
    wal.append(&put(b"k2", b"v2")).unwrap();
    assert!(wal.size() > size_after_one);
    std::fs::remove_file(&path).ok();
}

#[test]
fn size_matches_file_size_on_disk() {
    let path = tmp_path();
    let mut wal = Wal::new(&path).unwrap();
    wal.append(&put(b"hello", b"world")).unwrap();
    wal.append(&del(b"hello")).unwrap();
    let on_disk = std::fs::metadata(&path).unwrap().len();
    assert_eq!(wal.size(), on_disk);
    std::fs::remove_file(&path).ok();
}

#[test]
fn size_initialised_from_existing_file_on_reopen() {
    let path = tmp_path();
    let mut wal = Wal::new(&path).unwrap();
    wal.append(&put(b"k", b"v")).unwrap();
    let size_before = wal.size();
    drop(wal);

    let wal2 = Wal::new(&path).unwrap();
    assert_eq!(wal2.size(), size_before);
    std::fs::remove_file(&path).ok();
}

#[test]
fn size_grows_after_append_batch() {
    let path = tmp_path();
    let mut wal = Wal::new(&path).unwrap();
    let recs = vec![put(b"a", b"1"), put(b"b", b"2"), del(b"a")];
    wal.append_batch(&recs).unwrap();
    let on_disk = std::fs::metadata(&path).unwrap().len();
    assert_eq!(wal.size(), on_disk);
    std::fs::remove_file(&path).ok();
}

#[test]
fn batch_only_mode_recovers_correctly() {
    let path = tmp_path();
    let mut wal = Wal::with_sync_mode(&path, SyncMode::FsyncOnBatch).unwrap();
    let recs = vec![put(b"x", b"1"), del(b"y")];
    for r in &recs {
        wal.append(r).unwrap();
    }
    assert_eq!(Wal::recover(&path).unwrap(), recs);
    std::fs::remove_file(&path).ok();
}

#[test]
fn never_mode_recovers_correctly() {
    let path = tmp_path();
    let mut wal = Wal::with_sync_mode(&path, SyncMode::FsyncNever).unwrap();
    let recs = vec![put(b"x", b"1"), del(b"y")];
    for r in &recs {
        wal.append(r).unwrap();
    }
    assert_eq!(Wal::recover(&path).unwrap(), recs);
    std::fs::remove_file(&path).ok();
}

#[test]
fn sync_mode_does_not_affect_recovered_data() {
    let modes = [SyncMode::FsyncAlways, SyncMode::FsyncOnBatch, SyncMode::FsyncNever];
    let recs = vec![put(b"k1", b"v1"), del(b"k2"), put(b"k3", b"v3")];

    let paths: Vec<String> = modes.iter().map(|_| tmp_path()).collect();
    for (mode, path) in modes.iter().zip(paths.iter()) {
        let mut wal = Wal::with_sync_mode(path, *mode).unwrap();
        wal.append_batch(&recs).unwrap();
    }

    let recovered: Vec<Vec<Record>> = paths
        .iter()
        .map(|p| Wal::recover(p).unwrap())
        .collect();

    assert!(recovered.windows(2).all(|w| w[0] == w[1]));

    for path in &paths {
        std::fs::remove_file(path).ok();
    }
}

#[test]
fn iter_yields_same_records_as_recover() {
    let path = tmp_path();
    let mut wal = Wal::new(&path).unwrap();
    let recs = vec![put(b"a", b"1"), del(b"b"), put(b"c", b"3")];
    wal.append_batch(&recs).unwrap();
    drop(wal);

    let from_iter: Vec<Record> = WalIter::open(&path)
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    assert_eq!(from_iter, Wal::recover(&path).unwrap());
    std::fs::remove_file(&path).ok();
}

#[test]
fn iter_on_empty_file_yields_nothing() {
    let path = tmp_path();
    Wal::new(&path).unwrap();
    assert_eq!(WalIter::open(&path).unwrap().count(), 0);
    std::fs::remove_file(&path).ok();
}

#[test]
fn iter_stops_at_truncated_record() {
    let path = tmp_path();
    let mut wal = Wal::new(&path).unwrap();
    wal.append(&put(b"good", b"record")).unwrap();
    drop(wal);

    let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
    f.write_all(&[0x00, 0x03, 0x00, 0x00, 0x00]).unwrap();

    let records: Vec<Record> = WalIter::open(&path).unwrap()
        .take_while(|r| r.is_ok())
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(records, vec![put(b"good", b"record")]);
    std::fs::remove_file(&path).ok();
}

#[test]
fn iter_surfaces_crc_error() {
    let path = tmp_path();
    let mut wal = Wal::new(&path).unwrap();
    wal.append(&put(b"key", b"val")).unwrap();
    drop(wal);

    let mut data = std::fs::read(&path).unwrap();
    *data.last_mut().unwrap() ^= 0xFF;
    std::fs::write(&path, &data).unwrap();

    let result: Vec<Result<Record, _>> = WalIter::open(&path).unwrap().collect();
    assert!(matches!(result[0], Err(RecordError::CrcMismatch { .. })));
    std::fs::remove_file(&path).ok();
}
