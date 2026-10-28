fn main() {
    println!("cargo:rerun-if-changed=assets/rst.ico");

    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/rst.ico");
        resource
            .compile()
            .expect("failed to embed the Windows icon");
    }
}
