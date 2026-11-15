fn main() -> Result<(), fairygui_build::Error> {
    println!("cargo::rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        // Full Bevy debug symbols exceed MSVC's default PDB page capacity.
        // Keep debug information while allowing the demo to link normally.
        println!("cargo::rustc-link-arg-bin=fairygui-demo=/PDBPAGESIZE:8192");
    }
    fairygui_build::Bindings::new()
        .package("assets/ui/Basics.fui")
        .package("assets/ui/ModalWaiting.fui")
        .package("assets/ui/PullToRefresh.fui")
        .generate_to_out_dir("fairygui.rs")?;
    Ok(())
}
