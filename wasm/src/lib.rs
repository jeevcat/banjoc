// https://github.com/rustwasm/wasm-bindgen/issues/2774
#![allow(clippy::unused_unit)]

mod utils;

use banjoc::{ast::Source, error::BanjoError, output::Output, vm::Vm};
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
    let value = parse_interpret(source);
    let serializer = serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true);
    value
        .serialize(&serializer)
        .unwrap_or_else(|_| JsValue::from_str("compile error: couldn't serialize result"))
}

fn parse_interpret(source: JsValue) -> Output {
    let mut vm = Vm::new();
    let source: Source = match serde_wasm_bindgen::from_value(source) {
        Ok(source) => source,
        Err(e) => {
            return Output::from_single_error(BanjoError::Compile(format!(
                "JSON parsing error: {e}"
            )))
        }
    };
    vm.interpret(source)
}
