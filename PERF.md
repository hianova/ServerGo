# v0.3.1 Performance Benchmark Report

## 效能測試結果 (cdDB 無鎖架構 + 全域熱索引最佳化)

在更新 `cdDB` 的佇列與 Backoff 機制後，我們重新測試了主系統的延遲。由於引進了 `GlobalHotIndex` 與消除 `Vec::clone`，讀取效能獲得了爆發性的改善，且寫入延遲依然維持在極度優越的亞微秒級別。

| 基準測試 | 原先延遲 (v0.2.x) | 最終延遲 (v0.3.1) | 效能提升比例 | 備註 |
| --- | --- | --- | --- | --- |
| `storage_pure_write/pure_apply` | ~1.43 µs | **273.29 ns** | **5.2倍提升** | 背景 WAL 為 Noop |
| `storage_tiered_write/tiered_apply` | 3.97 µs | **784.13 ns** | **5.0倍提升** | 寫入 Cache + 寫入磁碟 (無鎖派發) |
| `storage_pure_read/pure_get` | ~111 ns | **88.35 ns** | **1.25倍提升** | 全域熱索引直接訪問 |
| `storage_tiered_read/tiered_get` | ~343 ns | **249.61 ns** | **1.37倍提升** | 整合 L1/L2 讀取架構優化 |

### 優化關鍵點分析
1. **全域熱索引 (Global Hot-Index) 消除指標追逐**:
   我們將多層快取扁平化為單一的全域扁平熱索引表 `GlobalHotIndex`。透過 Wait-Free RCU 指標加載與 64 位元原子狀態設計，將 `tiered_get` 的讀取效能直接推升到 **~249 ns**。
2. **零拷貝寫入派發 (Zero-Copy Arc Dispatch)**:
   消除了雙佇列競爭後，主執行緒不再需要複製 payload 內容即可同時將資料送往 Cache 與 WAL 背景執行緒。我們使用 `CachedRecord` 搭配 `Arc<Vec<u8>>`，使記憶體分配次數最小化，進一步降低 `jemalloc` 鎖競爭。
3. **無鎖 BoundedQueue 與 Backoff 機制**:
   配合 `cdDB` 新的 `BoundedQueue` 設計，結合背景執行緒的 Backoff 等待和讓渡資源，兼顧了 CPU 利用率與整體寫入吞吐量。
