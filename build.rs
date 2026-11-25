fn main() {
    println!("cargo:rerun-if-changed=assets/gatin.ico");
    println!("cargo:rerun-if-changed=assets/gatin.exe.manifest");

    #[cfg(windows)]
    {
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("assets/gatin.ico");
            res.set_manifest_file("assets/gatin.exe.manifest");
            res.set("ProductName", "Gatin");
            res.set("FileDescription", "Gatin Desktop Pet");
            res.set("OriginalFilename", "gatin.exe");
            res.set("InternalName", "gatin");
            res.set("CompanyName", "c0p3rn1cus (Open Source)");
            res.set("LegalCopyright", "MIT License");
            if let Err(e) = res.compile() {
                panic!("Failed to compile Windows resources: {e}");
            }
        }
    }
}
