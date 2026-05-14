use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ServerGo::storage::PureCacheStore;
use io_oi_core::{SignedRecord, Hash32, StateStore};
use dualcache_ff::Config;

fn bench_storage(c: &mut Criterion) {
    let namespace: Hash32 = [0xAA; 32];
    let config = Config::with_memory_budget(512, 60);
    let store = PureCacheStore::new(namespace, config);

    let mut group = c.benchmark_group("storage");

    group.bench_function("apply_record", |b| {
        b.iter(|| {
            let record = SignedRecord {
                epoch_id: 1,
                payload: vec![0u8; 100],
                judge_signature: [0u8; 64],
                record_type: 0,
            };
            store.apply_signed_record(black_box(record));
        })
    });

    let hash: Hash32 = [0u8; 32];
    group.bench_function("get_record", |b| {
        b.iter(|| {
            store.get_record(black_box(&hash));
        })
    });

    group.finish();
}

criterion_group!(benches, bench_storage);
criterion_main!(benches);
