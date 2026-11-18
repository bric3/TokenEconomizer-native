use super::{
    Failure, Result, Runtime, Text, Value, boolean, builtin, call, copy, fallback, function,
    invoke, json, literal, push, returned, text, truthy,
};
use crate::experiments::swebench::optimizer::code_points;

fn get(rt: &mut dyn Runtime) -> Result<Value> {
    if !boolean(rt, "name")? {
        return Ok(Value::Null);
    }
    let constructed = (|| {
        function(rt, "SCAFFOLDS")?;
        rt.call("scaffold_item", json!(["function", "name", "function"]))?;
        invoke(rt, &[], &[])
    })();
    if let Err(error) = constructed {
        if !truthy(&rt.call("exception_matches", json!([error, ["KeyError"]]))?) {
            return Err(error);
        }
        rt.call("handler_exception", json!([error]))?;
        rt.call("handler_function", json!(["ScaffoldError"]))?;
        copy(rt, "error_class", "function")?;
        let mut message = Text::from("unknown scaffold ");
        builtin(rt, "repr")?;
        rt.call("handler_invoke", json!([["name"], []]))?;
        message
            .0
            .extend(code_points(rt.call("handler_format", json!(["called"]))?)?.0);
        push(&mut message, "; known: ");
        builtin(rt, "sorted")?;
        copy(rt, "sorted", "function")?;
        rt.call("handler_function", json!(["SCAFFOLDS"]))?;
        copy(rt, "registry", "function")?;
        copy(rt, "function", "sorted")?;
        rt.call("handler_invoke", json!([["registry"], []]))?;
        copy(rt, "names", "called")?;
        literal(rt, "separator", json!(", "))?;
        rt.call("handler_member", json!(["separator", "join", "function"]))?;
        rt.call("handler_invoke", json!([["names"], []]))?;
        message
            .0
            .extend(code_points(rt.call("handler_format", json!(["called"]))?)?.0);
        text(rt, "message", message)?;
        copy(rt, "function", "error_class")?;
        rt.call("handler_invoke", json!([["message"], []]))?;
        return rt.call("scaffold_raise", json!(["called", true]));
    }
    Ok(returned("called"))
}
fn for_arm(rt: &mut dyn Runtime) -> Result<Value> {
    function(rt, "get")?;
    copy(rt, "get", "function")?;
    function(rt, "ARM_SCAFFOLDS.get")?;
    copy(rt, "arm_get", "function")?;
    copy(rt, "key", "arm")?;
    fallback(rt, "key", json!(""))?;
    call(rt, "arm_get", &["key"], "name")?;
    call(rt, "get", &["name"], "scaffold")?;
    Ok(returned("scaffold"))
}
pub(super) fn dispatch(operation: &str, rt: &mut dyn Runtime) -> Result<Value> {
    match operation {
        "get" => get(rt),
        "for_arm" => for_arm(rt),
        _ => Err(Failure::new(
            "ValueError",
            format!("unknown scaffold registry operation: {operation}"),
        )),
    }
}
