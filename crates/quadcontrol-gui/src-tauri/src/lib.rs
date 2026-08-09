#[tauri::command]
async fn ping() -> Result<String, String> {
    // Tauri synchronous commands run on the main thread. Keep this async plus
    // spawn_blocking shape as the template for future Session/device work.
    tauri::async_runtime::spawn_blocking(|| {
        format!("QuadControl GUI {}", env!("CARGO_PKG_VERSION"))
    })
    .await
    .map_err(|error| format!("ping worker failed: {error}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![ping])
        .run(tauri::generate_context!())
        .expect("error while running QuadControl GUI");
}
