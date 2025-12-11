use anyhow::Result;
use rocksdb::{DB, Options};
use crate::types::Block;

pub struct Storage {
    db: DB,
}

impl Storage {
    pub fn new(path: &str) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db = DB::open(&opts, path)?;
        Ok(Self { db })
    }

    pub fn store_block(&self, block: &Block) -> Result<()> {
        let key = format!("block:{}", block.number);
        let value = serde_json::to_vec(block)?;
        self.db.put(key.as_bytes(), value)?;
        Ok(())
    }

    pub fn get_block(&self, number: u64) -> Result<Option<Block>> {
        let key = format!("block:{}", number);
        match self.db.get(key.as_bytes())? {
            Some(data) => {
                let block: Block = serde_json::from_slice(&data)?;
                Ok(Some(block))
            }
            None => Ok(None),
        }
    }

    pub fn scan_blocks(&self) -> Result<Vec<u64>> {
        let mut block_numbers = Vec::new();
        let iter = self.db.iterator(rocksdb::IteratorMode::Start);
        
        for item in iter {
            let (key, _value) = item?;
            let key_str = String::from_utf8_lossy(&key);
            
            if let Some(num_str) = key_str.strip_prefix("block:") {
                if let Ok(num) = num_str.parse::<u64>() {
                    block_numbers.push(num);
                }
            }
        }
        
        block_numbers.sort();
        Ok(block_numbers)
    }
}
