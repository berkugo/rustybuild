use std::path::Path;

fn main() {
    // Generate icons/icon.ico from 32x32.png if missing (required by tauri-build on Windows)
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let icons_dir = Path::new(&manifest_dir).join("icons");
    let icon_ico = icons_dir.join("icon.ico");
    let icon_png = icons_dir.join("32x32.png");

    if !icon_ico.exists() && icon_png.exists() {
        if let Ok(img) = image::open(&icon_png) {
            let _ = std::fs::create_dir_all(&icons_dir);
            if img.save(&icon_ico).is_ok() {
                println!("cargo:warning=Generated missing icons/icon.ico from 32x32.png");
            }
        }
    }

    tauri_build::build()
}
