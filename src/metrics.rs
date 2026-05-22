use foundations::telemetry::metrics::{metrics, Counter};

#[metrics]
pub(crate) mod db_metrics {
    /// Total number of database GET requests
    pub fn db_gets() -> Counter;
    
    /// Total number of database PUT requests
    pub fn db_puts() -> Counter;
    
    /// Total number of L1 cache hits in Tiered Storage
    pub fn cache_hits() -> Counter;
    
    /// Total number of L1 cache misses in Tiered Storage
    pub fn cache_misses() -> Counter;
}
