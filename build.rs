fn main() {
    let out = std::env::var("OUT_DIR").unwrap();
    std::fs::copy("memory.x", format!("{out}/memory.x")).unwrap();
    println!("cargo:rustc-link-search={out}");
    println!("cargo:rustc-link-arg=-Tembedded-test.x");
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=build.rs");
}
