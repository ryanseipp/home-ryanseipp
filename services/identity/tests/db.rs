#![cfg(feature = "db-tests")]

use identity::outbox;
use identity::services::sign_up;
use prost::Message;
use uuid::Uuid;

// -- Outbox tests --

#[tokio::test]
async fn outbox_fetch_unpublished_returns_inserted_event() {
    let (_container, pool) = test_utils::test_db_pool().await;

    let id = Uuid::now_v7();
    let aggregate_id = Uuid::now_v7();

    let client = pool.get().await.unwrap();
    outbox::db::insert_event(
        &client,
        id,
        "user",
        aggregate_id,
        "auth_email",
        b"test-payload",
        Some("0af7651916cd43dd8448eb211c80319c"),
        Some("b7ad6b7169203331"),
    )
    .await
    .unwrap();

    let events = outbox::db::fetch_unpublished(&pool, 10).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, id);
    assert_eq!(events[0].aggregate_type, "user");
    assert_eq!(events[0].aggregate_id, aggregate_id);
    assert_eq!(events[0].event_type, "auth_email");
    assert_eq!(events[0].payload, b"test-payload");
    assert_eq!(
        events[0].trace_id.as_deref(),
        Some("0af7651916cd43dd8448eb211c80319c")
    );
    assert_eq!(events[0].span_id.as_deref(), Some("b7ad6b7169203331"));
}

#[tokio::test]
async fn outbox_mark_published_excludes_from_fetch() {
    let (_container, pool) = test_utils::test_db_pool().await;

    let id = Uuid::now_v7();
    let aggregate_id = Uuid::now_v7();

    let client = pool.get().await.unwrap();
    outbox::db::insert_event(
        &client,
        id,
        "user",
        aggregate_id,
        "auth_email",
        b"payload",
        None,
        None,
    )
    .await
    .unwrap();

    // Before marking published
    let events = outbox::db::fetch_unpublished(&pool, 10).await.unwrap();
    assert_eq!(events.len(), 1);

    // Mark published
    outbox::db::mark_published(&pool, id).await.unwrap();

    // After marking published
    let events = outbox::db::fetch_unpublished(&pool, 10).await.unwrap();
    assert!(events.is_empty());
}

#[tokio::test]
async fn outbox_fetch_respects_batch_size() {
    let (_container, pool) = test_utils::test_db_pool().await;

    let client = pool.get().await.unwrap();
    for _ in 0..5 {
        outbox::db::insert_event(
            &client,
            Uuid::now_v7(),
            "user",
            Uuid::now_v7(),
            "auth_email",
            b"payload",
            None,
            None,
        )
        .await
        .unwrap();
    }

    let events = outbox::db::fetch_unpublished(&pool, 2).await.unwrap();
    assert_eq!(events.len(), 2);
}

#[tokio::test]
async fn outbox_advisory_lock_leader_election() {
    let (_container, pool) = test_utils::test_db_pool().await;

    // First connection acquires lock
    let conn1 = pool.get().await.unwrap();
    assert!(outbox::db::try_acquire_leader_lock(&conn1).await.unwrap());

    // Same connection re-acquires (idempotent)
    assert!(outbox::db::try_acquire_leader_lock(&conn1).await.unwrap());

    // Second connection cannot acquire it
    let conn2 = pool.get().await.unwrap();
    assert!(!outbox::db::try_acquire_leader_lock(&conn2).await.unwrap());
}

#[tokio::test]
async fn sign_up_inserts_auth_email_outbox_event() {
    use identity::proto::email::v1::{AuthEmailMessage, auth_email_message};

    let (_container, pool) = test_utils::test_db_pool().await;

    sign_up::execute(
        &pool,
        "outboxuser",
        "outbox@example.com",
        "Test",
        "User",
        Some("password123"),
        "https://test.example.com",
    )
    .await
    .unwrap();

    let events = outbox::db::fetch_unpublished(&pool, 10).await.unwrap();
    assert_eq!(events.len(), 1);

    let event = &events[0];
    assert_eq!(event.aggregate_type, "user");
    assert_eq!(event.event_type, "auth_email");

    // Decode the payload as AuthEmailMessage
    let msg = AuthEmailMessage::decode(event.payload.as_slice()).unwrap();
    assert_eq!(msg.recipient_email, "outbox@example.com");
    assert_eq!(msg.recipient_name, "Test User");
    assert!(!msg.idempotency_key.is_empty());

    // Verify the payload contains an EmailVerification
    match msg.payload {
        Some(auth_email_message::Payload::EmailVerification(ev)) => {
            assert!(!ev.verification_code.is_empty());
            assert!(
                ev.verification_link
                    .starts_with("https://test.example.com/verify-email?token=")
            );
            assert_eq!(ev.expires_in_minutes, 60);
        }
        other => panic!("expected EmailVerification payload, got {other:?}"),
    }
}
