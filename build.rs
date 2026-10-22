// Embute o ícone do app no .exe do Windows.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        if let Err(e) = res.compile() {
            println!("cargo:warning=não foi possível embutir o ícone: {e}");
        }
    }
    println!("cargo:rerun-if-changed=assets/icon.ico");
}
