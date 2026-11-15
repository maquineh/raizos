fn main() {
    let caminho = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../linker.ld")
        .canonicalize()
        .unwrap();
    println!("cargo:rustc-link-arg=-T{}", caminho.display());
    println!("cargo:rerun-if-changed={}", caminho.display());
}
