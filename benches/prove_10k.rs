use criterion::{
    black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion,
};
use rand::{rngs::StdRng, RngCore, SeedableRng};
use std::time::Duration;

use rootsmith::crypto::merkle_accumulator::MerkleAccumulator;
use rootsmith::crypto::sparse_merkle_accumulator::SparseMerkleAccumulator;
use rootsmith::traits::Accumulator;
use rootsmith::types::{Key32, Value32};

const N: usize = 1000_000;

// deterministic data
fn gen_kv(n: usize) -> Vec<(Key32, Value32)> {
    let mut rng = StdRng::seed_from_u64(42);
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let mut k = [0u8; 32];
        let mut v = [0u8; 32];
        rng.fill_bytes(&mut k);
        rng.fill_bytes(&mut v);
        out.push((k, v));
    }
    out
}

fn bench_prove_10k(c: &mut Criterion) {
    let kvs = gen_kv(N);
    let keys: Vec<Key32> = kvs.iter().map(|(k, _)| *k).collect();

    // Criterion config riêng cho bài này (vì mỗi iter nặng)
    let mut group = c.benchmark_group("prove_10k");
    group.sample_size(10); // 100 samples là quá nặng với prove 10k
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(8));

    // ---------- Merkle: prove_many ----------
    group.bench_function(BenchmarkId::new("merkle_prove_many", N), |b| {
        b.iter_batched(
            || {
                // setup state mỗi iteration (để fair, không reuse state giữa iterations)
                let mut merkle = MerkleAccumulator::new();
                for (k, v) in &kvs {
                    merkle.put(*k, *v).unwrap();
                }
                // nếu implementation của bạn có cache tree thì build_root ở đây để "warm cache"
                let _ = merkle.build_root().unwrap();
                merkle
            },
            |merkle| {
                // prove 10k keys once
                let proofs = merkle.prove_many(&keys).unwrap();
                black_box(proofs);
            },
            BatchSize::LargeInput,
        )
    });

    // ---------- Sparse Merkle: loop prove (hoặc prove_many nếu bạn có) ----------
    group.bench_function(BenchmarkId::new("sparse_merkle_prove_loop", N), |b| {
        b.iter_batched(
            || {
                let mut smt = SparseMerkleAccumulator::new();
                for (k, v) in &kvs {
                    smt.put(*k, *v).unwrap();
                }
                let _ = smt.build_root().unwrap();
                smt
            },
            |smt| {
                for k in &keys {
                    let p = smt.prove(k).unwrap();
                    black_box(p);
                }
            },
            BatchSize::LargeInput,
        )
    });

    group.finish();
}

criterion_group!(benches, bench_prove_10k);
criterion_main!(benches);