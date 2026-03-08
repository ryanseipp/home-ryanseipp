fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=migrations");

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(
            &[
                "../../proto/ryanseipp/identity/v1/identity.proto",
                "../../proto/ryanseipp/events/v1/identity.proto",
                "../../proto/ryanseipp/email/v1/auth.proto",
            ],
            &["../../proto"],
        )?;
    Ok(())
}
