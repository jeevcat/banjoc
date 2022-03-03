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
pub fn interpret(source: JsValue) -> Result<JsValue, JsValue> {
    set_panic_hook();
    parse_interpret(source)
        .map_err(|e| match e {
            BanjoError::CompileError((node_id, msg)) => {
                JsValue::from_str(&format!("compile error: [{node_id}] {msg}"))
            }
            BanjoError::CompileErrors(errors) => {
                let mut s = "compile errors:".to_owned();
                for (node_id, msg) in errors {
                    s += &format!("\n[{node_id}] {msg}");
                }
                JsValue::from_str(&s)
            }
            BanjoError::RuntimeError(msg) => JsValue::from_str(&format!("runtime error: {msg}")),
        })
        .map(|value| {
            let serializer = serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true);
            value
                .serialize(&serializer)
                .unwrap_or_else(|_| JsValue::from_str("compile error: couldn't serialize result"))
        })
}

fn parse_interpret(source: JsValue) -> Result<NodeOutputs, BanjoError> {
    let mut vm = Vm::new();
    let source: Ast = serde_wasm_bindgen::from_value(source)
        .map_err(|e| BanjoError::compile("any", &format!("JSON parsing error: {e}")))?;
    vm.interpret(source)
}
