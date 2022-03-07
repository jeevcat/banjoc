use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    error::{Error, Result},
    value::Value,
    vm::Vm,
};

pub fn clock(_args: &[Value], _vm: &mut Vm) -> Result<Value> {
    Ok(Value::Number(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| Error::runtime(e.to_string()))?
            .as_secs_f64(),
    ))
}

pub fn sum(args: &[Value], vm: &mut Vm) -> Result<Value> {
    Ok(args
        .iter()
        .copied()
        .reduce(|accum, item| accum.add(item, vm))
        .unwrap_or(Value::Nil))
}

pub fn product(args: &[Value], _vm: &mut Vm) -> Result<Value> {
    Ok(args
        .iter()
        .copied()
        .reduce(|accum, item| {
            accum
                .binary_op(item, |a, b| Value::Number(a * b))
                .unwrap_or(accum)
        })
        .unwrap_or(Value::Nil))
}
