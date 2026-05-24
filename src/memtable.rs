use std::collections::BTreeMap;

pub struct Memtable {
    map: BTreeMap<Vec<u8>, Option<Vec<u8>>>,
}

impl Memtable {
    pub fn new() -> Result<Self, ()> {
        Ok(Memtable {
            map: BTreeMap::new(),
        })
    }

    pub fn insert(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.map.insert(key, Some(value));
    }
    pub fn delete(&mut self, key: Vec<u8>) {
        self.map.remove(&key);
    }

    pub fn get(&self, key: &[u8]) -> Lookup {
        match self.map.get(key) {
            Some(Some(value)) => Lookup::Found(value.clone()),
            Some(None) => Lookup::Deleted,
            None => Lookup::Missing,
        }
    }

    // pub fn approximate_size(&self) -> usize;
    // pub fn len(&self) -> usize;
    // pub fn is_empty(&self) -> bool;

    // pub fn iter(&self) -> impl Iterator<Item = (&[u8], &Option<Vec<u8>>)>;
}

#[derive(Debug)]
pub enum Lookup {
    Found(Vec<u8>),
    Deleted,
    Missing,
}
