fn main() {
    let out = std::env::var("OUT_DIR").unwrap();
    std::fs::copy("memory.x", format!("{out}/memory.x")).unwrap();
    println!("cargo:rustc-link-search={out}");
    // Only emit the embedded-test linker script when running `cargo test`
    // (CARGO_CFG_TEST is set by cargo for test artifacts).
    if std::env::var_os("CARGO_CFG_TEST").is_some() {
        println!("cargo:rustc-link-arg=-Tembedded-test.x");
    }
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=build.rs");
}
