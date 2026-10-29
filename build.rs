fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=locales");
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile_for("assets/app.rc", ["photo-frame"], embed_resource::NONE)
            .manifest_required()
            .expect("Unable to embed the PhotoFrame application icon");
    }
}
