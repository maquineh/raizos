fn main() {
    let manifesto = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let linker = manifesto
        .join("../linker.ld")
        .canonicalize()
        .expect("linker dos servicos");
    println!("cargo:rerun-if-changed={}", linker.display());
    println!("cargo:rustc-link-arg=-T{}", linker.display());
    println!("cargo:rustc-link-arg=--no-dynamic-linker");
    println!("cargo:rustc-link-arg=--build-id=none");
}
