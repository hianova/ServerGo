use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ServerGo::storage::{PureCacheStore, TieredStore};
use io_oi_core::{SignedRecord, Hash32, StateStore};
use dualcache_ff::Config;
use cdDB::CdDBDispatcher;

fn bench_storage(c: &mut Criterion) {
    let namespace: Hash32 = [0xAA; 32];
    
    // 1. Bench PureCacheStore
    let config1 = Config::with_memory_budget(512, 60);
    let pure_store = PureCacheStore::new(namespace, config1);
    let mut group = c.benchmark_group("storage_pure");

    group.bench_function("pure_apply", |b| {
        b.iter(|| {
            let record = SignedRecord {
                epoch_id: 1,
                payload: vec![0u8; 100],
                judge_signature: [0u8; 64],
                record_type: 0,
            };
            pure_store.apply_signed_record(black_box(record));
        })
    });

    let hash: Hash32 = [0u8; 32];
    group.bench_function("pure_get", |b| {
        b.iter(|| {
            pure_store.get_record(black_box(&hash));
        })
    });
    group.finish();

    // 2. Bench TieredStore
    let config2 = Config::with_memory_budget(512, 60);
    let mut db = CdDBDispatcher::new_std(Some("data_bench".to_string()));
    let tiered_store = TieredStore::new(namespace, config2, &mut db, "bench_partition".to_string());
    let mut group = c.benchmark_group("storage_tiered");

    group.bench_function("tiered_apply", |b| {
        b.iter(|| {
            let record = SignedRecord {
                epoch_id: 1,
                payload: vec![0u8; 100],
                judge_signature: [0u8; 64],
                record_type: 0,
            };
            tiered_store.apply_signed_record(black_box(record));
        })
    });

    group.bench_function("tiered_get", |b| {
        b.iter(|| {
            tiered_store.get_record(black_box(&hash));
        })
    });
    group.finish();
}

criterion_group!(benches, bench_storage);
criterion_main!(benches);
