use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ServerGo::storage::{PureCacheStore, TieredStore};
use io_oi_core::{SignedRecord, Hash32, StateStore};
use cdDB::CdDBDispatcher;

fn bench_storage(c: &mut Criterion) {
    let namespace: Hash32 = [0xAA; 32];
    
    // 1. Bench PureCacheStore
    let pure_store = PureCacheStore::new(namespace, 512);
    let mut group = c.benchmark_group("storage_pure");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(1));
    group.warm_up_time(std::time::Duration::from_secs(1));

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
    let mut db = CdDBDispatcher::<1024>::new_std(Some("data_bench".to_string()));
    let tiered_store = TieredStore::new(namespace, 512, &mut db, "bench_partition".to_string());
    let mut group = c.benchmark_group("storage_tiered");
    group.sample_size(10);
    // Use very small measurement time for tiered_store because it pushes async vectors rapidly and can OOM
    group.measurement_time(std::time::Duration::from_millis(50));
    group.warm_up_time(std::time::Duration::from_millis(10));

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
