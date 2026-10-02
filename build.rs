fn main() {
    // Ship our own protoc so `cargo build` needs no system prerequisite.
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc");
    // SAFETY: single-threaded build script, before any other thread starts.
    unsafe { std::env::set_var("PROTOC", protoc) };

    println!("cargo:rerun-if-changed=proto");

    tonic_prost_build::configure()
        .build_client(false)
        .include_file("_includes.rs")
        .compile_protos(
            &["proto/envoy/service/auth/v3/external_auth.proto"],
            &["proto"],
        )
        .expect("compile extauthz protos");
}
