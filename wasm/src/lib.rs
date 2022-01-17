mod utils;

use banjoc::{error::LoxError, value::Value, vm::Vm};
use utils::set_panic_hook;
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen(catch)]
pub fn interpret(source: &str) -> Result<JsValue, JsValue> {
    set_panic_hook();
    let mut vm = Vm::new();
    vm.interpret(source)
        .map_err(|e| match e {
            LoxError::CompileError(msg) => JsValue::from_str(&format!("compile error: {msg}")),
            LoxError::CompileErrors(msg) => {
                JsValue::from_str(&format!("compiler errors:\n{}", msg.join("\n")))
            }
            LoxError::RuntimeError => JsValue::from_str("runtime error"),
        })
        .map(|v| match v {
            Value::Bool(b) => match b {
                true => JsValue::TRUE,
                false => JsValue::FALSE,
            },
            Value::Nil => JsValue::NULL,
            Value::Number(n) => JsValue::from_f64(n),
            Value::String(s) => JsValue::from_str(s.as_str()),
            _ => JsValue::from_str(&format!("{v}")),
        })
}
