//! Windows: put the OneClick icon and the version info inside store.exe.
//! Without this, Explorer shows a blank page icon and "Unknown publisher"
//! in the file properties (the signature is a separate problem — see
//! docs/OPS-003-windows-startup.md).

fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/oneclick.ico");
        res.set("ProductName", "OneClick");
        res.set("FileDescription", "OneClick — caisse et gestion de magasin");
        res.set("ProductVersion", env!("CARGO_PKG_VERSION"));
        res.set("FileVersion", env!("CARGO_PKG_VERSION"));
        res.set("LegalCopyright", "OneClick");
        res.set("OriginalFilename", "store.exe");
        res.set("CompanyName", "OneClick");
        if let Err(err) = res.compile() {
            // Never fail a release build over a decoration, but say it loudly.
            println!("cargo:warning=icône Windows non appliquée : {err}");
        }
    }
}
