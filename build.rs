use std::env;

fn main() {
    // Simple build script that embeds pre-built eBPF bytecode
    // The eBPF program should be built first using scripts/build-ebpf.sh
    // and placed at: target/bpfel-unknown-none/release/mefirst-ebpf
    
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=target/bpfel-unknown-none/release/mefirst-ebpf");
    
    // When building with ebpf feature, check if the eBPF binary exists
    if env::var("CARGO_FEATURE_EBPF").is_ok() {
        let profile = env::var("PROFILE").unwrap_or_else(|_| "release".to_string());
        let ebpf_path = format!("target/bpfel-unknown-none/{}/mefirst-ebpf", profile);
        
        if !std::path::Path::new(&ebpf_path).exists() {
            println!("cargo:warning=eBPF binary not found at: {}", ebpf_path);
            println!("cargo:warning=Run: ./scripts/build-ebpf.sh first");
            println!("cargo:warning=Or the binary will load eBPF from external file at runtime");
        }
    }
}
