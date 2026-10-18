fn main() -> std::io::Result<()> {
    println!("cargo:rerun-if-changed=clin.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("clin.ico")
            .compile_for("clin")?;
    }
    Ok(())
}
