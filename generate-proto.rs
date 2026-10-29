// Generate protobuf code - run with: cargo run --bin generate-proto
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_path = PathBuf::from("proto/executor.proto");
    let out_dir = PathBuf::from("proto-gen/src");

    std::fs::create_dir_all(&out_dir)?;

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir(&out_dir)
        .compile(&[proto_path], &["proto"])?;

    println!("cargo:rerun-if-changed=proto/executor.proto");
    Ok(())
}