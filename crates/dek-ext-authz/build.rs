use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup protoc via protoc-bin-vendored
    let protoc_bin = protoc_bin_vendored::protoc_bin_path()?;
    unsafe {
        env::set_var("PROTOC", protoc_bin);
    }

    let proto_file = "proto/ext_authz.proto";
    println!("cargo:rerun-if-changed={}", proto_file);

    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .compile(&[proto_file], &["proto"])?;

    Ok(())
}
