pub use test_utils::{SpireTestCluster, SpireWorkloadEntry};

/// Start a SPIRE cluster with gateway and identity workload entries.
pub async fn start() -> SpireTestCluster {
    SpireTestCluster::start(&[
        SpireWorkloadEntry {
            spiffe_id: "spiffe://home.ryanseipp.com/gateway".into(),
            dns: "gateway".into(),
        },
        SpireWorkloadEntry {
            spiffe_id: "spiffe://home.ryanseipp.com/identity".into(),
            dns: "identity".into(),
        },
    ])
    .await
}
