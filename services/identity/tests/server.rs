#![cfg(feature = "db-tests")]

mod helpers;

use identity::config::{AppConfig, DbConfig, KafkaConfig};
use identity::db::DatabasePool;
use identity::proto::GetJwksRequest;
use identity::proto::identity_service_client::IdentityServiceClient;
use tokio::net::TcpListener;

use helpers::{
    activate_user_by_email, authenticated_request, random_user_data, register_activate_login,
    start_test_server, test_db_pool, test_kek,
};

#[tokio::test]
async fn server_starts_and_accepts_connections() {
    let (addr, _container, _pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    let response = client.get_jwks(GetJwksRequest {}).await.unwrap();
    let jwks: serde_json::Value = serde_json::from_str(&response.into_inner().keys).unwrap();
    assert_eq!(jwks["keys"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn sign_up_succeeds() {
    let (addr, _container, _pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    let user = random_user_data();
    client
        .sign_up(identity::proto::SignUpRequest {
            username: user.username,
            given_name: user.given_name,
            family_name: user.family_name,
            email: user.email,
            password: Some(user.password),
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn sign_up_duplicate_username_fails() {
    let (addr, _container, _pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    let user1 = random_user_data();
    client
        .sign_up(identity::proto::SignUpRequest {
            username: user1.username.clone(),
            given_name: user1.given_name,
            family_name: user1.family_name,
            email: user1.email,
            password: Some(user1.password),
        })
        .await
        .unwrap();

    let user2 = random_user_data();
    let err = client
        .sign_up(identity::proto::SignUpRequest {
            username: user1.username, // duplicate
            given_name: user2.given_name,
            family_name: user2.family_name,
            email: user2.email,
            password: Some(user2.password),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::AlreadyExists);
}

#[tokio::test]
async fn sign_up_validation_errors() {
    let (addr, _container, _pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    // No password
    let err = client
        .sign_up(identity::proto::SignUpRequest {
            username: "testuser".into(),
            given_name: "Test".into(),
            family_name: "User".into(),
            email: "test@example.com".into(),
            password: None,
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::InvalidArgument);

    // Short password
    let err = client
        .sign_up(identity::proto::SignUpRequest {
            username: "testuser".into(),
            given_name: "Test".into(),
            family_name: "User".into(),
            email: "test@example.com".into(),
            password: Some("short".into()),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::InvalidArgument);

    // Invalid username
    let err = client
        .sign_up(identity::proto::SignUpRequest {
            username: "bad user!".into(),
            given_name: "Test".into(),
            family_name: "User".into(),
            email: "test@example.com".into(),
            password: Some("password123".into()),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn verify_email_with_bad_token_returns_not_found() {
    let (addr, _container, _pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    let err = client
        .verify_email(identity::proto::VerifyEmailRequest {
            token: "invalid-token-not-base64url".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn resend_verification_for_unknown_email_returns_ok() {
    let (addr, _container, _pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    // Should return OK even for unknown email (prevent enumeration)
    client
        .resend_verification(identity::proto::ResendVerificationRequest {
            email: "nonexistent@example.com".into(),
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn login_with_valid_credentials_succeeds() {
    let (addr, _container, pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    let user = random_user_data();
    let token = register_activate_login(&mut client, &pool, &user).await;
    assert!(!token.is_empty());
}

#[tokio::test]
async fn login_with_wrong_password_fails() {
    let (addr, _container, pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    let user = random_user_data();
    client
        .sign_up(identity::proto::SignUpRequest {
            username: user.username,
            given_name: user.given_name,
            family_name: user.family_name,
            email: user.email.clone(),
            password: Some(user.password),
        })
        .await
        .unwrap();
    activate_user_by_email(&pool, &user.email).await;

    let err = client
        .login(identity::proto::LoginRequest {
            email: user.email,
            password: "wrong-password".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::Unauthenticated);
}

#[tokio::test]
async fn login_with_unknown_email_fails() {
    let (addr, _container, _pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    let err = client
        .login(identity::proto::LoginRequest {
            email: "nonexistent@example.com".into(),
            password: "password123".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::Unauthenticated);
}

#[tokio::test]
async fn login_with_pending_verification_fails() {
    let (addr, _container, _pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    let user = random_user_data();
    client
        .sign_up(identity::proto::SignUpRequest {
            username: user.username,
            given_name: user.given_name,
            family_name: user.family_name,
            email: user.email.clone(),
            password: Some(user.password.clone()),
        })
        .await
        .unwrap();

    let err = client
        .login(identity::proto::LoginRequest {
            email: user.email,
            password: user.password,
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::PermissionDenied);
}

// --- UserInfo tests ---

#[tokio::test]
async fn user_info_returns_profile() {
    let (addr, _container, pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    let user = random_user_data();
    let token = register_activate_login(&mut client, &pool, &user).await;

    let response = client
        .user_info(authenticated_request(
            identity::proto::UserInfoRequest {},
            &token,
        ))
        .await
        .unwrap();

    let info = response.into_inner();
    assert_eq!(info.username, user.username);
    assert_eq!(info.given_name, user.given_name);
    assert_eq!(info.family_name, user.family_name);
    assert_eq!(info.email, user.email);
    assert!(info.email_verified);
    assert!(!info.sub.is_empty());
}

#[tokio::test]
async fn user_info_without_auth_fails() {
    let (addr, _container, _pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    let err = client
        .user_info(identity::proto::UserInfoRequest {})
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::Unauthenticated);
}

// --- UpdateProfile tests ---

#[tokio::test]
async fn update_profile_changes_username() {
    let (addr, _container, pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    let user = random_user_data();
    let token = register_activate_login(&mut client, &pool, &user).await;

    let new_username = format!("new-{}", &user.username[..8]);
    client
        .update_profile(authenticated_request(
            identity::proto::UpdateProfileRequest {
                given_name: None,
                family_name: None,
                username: Some(new_username.clone()),
            },
            &token,
        ))
        .await
        .unwrap();

    // Verify via UserInfo
    let info = client
        .user_info(authenticated_request(
            identity::proto::UserInfoRequest {},
            &token,
        ))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(info.username, new_username);
}

#[tokio::test]
async fn update_profile_changes_names() {
    let (addr, _container, pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    let user = random_user_data();
    let token = register_activate_login(&mut client, &pool, &user).await;

    client
        .update_profile(authenticated_request(
            identity::proto::UpdateProfileRequest {
                given_name: Some("NewFirst".into()),
                family_name: Some("NewLast".into()),
                username: None,
            },
            &token,
        ))
        .await
        .unwrap();

    let info = client
        .user_info(authenticated_request(
            identity::proto::UserInfoRequest {},
            &token,
        ))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(info.given_name, "NewFirst");
    assert_eq!(info.family_name, "NewLast");
}

#[tokio::test]
async fn update_profile_duplicate_username_fails() {
    let (addr, _container, pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    let user1 = random_user_data();
    let _token1 = register_activate_login(&mut client, &pool, &user1).await;

    let user2 = random_user_data();
    let token2 = register_activate_login(&mut client, &pool, &user2).await;

    // user2 tries to take user1's username
    let err = client
        .update_profile(authenticated_request(
            identity::proto::UpdateProfileRequest {
                given_name: None,
                family_name: None,
                username: Some(user1.username),
            },
            &token2,
        ))
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::AlreadyExists);
}

// --- UpdatePassword tests ---

#[tokio::test]
async fn update_password_succeeds() {
    let (addr, _container, pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    let user = random_user_data();
    let token = register_activate_login(&mut client, &pool, &user).await;

    let new_password = "new-secure-password-123";

    client
        .update_password(authenticated_request(
            identity::proto::UpdatePasswordRequest {
                current_password: user.password.clone(),
                new_password: new_password.into(),
            },
            &token,
        ))
        .await
        .unwrap();

    // Verify login with new password works
    let response = client
        .login(identity::proto::LoginRequest {
            email: user.email,
            password: new_password.into(),
        })
        .await
        .unwrap();
    assert!(!response.into_inner().access_token.is_empty());
}

#[tokio::test]
async fn update_password_wrong_current_fails() {
    let (addr, _container, pool) = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    let user = random_user_data();
    let token = register_activate_login(&mut client, &pool, &user).await;

    let err = client
        .update_password(authenticated_request(
            identity::proto::UpdatePasswordRequest {
                current_password: "wrong-password".into(),
                new_password: "new-secure-password-123".into(),
            },
            &token,
        ))
        .await
        .unwrap_err();
    assert_eq!(err.code(), tonic::Code::Unauthenticated);
}

// --- TLS test ---

#[tokio::test]
async fn server_accepts_tls_connections() {
    use rcgen::generate_simple_self_signed;
    use std::io::Write;
    use tonic::transport::{Certificate, Channel, ClientTlsConfig};

    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let (_container, pool) = test_db_pool().await;

    let kek = test_kek();
    identity::services::ensure_signing_key(&pool, &kek)
        .await
        .unwrap();

    let subject_alt_names = vec!["::1".into(), "localhost".into()];
    let certified_key = generate_simple_self_signed(subject_alt_names).unwrap();

    let cert_pem = certified_key.cert.pem();
    let key_pem = certified_key.signing_key.serialize_pem();

    let dir = tempfile::tempdir().unwrap();
    let cert_path = dir.path().join("tls.crt");
    let key_path = dir.path().join("tls.key");

    std::fs::File::create(&cert_path)
        .unwrap()
        .write_all(cert_pem.as_bytes())
        .unwrap();
    std::fs::File::create(&key_path)
        .unwrap()
        .write_all(key_pem.as_bytes())
        .unwrap();

    let listener = TcpListener::bind("[::1]:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let config = AppConfig {
        listen_addr: addr,
        tls_cert_file: cert_path.to_str().unwrap().into(),
        tls_key_file: key_path.to_str().unwrap().into(),
        web_base_url: "https://test.example.com".into(),
        db: DbConfig {
            host: String::new(),
            port: 5432,
            database: String::new(),
            username: String::new(),
            password: None,
            ssl_mode: "disable".into(),
            ssl_root_cert: String::new(),
            ssl_client_cert: String::new(),
            ssl_client_key: String::new(),
            max_connections: 2,
            min_connections: 1,
        },
        db_read: None,
        kafka: KafkaConfig::default(),
    };
    let db = DatabasePool::from_pool(pool);

    tokio::spawn(async move {
        identity::server::run(listener, &config, db, kek)
            .await
            .unwrap();
    });
    tokio::task::yield_now().await;

    let tls_config = ClientTlsConfig::new()
        .ca_certificate(Certificate::from_pem(&cert_pem))
        .domain_name("::1");

    let channel = Channel::from_shared(format!("https://[::1]:{}", addr.port()))
        .unwrap()
        .tls_config(tls_config)
        .unwrap()
        .connect()
        .await
        .unwrap();

    let mut client = IdentityServiceClient::new(channel);

    let response = client.get_jwks(GetJwksRequest {}).await.unwrap();
    let jwks: serde_json::Value = serde_json::from_str(&response.into_inner().keys).unwrap();
    assert_eq!(jwks["keys"].as_array().unwrap().len(), 1);
}
