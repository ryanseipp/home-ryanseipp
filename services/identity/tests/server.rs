use std::net::SocketAddr;

use identity::config::AppConfig;
use identity::proto::GetJwksRequest;
use identity::proto::identity_service_client::IdentityServiceClient;
use tokio::net::TcpListener;

fn test_config(addr: SocketAddr) -> AppConfig {
    AppConfig {
        listen_addr: addr,
        // Point at nonexistent paths so tls_available() returns false
        tls_cert_file: "/nonexistent/tls.crt".into(),
        tls_key_file: "/nonexistent/tls.key".into(),
    }
}

async fn start_test_server() -> SocketAddr {
    let listener = TcpListener::bind("[::1]:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let config = test_config(addr);

    tokio::spawn(async move {
        identity::server::run(listener, &config).await.unwrap();
    });

    // Yield to let the server start accepting connections
    tokio::task::yield_now().await;

    addr
}

#[tokio::test]
async fn server_starts_and_accepts_connections() {
    let addr = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    // All RPCs should return UNIMPLEMENTED
    let err = client.get_jwks(GetJwksRequest {}).await.unwrap_err();
    assert_eq!(err.code(), tonic::Code::Unimplemented);
}

#[tokio::test]
async fn all_rpcs_return_unimplemented() {
    let addr = start_test_server().await;

    let mut client = IdentityServiceClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .unwrap();

    let sign_up_err = client
        .sign_up(identity::proto::SignUpRequest {
            username: "test".into(),
            given_name: "Test".into(),
            family_name: "User".into(),
            email: "test@example.com".into(),
            password: Some("password".into()),
        })
        .await
        .unwrap_err();
    assert_eq!(sign_up_err.code(), tonic::Code::Unimplemented);

    let login_err = client
        .login(identity::proto::LoginRequest {
            username: "test".into(),
            password: "password".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(login_err.code(), tonic::Code::Unimplemented);

    let user_info_err = client
        .user_info(identity::proto::UserInfoRequest {})
        .await
        .unwrap_err();
    assert_eq!(user_info_err.code(), tonic::Code::Unimplemented);
}

#[tokio::test]
async fn server_accepts_tls_connections() {
    use rcgen::generate_simple_self_signed;
    use std::io::Write;
    use tonic::transport::{Certificate, Channel, ClientTlsConfig};

    // Generate self-signed cert for localhost/[::1]
    let subject_alt_names = vec!["::1".into(), "localhost".into()];
    let certified_key = generate_simple_self_signed(subject_alt_names).unwrap();

    let cert_pem = certified_key.cert.pem();
    let key_pem = certified_key.signing_key.serialize_pem();

    // Write to temp files
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

    // Start TLS server
    let listener = TcpListener::bind("[::1]:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let config = AppConfig {
        listen_addr: addr,
        tls_cert_file: cert_path.to_str().unwrap().into(),
        tls_key_file: key_path.to_str().unwrap().into(),
    };

    tokio::spawn(async move {
        identity::server::run(listener, &config).await.unwrap();
    });
    tokio::task::yield_now().await;

    // Connect with TLS client trusting our self-signed CA
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

    let err = client.get_jwks(GetJwksRequest {}).await.unwrap_err();
    assert_eq!(err.code(), tonic::Code::Unimplemented);
}
