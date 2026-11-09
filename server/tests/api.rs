mod common;

use amber_server::store::memory::MemoryStore;
use std::sync::Arc;

#[tokio::test]
async fn api_behaviour_with_memory_store() {
    common::run_suite(Arc::new(MemoryStore::new())).await;
}

#[tokio::test]
async fn admin_changes_disabled_without_pin() {
    common::staff_cannot_change_rules(Arc::new(MemoryStore::new())).await;
}

#[tokio::test]
async fn login_roles_and_accounts_with_memory_store() {
    common::auth_suite(Arc::new(MemoryStore::new())).await;
}

#[tokio::test]
async fn business_profile_and_letterhead_with_memory_store() {
    common::business_suite(Arc::new(MemoryStore::new())).await;
}
