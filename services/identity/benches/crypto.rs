use identity::crypto::jwk::{Algorithm, generate_key_pair};
use identity::crypto::jwt::{
    Audience, Claims, VerificationOptions, key_pair_from_pkcs8, sign_jwt, verify_jwt,
};
use identity::crypto::password::{hash_password, verify_password};
use identity::crypto::token::generate_verification_token;

#[divan::bench]
fn bench_hash_password(bencher: divan::Bencher) {
    bencher.bench(|| hash_password(divan::black_box("secure-password-123")));
}

#[divan::bench]
fn bench_verify_password(bencher: divan::Bencher) {
    let hash = hash_password("secure-password-123").unwrap();
    bencher.bench(|| {
        verify_password(
            divan::black_box("secure-password-123"),
            divan::black_box(&hash),
        )
    });
}

#[divan::bench(args = [Algorithm::ES256, Algorithm::ES384])]
fn bench_sign_jwt(bencher: divan::Bencher, alg: Algorithm) {
    let kp = generate_key_pair(alg).unwrap();
    let key_pair = key_pair_from_pkcs8(alg, kp.private_key_pkcs8.as_bytes()).unwrap();
    let claims = sample_claims();

    bencher.bench(|| sign_jwt(alg, &kp.kid, divan::black_box(&claims), &key_pair));
}

#[divan::bench(args = [Algorithm::ES256, Algorithm::ES384])]
fn bench_verify_jwt(bencher: divan::Bencher, alg: Algorithm) {
    let kp = generate_key_pair(alg).unwrap();
    let key_pair = key_pair_from_pkcs8(alg, kp.private_key_pkcs8.as_bytes()).unwrap();
    let claims = sample_claims();
    let encoded = sign_jwt(alg, &kp.kid, &claims, &key_pair).unwrap();

    let options = VerificationOptions {
        allowed_algorithms: &[alg],
        issuer: Some("https://home.ryanseipp.com"),
        audience: Some("https://api.home.ryanseipp.com"),
        current_time: now_secs(),
        leeway: 3600,
    };

    bencher.bench(|| verify_jwt(divan::black_box(&encoded.token), &kp.public_jwk, &options));
}

#[divan::bench(args = [Algorithm::ES256, Algorithm::ES384])]
fn bench_generate_key_pair(bencher: divan::Bencher, alg: Algorithm) {
    bencher.bench(|| generate_key_pair(alg));
}

#[divan::bench]
fn bench_generate_verification_token(bencher: divan::Bencher) {
    bencher.bench(generate_verification_token);
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn sample_claims() -> Claims {
    Claims {
        iss: Some("https://home.ryanseipp.com".into()),
        sub: Some("user-123".into()),
        aud: Some(Audience::Single("https://api.home.ryanseipp.com".into())),
        exp: Some(now_secs() + 3600),
        iat: Some(now_secs()),
        jti: Some(uuid::Uuid::new_v4().to_string()),
        ..Default::default()
    }
}

fn main() {
    divan::main();
}
