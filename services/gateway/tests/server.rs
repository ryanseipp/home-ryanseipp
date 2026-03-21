use std::net::SocketAddr;

use gateway::config::{AppConfig, IdentityConfig};
use gateway::server::run;
use tokio::net::TcpListener;
use tokio::task;

fn test_config(addr: SocketAddr) -> AppConfig {
    AppConfig {
        listen_addr: addr,
        tls_cert_file: "/nonexistent/tls.crt".into(),
        tls_key_file: "/nonexistent/tls.key".into(),
        identity: IdentityConfig::default(),
    }
}

async fn start_test_server() -> SocketAddr {
    let listener = TcpListener::bind("[::1]:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let config = test_config(addr);

    tokio::spawn(async move {
        run(listener, &config, None).await.unwrap();
    });

    task::yield_now().await;
    addr
}

#[tokio::test]
async fn health_check_returns_ok() {
    let addr = start_test_server().await;

    let resp = reqwest::get(format!("http://[::1]:{}/healthz", addr.port()))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn server_accepts_tls_connections() {
    use rcgen::generate_simple_self_signed;
    use std::io::Write;

    // Ensure aws-lc-rs is installed as the rustls crypto provider for the
    // test client. The server side handles this via its own rustls config,
    // but reqwest needs it explicitly when rustls has no default provider.
    use rustls::crypto::aws_lc_rs::default_provider;
    let _ = default_provider().install_default();

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
        identity: IdentityConfig::default(),
    };

    tokio::spawn(async move {
        run(listener, &config, None).await.unwrap();
    });
    task::yield_now().await;

    let cert = reqwest::Certificate::from_pem(cert_pem.as_bytes()).unwrap();
    let client = reqwest::ClientBuilder::new()
        .add_root_certificate(cert)
        .build()
        .unwrap();

    let resp = client
        .get(format!("https://[::1]:{}/healthz", addr.port()))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}
