fn main() {
    println!("cargo:rerun-if-env-changed=STICKYNOTE_GITHUB_CLIENT_ID");
    println!("cargo:rerun-if-env-changed=STICKYNOTE_GITHUB_APP_SLUG");
    println!("cargo:rerun-if-changed=github-app.json");
    tauri_build::build()
}
