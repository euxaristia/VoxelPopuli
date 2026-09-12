//! Browser lifecycle and durable world storage.
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/web/runtime.js")]
extern "C" {
    #[wasm_bindgen(catch, js_name = nextFrame)]
    async fn frame() -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = loadSave)]
    async fn read_save() -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = storeSave)]
    async fn store_save(bytes: &[u8]) -> Result<JsValue, JsValue>;
    pub fn ready();
    #[wasm_bindgen(js_name = hasStarted)]
    pub fn has_started() -> bool;
    pub fn finished();
    pub fn error(message: &str);
    #[wasm_bindgen(js_name = saveStatus)]
    fn save_status(message: &str);
    #[wasm_bindgen(js_name = toggleFullscreen)]
    pub fn toggle_fullscreen();
    #[wasm_bindgen(js_name = takeSaveRequest)]
    pub fn take_save_request() -> bool;
}

pub async fn next_frame() {
    let _ = frame().await;
}

fn storage_error(error: JsValue) -> std::io::Error {
    std::io::Error::other(format!("Browser storage failed: {error:?}"))
}

pub async fn load_save() -> std::io::Result<crate::save::GameSave> {
    let bytes = read_save().await.map_err(storage_error)?;
    if bytes.is_undefined() || bytes.is_null() {
        return Err(std::io::ErrorKind::NotFound.into());
    }
    crate::save::GameSave::decode(&js_sys::Uint8Array::new(&bytes).to_vec())
}

pub async fn save_result(result: std::io::Result<crate::save::GameSave>) -> bool {
    let result = match result.and_then(|save| save.encode()) {
        Ok(bytes) => store_save(&bytes).await.map(|_| ()).map_err(storage_error),
        Err(error) => Err(error),
    };
    match result {
        Ok(()) => {
            save_status("World saved in this browser");
            true
        }
        Err(error) => {
            save_status(&format!(
                "Save failed: {error}. Keep this tab open and try saving again."
            ));
            false
        }
    }
}
