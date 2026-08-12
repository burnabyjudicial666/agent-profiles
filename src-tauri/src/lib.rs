mod paths;
mod platform;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let menu = tauri::menu::MenuBuilder::new(app)
                .text("about", "Claude Profiles")
                .build()?;
            let icon =
                tauri::image::Image::from_bytes(include_bytes!("../icons/tray/tray-icon.png"))?;

            let mut tray = tauri::tray::TrayIconBuilder::with_id("main")
                .icon(icon)
                .menu(&menu)
                .tooltip("Claude Profiles");
            #[cfg(target_os = "macos")]
            {
                tray = tray.icon_as_template(true);
            }
            tray.build(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
