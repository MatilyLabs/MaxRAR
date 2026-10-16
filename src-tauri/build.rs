fn main() {
    println!("cargo:rerun-if-env-changed=MAXRAR_UPDATER_PUBLIC_KEY");
    tauri_build::build()
}
