use crate::{
    infra::delta_sync::{DeltaReplicaTestGuard, set_replica_preprocessing_override},
    tests::{
        self,
        setup::{ab_order, ab_pool, ab_solution},
    },
};

struct ReplicaOverrideGuard;

impl ReplicaOverrideGuard {
    fn enable() -> Self {
        set_replica_preprocessing_override(Some(true));
        Self
    }
}

impl Drop for ReplicaOverrideGuard {
    fn drop(&mut self) {
        set_replica_preprocessing_override(None);
    }
}

#[tokio::test]
async fn thin_mode_falls_back_to_full_body_when_replica_unavailable() {
    let _replica_guard = DeltaReplicaTestGuard::acquire();
    let _env = ReplicaOverrideGuard::enable();

    let test = tests::setup()
        .name("thin body fallback")
        .pool(ab_pool())
        .order(ab_order())
        .solution(ab_solution())
        .done()
        .await;

    let solve = test.solve_with_body_mode("thin").await;
    solve.ok();
}
