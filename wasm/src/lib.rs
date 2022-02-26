// https://github.com/rustwasm/wasm-bindgen/issues/2774
#![allow(clippy::unused_unit)]

mod utils;

use banjoc::{
    ast::Ast,
    error::BanjoError,
    vm::{NodeOutputs, Vm},
};
use serde::Serialize;
use utils::set_panic_hook;
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen(catch)]
pub fn interpret(source: JsValue) -> JsValue {
    set_panic_hook();
    match parse_interpret(source) {
        Err(e) => match e {
            BanjoError::CompileError(msg) => JsValue::from_str(&format!("compile error: {msg}")),
            BanjoError::CompileErrors(msg) => {
                JsValue::from_str(&format!("compiler errors:\n{}", msg.join("\n")))
            }
            BanjoError::RuntimeError(msg) => JsValue::from_str(&format!("runtime error: {msg}")),
        },
        Ok(value) => {
            let serializer = serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true);
            value
                .serialize(&serializer)
                .unwrap_or_else(|_| JsValue::from_str("compile error: couldn't serialize result"))
        }
    }
}

fn parse_interpret(source: JsValue) -> Result<NodeOutputs, BanjoError> {
    let mut vm = Vm::new();
    let ast: Ast = serde_wasm_bindgen::from_value(source)
        .map_err(|e| BanjoError::compile(&format!("JSON parsing error: {e}")))?;
    vm.interpret(ast)
}
