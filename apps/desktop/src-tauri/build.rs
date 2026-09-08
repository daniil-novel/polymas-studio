fn main() {
    #[cfg(windows)]
    {
        // lib test executables do not receive Tauri's binary resource manifest.
        // The same v6 dependency must apply to all link targets using its dialogs.
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }
    #[cfg(windows)]
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest())
            .app_manifest(tauri_build::AppManifest::new().commands(&[
                "api_request",
                "desktop_status",
                "desktop_reconnect",
                "desktop_close",
                "desktop_cancel_close",
                "desktop_reset_close",
            ])),
    )
    .expect("Tauri build configuration must be valid");
}
