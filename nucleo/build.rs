fn main() {
    let raiz = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("none") {
        println!(
            "cargo:rustc-link-arg-bin=nucleo=-T{}",
            raiz.join("linker.ld").display()
        );
    }
    println!("cargo:rerun-if-changed=linker.ld");
    for nome in ["RAIZ_TERMINAL", "RAIZ_REDE", "RAIZ_SUPERVISOR"] {
        println!("cargo:rerun-if-env-changed={nome}");
        if let Some(p) = std::env::var_os(nome) {
            println!(
                "cargo:rerun-if-changed={}",
                std::path::Path::new(&p).display()
            );
        }
    }
}
