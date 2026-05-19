use std::io::Cursor;

use mini_lsm::{Op, Record};

fn main() {
    let record = Record {
        op: Op::Put,
        key: b"hello".to_vec(),
        value: b"world".to_vec(),
    };

    let bytes = record.encode();
    println!("encoded {} bytes: {:?}", bytes.len(), bytes);

    let decoded = Record::decode(&mut Cursor::new(&bytes))
        .expect("decode error")
        .expect("unexpected EOF");

    println!(
        "decoded: op={:?} key={:?} value={:?}",
        decoded.op, decoded.key, decoded.value
    );
    assert_eq!(decoded, record);
    println!("round-trip OK");
}
