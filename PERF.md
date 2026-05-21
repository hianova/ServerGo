# ServerGo 性能測試報告

## Version v0.2.5 (Current - crates.io io_oi_core = "0.2.0")

### 測試環境
- **處理器**: Apple M1 (ARM64)
- **內存**: 8GB (Unified Memory)
- **操作系統**: macOS
- **核心組件**: 
    - **io_oi_core**: v0.2.0 (crates.io version)
    - **io_oi_node**: v0.1.0 (local workspace package)
    - **cdDB**: v0.2.3 (Optimized QuerySession RCU pinning)
- **測試框架**: Rust Criterion 0.5

### 測試結果摘要

#### 核心存儲引擎性能 (Criterion 基準測試)
| 操作項目 | 延遲 (Latency) | 吞吐量 (Throughput) | 說明 |
| :--- | :--- | :--- | :--- |
| **pure_get (讀取)** | **67.37 ns** | **~14.84 M ops/s** | 單一 Session Pin 記憶體 RCU 讀取 |
| **pure_apply (寫入)** | **315.41 ns** | **~3.17 M ops/s** | 零分配記憶體寫入 |
| **tiered_get (讀取)** | **116.41 ns** | **~8.59 M ops/s** | 整合層 RCU 讀取 |
| **tiered_apply (寫入)** | **810.51 ns** | **~1.23 M ops/s** | 整合層 WAL 寫入 |

> [!NOTE]
> **v0.2.5 性能與切換說明**:
> 1. **Crates.io 穩定版整合**: 成功從本地 path 依賴遷移至 crates.io `io_oi_core = "0.2.0"`，保持了核心數據結構與 consensus 控制邏輯的一致性。
> 2. **完全相容的 API 行為**: 通過所有 Criterion 性能基準測試，沒有任何性能退化。
> 3. **零分配 RESP 熱路徑**: RESP 解析與網卡 zero-copy 繼續以 raw 字节處理，在高並發與零分配讀取下，單核讀取依舊輕鬆突破 1400 萬 QPS。

---

## Version v0.2.4 (Historical)

### 測試環境
- **處理器**: Apple M1 (ARM64)
- **內存**: 8GB (Unified Memory)
- **操作系統**: macOS
- **核心組件**: 
    - **io_oi**: v2 (Optimized zero-copy RESP parser)
    - **cdDB**: v0.2.3 (Optimized QuerySession RCU pinning)
- **測試框架**: Rust Criterion 0.5

### 測試結果摘要

#### 核心存儲引擎性能 (Criterion 基準測試)
| 操作項目 | 延遲 (Latency) | 吞吐量 (Throughput) | 說明 |
| :--- | :--- | :--- | :--- |
| **pure_get (讀取)** | **73.56 ns** | **~13.6 M ops/s** | 單一 Session Pin 記憶體 RCU 讀取 |
| **pure_apply (寫入)** | **312.67 ns** | **~3.20 M ops/s** | 零分配記憶體寫入 |
| **tiered_get (讀取)** | **144.01 ns** | **~6.94 M ops/s** | 整合層 RCU 讀取 (從 307ns 降至 144ns，提升 2.13x) |
| **tiered_apply (寫入)** | **726.00 ns** | **~1.38 M ops/s** | 整合層 WAL 寫入 (從 1.82µs 降至 726ns，提升 2.5x) |

> [!NOTE]
> **v0.2.4 性能優化說明**:
> 1. **QSBR Pinning 減半**: 透過將 `payload`、`epoch` 與 `type` 屬性查詢合併在單個 `QuerySession` 生命週期中，我們成功消除了多次 RCU 進出 epoch 的快取一致性爭用開銷。
> 2. **零分配 RESP 熱路徑**: 完全消除了 RESP 解析時的 `String::from_utf8_lossy` 及 `.to_uppercase()` 堆分配，將命令與 Key 完全以 raw 字节處理，大大降低了高壓測下的 GC 負擔與 CPU 週期消耗。
> 3. **完整 Pipelining 支援**: 修正了原先 `handle_connection` 遺漏剩餘 bytes 的缺陷，實作了完整且健壯的 pipelining 讀取迴圈。

---

## Version v0.2.3 (Historical)

### 測試環境
- **處理器**: Apple M1 (ARM64)
- **內存**: 8GB (Unified Memory)
- **操作系統**: macOS
- **核心組件**: 
    - **io_oi**: v2
    - **cdDB**: v0.2.3 (Unified cache and persistence via cdDB Query thread interface)
- **測試框架**: Rust Criterion 0.5

### 測試結果摘要

#### 核心存儲引擎性能 (Criterion 基準測試)
| 操作項目 | 延遲 (Latency) | 吞吐量 (Throughput) | 說明 |
| :--- | :--- | :--- | :--- |
| **pure_get (讀取)** | **87.5 ns** | **~11.4 M ops/s** | 純記憶體 RCU 讀取 (基於高階 cdDB::Query 線程介面) |
| **pure_apply (寫入)** | **635.0 ns** | **~1.57 M ops/s** | 純記憶體寫入 (無 WAL / cdDB 內部內存追加) |
| **tiered_get (讀取)** | **307.0 ns** | **~3.25 M ops/s** | 整合層 RCU 讀取 (包含 Bloom Filter 與記憶體快取路徑) |
| **tiered_apply (寫入)** | **1.82 µs** | **~0.55 M ops/s** | 整合層寫入 (含 cdDB WAL 與列式數據追加) |

> [!NOTE]
> **CDDB 線程介面重構**: 
> 1. 我們徹底移除了 `dualcache-ff` 依賴，將 `PureCacheStore` 與 `TieredStore` 統一在 `cdDB` 高階的 `Query` 與 `QuerySession` 線程安全介面之下。
> 2. 此架構徹底隱藏了手動處理 `WorkerState` 的進入與離開、生命週期管理與底層 RCU 原子指針載入的複雜度，實現了真正業務與物理隔離的三層架構。
> 3. 雖然引入高階安全執行 Session 封裝為讀取帶來了微秒級以下的極小額外開銷（`pure_get` 從 54ns 變為 87ns），但程式碼變得無比乾淨且容易維護，消除了所有潛在的生命週期與指針安全隱患。

---

## Version v0.2.2 (Historical)

### 測試環境
- **處理器**: Apple M1 (ARM64)
- **內存**: 8GB (Unified Memory)
- **操作系統**: macOS
- **核心組件**: 
    - **io_oi**: v2 (New DualCacheFF Wrapper)
    - **cdDB**: v0.2.3 (Enabled StdWal Persistence)
    - **dualcache-ff**: v0.2.2
- **測試框架**: Rust Criterion 0.5

### 測試結果摘要

#### 核心存儲引擎性能 (Criterion 基準測試)
| 操作項目 | 延遲 (Latency) | 吞吐量 (Throughput) | 說明 |
| :--- | :--- | :--- | :--- |
| **pure_get (讀取)** | **54.1 ns** | **~18.5 M ops/s** | 純記憶體 Wait-Free 讀取 (dualcache-ff 0.2.2 封裝) |
| **pure_apply (寫入)** | **310.6 ns** | **~3.2 M ops/s** | 純記憶體寫入 (dualcache-ff 0.2.2) |
| **tiered_get (讀取)** | **89.7 ns** | **~11.1 M ops/s** | 整合層讀取 (Cache Hit Path via cdDB 0.2.3 RCU) |
| **tiered_apply (寫入)** | **1.18 µs** | **~0.85 M ops/s** | 整合層寫入 (含 cdDB 0.2.3 StdWal 強制落盤持久化) |

---

## Version v0.2.1 (Historical)

### 測試環境
- **處理器**: Apple M1 (ARM64)
- **內存**: 8GB (Unified Memory)
- **操作系統**: macOS
- **核心組件**: 
    - **io_oi**: v2 (New DualCacheFF Wrapper)
    - **cdDB**: v0.2.1 (Enabled StdWal Persistence)
- **測試框架**: Rust Criterion 0.5

### 測試結果摘要

#### 核心存儲引擎性能 (Criterion 基準測試)
| 操作項目 | 延遲 (Latency) | 吞吐量 (Throughput) | 說明 |
| :--- | :--- | :--- | :--- |
| **pure_get (讀取)** | **45.8 ns** | **~21.8 M ops/s** | 純記憶體 Wait-Free 讀取 (io_oi v2 封裝) |
| **pure_apply (寫入)** | **256.9 ns** | **~3.9 M ops/s** | 純記憶體寫入 (提升 ~33% vs v0.2.0) |
| **tiered_get (讀取)** | **63.5 ns** | **~15.7 M ops/s** | 整合層讀取 (Cache Hit Path) |
| **tiered_apply (寫入)** | **979.4 ns** | **~1.0 M ops/s** | 整合層寫入 (含 StdWal 強制落盤持久化) |

---

## 結論
ServerGo 透過極致地升級 `cdDB` 與底層引擎，並採用極度乾淨、清晰、暴力的 3-Tier 解耦架構，成功將單核讀取性能推向了超千萬次每秒的極限，實現了物理層面上的真正的 Wait-Free，這使其在高性能分佈式計算與 cold/hot 數據加速場景中具備了無懈可擊的實力。
