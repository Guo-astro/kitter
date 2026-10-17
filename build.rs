fn main() {
    println!("cargo:rerun-if-changed=assets/app-icon.ico");
    println!("cargo:rerun-if-changed=locales");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/app-icon.ico")
            .set("ProductName", "Kitter")
            .set("FileDescription", "Kitter")
            .set("LegalCopyright", "Copyright Kitter contributors")
            .compile()
            .expect("failed to embed Windows application resources");
    }
}
