//! Build script for gRPC code generation.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_path = "src/grpc/mediaedge.proto";
    
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir("src/grpc/generated")
        .compile_protos(&[proto_path], &["src/grpc"])?;
    
    // Re-run if proto file changes
    println!("cargo:rerun-if-changed={}", proto_path);
    
    Ok(())
}