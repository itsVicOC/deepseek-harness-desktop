use std::{env, path::PathBuf};

fn main() {
    tauri_build::build();

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        cc::Build::new()
            .file("native/SparkleBridgeStub.c")
            .compile("sparkle_bridge");
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let framework_dir = manifest_dir.join("native/frameworks");
    let framework = framework_dir.join("Sparkle.framework");

    if framework.exists() {
        cc::Build::new()
            .file("native/SparkleBridge.m")
            .flag("-fobjc-arc")
            .flag("-fmodules")
            .flag("-Wno-deprecated-declarations")
            .flag(&format!("-F{}", framework_dir.display()))
            .compile("sparkle_bridge");
        println!(
            "cargo:rustc-link-search=framework={}",
            framework_dir.display()
        );
        println!("cargo:rustc-link-lib=framework=Sparkle");
        println!("cargo:rustc-cfg=dsh_sparkle");
        println!("cargo:rerun-if-changed={}", framework.display());
    } else {
        cc::Build::new()
            .file("native/SparkleBridgeStub.c")
            .compile("sparkle_bridge");
        println!(
            "cargo:warning=Sparkle.framework not found; app updates are disabled in this build"
        );
    }

    println!("cargo:rustc-check-cfg=cfg(dsh_sparkle)");
    println!("cargo:rerun-if-changed=native/SparkleBridge.m");
    println!("cargo:rerun-if-changed=native/SparkleBridgeStub.c");
}
