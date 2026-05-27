use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ServerGo::storage::{PureCacheStore, TieredStore};
use io_oi_core::{SignedRecord, Hash32, StateStore};
use cdDB::CdDBDispatcher;

fn bench_storage(c: &mut Criterion) {
    let namespace: Hash32 = [0xAA; 32];
    let hash: Hash32 = [0u8; 32];
    
    // ==========================================
    // 1. Pure Cache Store Benchmarks
    // ==========================================
    let pure_store = PureCacheStore::new(namespace, 512);

    // 1.a Pure Cache Write (Short measurement time to avoid large Vec clone overhead)
    let mut group_write = c.benchmark_group("storage_pure_write");
    group_write.sample_size(10);
    group_write.measurement_time(std::time::Duration::from_millis(50));
    group_write.warm_up_time(std::time::Duration::from_millis(10));
    group_write.bench_function("pure_apply", |b| {
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
    group_write.finish();

    // 1.b Pure Cache Read (Long measurement time for ultra-precise nanosecond latency)
    // Warm up the cache with the record and sleep to ensure the asynchronous background dispatcher has fully committed it to memory.
    let record = SignedRecord {
        epoch_id: 1,
        payload: vec![0u8; 100],
        judge_signature: [0u8; 64],
        record_type: 0,
    };
    pure_store.apply_signed_record(record);
    std::thread::sleep(std::time::Duration::from_millis(100)); // Ensure async commit completes

    let mut group_read = c.benchmark_group("storage_pure_read");
    group_read.sample_size(100);
    group_read.measurement_time(std::time::Duration::from_secs(3));
    group_read.warm_up_time(std::time::Duration::from_secs(1));
    group_read.bench_function("pure_get", |b| {
        b.iter(|| {
            pure_store.get_record(black_box(&hash));
        })
    });
    group_read.finish();

    // ==========================================
    // 2. Tiered Store Benchmarks
    // ==========================================
    let mut db = CdDBDispatcher::<1024>::new_std(Some("data_bench".to_string()));
    let tiered_store = TieredStore::new(namespace, 512, &mut db, "bench_partition".to_string());

    // 2.a Tiered Write (Short measurement time to avoid channel saturation / disk bottleneck)
    let mut group_write_t = c.benchmark_group("storage_tiered_write");
    group_write_t.sample_size(10);
    group_write_t.measurement_time(std::time::Duration::from_millis(50));
    group_write_t.warm_up_time(std::time::Duration::from_millis(10));
    group_write_t.bench_function("tiered_apply", |b| {
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
    group_write_t.finish();

    // 2.b Tiered Read (Long measurement time for precise nanosecond latency)
    // Warm up the L1 write-through cache and sleep to ensure the asynchronous commit completes.
    let record_t = SignedRecord {
        epoch_id: 1,
        payload: vec![0u8; 100],
        judge_signature: [0u8; 64],
        record_type: 0,
    };
    tiered_store.apply_signed_record(record_t);
    std::thread::sleep(std::time::Duration::from_millis(100)); // Ensure async commit completes

    let mut group_read_t = c.benchmark_group("storage_tiered_read");
    group_read_t.sample_size(100);
    group_read_t.measurement_time(std::time::Duration::from_secs(3));
    group_read_t.warm_up_time(std::time::Duration::from_secs(1));
    group_read_t.bench_function("tiered_get", |b| {
        b.iter(|| {
            tiered_store.get_record(black_box(&hash));
        })
    });
    group_read_t.finish();
}

criterion_group!(benches, bench_storage);
criterion_main!(benches);
