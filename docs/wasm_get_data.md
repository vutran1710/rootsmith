# WASM Host Usage: Upstream → Accumulator with Trait Objects

## Overview

This document shows how to update the MerkleAccumulator to accept trait objects (`dyn Trait`) directly from WASM host, eliminating the need for intermediate struct conversions.

## Current Flow vs Enhanced Flow

### Current Flow (with conversion)
```
WASM Host → Trait Object → IncomingRecord → RawRecord → Accumulator
```

### Enhanced Flow (direct trait objects)
```
WASM Host → Trait Object → Accumulator (direct)
```

## Updating MerkleAccumulator to Accept Trait Objects

### Step 1: Add Trait Object Method to Accumulator Trait

```rust
// In src/traits/accumulator.rs

use crate::wasm_host::traits::{RecordMeta, ToStandardData};

#[async_trait]
pub trait Accumulator: Send + Sync {
    fn id(&self) -> &'static str;

    // Existing method (for backward compatibility)
    async fn commit(
        &mut self,
        records: &[RawRecord],
        result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<()>;

    // New method: Accept trait objects directly
    async fn commit_trait(
        &mut self,
        records: &[Box<dyn ToStandardData>],
        result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<()>;
}
```

### Step 2: Implement Trait Object Method in MerkleAccumulator

```rust
// In src/accumulator/merkle_accumulator.rs

use crate::wasm_host::traits::{RecordMeta, ToStandardData};

#[async_trait]
impl Accumulator for MerkleAccumulator {
    fn id(&self) -> &'static str {
        "merkle"
    }

    // Existing implementation (unchanged)
    async fn commit(
        &mut self,
        records: &[RawRecord],
        result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<()> {
        // ... existing implementation ...
    }

    // New implementation: Accept trait objects directly
    async fn commit_trait(
        &mut self,
        records: &[Box<dyn ToStandardData>],
        result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<()> {
        // Clear any existing state
        self.flush()?;

        // Extract data from trait objects and build Merkle tree
        for trait_obj in records {
            // Use trait methods to get key and value
            let key = trait_obj.key();
            let value = trait_obj.value();
            
            // Build leaf hash: H(key || value)
            let leaf = Self::leaf_hash(&key, &value);
            let index = self.leaves.len();
            self.leaves.push(leaf);
            self.key_to_index.insert(key, index);
        }

        // Build the root
        let root = self.build_root()?;

        // Generate proofs for all records using trait objects
        let mut proofs = HashMap::new();
        for trait_obj in records {
            let key = trait_obj.key();
            if let Some(proof) = self.prove(&key)? {
                proofs.insert(key, proof);
            }
        }

        // Get timestamp from first record (all records in batch have same timestamp)
        let committed_at = if !records.is_empty() {
            records[0].timestamp()
        } else {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("System time before UNIX_EPOCH")
                .as_secs()
        };

        // Create and send result via channel
        let result = CommitmentResult {
            commitment: root,
            proofs: Some(proofs),
            committed_at,
        };

        result_tx.send(result).await?;
        Ok(())
    }
}
```

### Step 3: Update AccumulatorVariant to Support Trait Objects

```rust
// In src/accumulator/variant.rs

#[async_trait]
impl Accumulator for AccumulatorVariant {
    fn id(&self) -> &'static str {
        match self {
            AccumulatorVariant::Merkle(inner) => inner.id(),
            AccumulatorVariant::SparseMerkle(inner) => inner.id(),
        }
    }

    async fn commit(
        &mut self,
        records: &[RawRecord],
        result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<()> {
        match self {
            AccumulatorVariant::Merkle(inner) => inner.commit(records, result_tx).await,
            AccumulatorVariant::SparseMerkle(inner) => inner.commit(records, result_tx).await,
        }
    }

    async fn commit_trait(
        &mut self,
        records: &[Box<dyn ToStandardData>],
        result_tx: AsyncSender<CommitmentResult>,
    ) -> Result<()> {
        match self {
            AccumulatorVariant::Merkle(inner) => inner.commit_trait(records, result_tx).await,
            AccumulatorVariant::SparseMerkle(inner) => inner.commit_trait(records, result_tx).await,
        }
    }
}
```

## Complete Example: Upstream → Accumulator with Trait Objects

### Step 1: Upstream Receives Data and Processes Through WASM

```rust
// In upstream connector (HTTP/WebSocket/etc)

use crate::wasm_host::{WasmPluginHost, WasmLimits, ToStandardData};
use crate::types::IncomingRecord;
use kanal::AsyncSender;

pub struct WasmUpstream {
    wasm_host: Arc<Mutex<WasmPluginHost>>,
    // Channel for sending trait objects directly to accumulator
    accumulator_tx: AsyncSender<Vec<Box<dyn ToStandardData>>>,
}

impl WasmUpstream {
    async fn handle_request(&mut self, raw_bytes: &[u8]) -> Result<()> {
        // Process through WASM host
        let mut host = self.wasm_host.lock().await;
        let trait_obj: Box<dyn ToStandardData> = host.process_input::<dyn ToStandardData>(raw_bytes)?;
        
        // Option 1: Send trait object directly to accumulator (new way)
        let trait_objects = vec![trait_obj];
        self.accumulator_tx.send(trait_objects).await?;
        
        // Option 2: Convert to IncomingRecord for storage (if needed)
        // let record = IncomingRecord {
        //     namespace: trait_obj.namespace(),
        //     key: trait_obj.key(),
        //     value: trait_obj.value(),
        //     timestamp: trait_obj.timestamp(),
        // };
        // storage_tx.send(record).await?;
        
        Ok(())
    }
}
```

### Step 2: Commit Cycle Collects Trait Objects and Sends to Accumulator

```rust
// In commit cycle task

use crate::wasm_host::ToStandardData;
use crate::accumulator::AccumulatorVariant;
use kanal::{unbounded_async, AsyncSender, AsyncReceiver};

async fn process_commit_cycle_with_traits(
    accumulator: &mut AccumulatorVariant,
    trait_objects_rx: AsyncReceiver<Vec<Box<dyn ToStandardData>>>,
    commit_tx: AsyncSender<CommitmentResult>,
) -> Result<()> {
    // Collect trait objects from upstream
    let mut batch: Vec<Box<dyn ToStandardData>> = Vec::new();
    
    // Receive trait objects (with timeout)
    loop {
        tokio::select! {
            Ok(trait_objects) = trait_objects_rx.recv() => {
                batch.extend(trait_objects);
                
                // When batch is ready, send to accumulator
                if batch.len() >= BATCH_SIZE {
                    accumulator.commit_trait(&batch, commit_tx.clone()).await?;
                    batch.clear();
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(BATCH_INTERVAL_SECS)) => {
                // Timeout: send current batch even if not full
                if !batch.is_empty() {
                    accumulator.commit_trait(&batch, commit_tx.clone()).await?;
                    batch.clear();
                }
            }
        }
    }
}
```

### Step 3: Accumulator Processes Trait Objects Directly

```rust
// In MerkleAccumulator::commit_trait()

async fn commit_trait(
    &mut self,
    records: &[Box<dyn ToStandardData>],
    result_tx: AsyncSender<CommitmentResult>,
) -> Result<()> {
    println!("🌳 Accumulator processing {} trait objects", records.len());
    
    // Clear existing state
    self.flush()?;
    
    // Build Merkle tree from trait objects
    for trait_obj in records {
        // Extract data using trait methods
        let namespace = trait_obj.namespace();
        let key = trait_obj.key();
        let value = trait_obj.value();
        let timestamp = trait_obj.timestamp();
        
        println!("   Processing: namespace={}..., key={}..., timestamp={}",
            hex::encode(&namespace[..8]),
            hex::encode(&key[..8]),
            timestamp);
        
        // Build leaf hash
        let leaf = Self::leaf_hash(&key, &value);
        let index = self.leaves.len();
        self.leaves.push(leaf);
        self.key_to_index.insert(key, index);
    }
    
    // Build root
    let root = self.build_root()?;
    println!("✅ Root hash: {}...", hex::encode(&root[..8]));
    
    // Generate proofs using trait object methods
    let mut proofs = HashMap::new();
    for trait_obj in records {
        let key = trait_obj.key();
        if let Some(proof) = self.prove(&key)? {
            proofs.insert(key, proof);
        }
    }
    
    // Get timestamp from first record
    let committed_at = records[0].timestamp();
    
    // Create CommitmentResult
    let result = CommitmentResult {
        commitment: root,
        proofs: Some(proofs),
        committed_at,
    };
    
    // Send via channel
    result_tx.send(result).await?;
    Ok(())
}
```

## Usage Pattern (from test example)

Based on the test pattern in `tests/load_wasm_incoming_record_tests.rs`:

```rust
// Accumulator receives trait object directly
fn accumulator_build<T: ToStandardData>(data: Box<T>) {
    let namespace = data.namespace();
    let key = data.key();
    let value = data.value();
    let timestamp = data.timestamp();
    // ... use data for accumulation ...
}

// Usage:
let output = host.process_input::<dyn ToStandardData>(input)?;
accumulator_build(output);  // Direct trait object
```

## Complete Integration Example

```rust
use crate::wasm_host::{WasmPluginHost, WasmLimits, ToStandardData};
use crate::accumulator::{AccumulatorVariant, AccumulatorType};
use kanal::{unbounded_async, AsyncSender, AsyncReceiver};

async fn run_complete_example() -> Result<()> {
    // 1. Initialize WASM host
    let wasm_host = Arc::new(Mutex::new(
        WasmPluginHost::load("./plugins/partner.wasm", WasmLimits::default())?
    ));
    
    // 2. Initialize accumulator
    let mut accumulator = AccumulatorVariant::new(AccumulatorType::Merkle);
    
    // 3. Create channels
    let (trait_tx, trait_rx) = unbounded_async::<Vec<Box<dyn ToStandardData>>>();
    let (commit_tx, commit_rx) = unbounded_async::<CommitmentResult>();
    
    // 4. Simulate upstream receiving data
    let wasm_host_clone = Arc::clone(&wasm_host);
    let trait_tx_clone = trait_tx.clone();
    tokio::spawn(async move {
        // Simulate HTTP request
        let raw_bytes = br#"{"id":"event-123","ts_ms":1699123456000,"user_id":"user-abc","action":"click"}"#;
        
        // Process through WASM
        let mut host = wasm_host_clone.lock().await;
        match host.process_input::<dyn ToStandardData>(raw_bytes) {
            Ok(trait_obj) => {
                // Send trait object directly to accumulator
                trait_tx_clone.send(vec![trait_obj]).await.unwrap();
                println!("✅ Sent trait object to accumulator");
            }
            Err(e) => println!("❌ WASM processing failed: {}", e),
        }
    });
    
    // 5. Commit cycle: collect and send to accumulator
    tokio::spawn(async move {
        let mut batch: Vec<Box<dyn ToStandardData>> = Vec::new();
        
        while let Ok(trait_objects) = trait_rx.recv().await {
            batch.extend(trait_objects);
            
            // Send batch to accumulator
            if !batch.is_empty() {
                accumulator.commit_trait(&batch, commit_tx.clone()).await.unwrap();
                println!("✅ Sent {} trait objects to accumulator", batch.len());
                batch.clear();
            }
        }
    });
    
    // 6. Receive commitment result
    if let Ok(result) = commit_rx.recv().await {
        println!("🎉 Commitment result:");
        println!("   root: {}...", hex::encode(&result.commitment[..8]));
        println!("   proofs: {}", result.proofs.as_ref().map(|p| p.len()).unwrap_or(0));
    }
    
    Ok(())
}
```

## Benefits of Direct Trait Object Approach

1. **No Conversion Overhead**: Trait objects go directly to accumulator
2. **Type Safety**: Compile-time guarantees via trait bounds
3. **Flexibility**: Accumulator can access rich metadata (namespace, timestamp) directly
4. **Clean API**: Single method `commit_trait()` handles trait objects
5. **Backward Compatible**: Existing `commit()` method still works with `RawRecord[]`

## Migration Path

1. **Phase 1**: Add `commit_trait()` method alongside existing `commit()`
2. **Phase 2**: Update commit cycle to use `commit_trait()` when trait objects are available
3. **Phase 3**: Gradually migrate all paths to use trait objects
4. **Phase 4**: (Optional) Deprecate `commit()` method if no longer needed

## Summary

The enhanced MerkleAccumulator can now:
- Accept trait objects directly via `commit_trait()`
- Extract data using trait methods (`namespace()`, `key()`, `value()`, `timestamp()`)
- Build Merkle tree without intermediate struct conversions
- Maintain backward compatibility with existing `commit()` method

This eliminates the need for `IncomingRecord → RawRecord` conversion when using WASM plugins, making the flow more efficient and type-safe.
