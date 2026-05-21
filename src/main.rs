use mini_lsm::{Op, Record, Wal};

fn main() {
    let record = Record {
        op: Op::Put,
        key: b"hello".to_vec(),
        value: b"world".to_vec(),
    };

    let mut wal = Wal::new("wal.log").expect("Failed to create WAL");
    wal.append(&record).expect("Failed to append to WAL");
}
