use wasm_bindgen::JsValue;
use web_sys::WorkerGlobalScope;

pub async fn sleep(delay: i32) -> Result<(), JsValue> {
    use wasm_bindgen::JsCast;
    let mut cb = |resolve: js_sys::Function, _reject: js_sys::Function| {
        // if we are in a worker, use the global worker's scope
        // otherwise, use the window's scope
        if let Ok(worker_scope) = js_sys::global().dyn_into::<WorkerGlobalScope>() {
            worker_scope
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, delay)
                .expect("should register `setTimeout`");
        } else {
            web_sys::window()
                .unwrap()
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, delay)
                .expect("should register `setTimeout`");
        }
    };
    let p = js_sys::Promise::new(&mut cb);
    wasm_bindgen_futures::JsFuture::from(p).await?;
    Ok(())
}
