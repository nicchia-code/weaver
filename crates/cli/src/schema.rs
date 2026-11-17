//! JSON Schemas (draft 2020-12) of the input and output of a command, and of its command line,
//! made from the fields of the registry. `--help --json` prints them as
//! `{command, input, output}`.

use serde_json::{json, Map, Value};

use crate::spec::{Arg, Command, Field, StrRule, Ty};

pub const DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";

fn string_schema(rule: &StrRule) -> Map<String, Value> {
    let mut schema = Map::new();
    schema.insert("type".into(), json!("string"));
    if let Some(max) = rule.max {
        schema.insert("maxLength".into(), json!(max));
    }
    let pattern = match (rule.nonblank, rule.line) {
        (true, true) => Some(r"^[^\r\n]*\S[^\r\n]*$"),
        (true, false) => Some(r"\S"),
        (false, true) => Some(r"^[^\r\n]*$"),
        (false, false) => None,
    };
    if let Some(pattern) = pattern {
        schema.insert("pattern".into(), json!(pattern));
    }
    schema
}

/// The schema of one type.
pub fn type_schema(ty: &Ty) -> Value {
    match ty {
        Ty::Str(rule) => Value::Object(string_schema(rule)),
        Ty::Int { min, max } => {
            let mut schema = Map::new();
            schema.insert("type".into(), json!("integer"));
            if let Some(min) = min {
                schema.insert("minimum".into(), json!(min));
            }
            if let Some(max) = max {
                schema.insert("maximum".into(), json!(max));
            }
            Value::Object(schema)
        }
        Ty::Bool => json!({ "type": "boolean" }),
        Ty::Enum(values) => json!({ "type": "string", "enum": values }),
        Ty::List { item, min, max } => {
            let mut schema = Map::new();
            schema.insert("type".into(), json!("array"));
            schema.insert("items".into(), type_schema(item));
            if *min > 0 {
                schema.insert("minItems".into(), json!(min));
            }
            if let Some(max) = max {
                schema.insert("maxItems".into(), json!(max));
            }
            Value::Object(schema)
        }
        Ty::Object(fields) => Value::Object(properties(fields, true)),
        Ty::Nullable(inner) => json!({ "anyOf": [type_schema(inner), { "type": "null" }] }),
        Ty::Any => json!({}),
    }
}

/// `type`, `properties`, `required` and, for an input, `additionalProperties: false`.
fn properties(fields: &[Field], closed: bool) -> Map<String, Value> {
    let mut schema = Map::new();
    schema.insert("type".into(), json!("object"));
    let mut props = Map::new();
    for field in fields {
        let mut one = match type_schema(&field.ty) {
            Value::Object(map) => map,
            other => {
                let mut map = Map::new();
                map.insert("allOf".into(), json!([other]));
                map
            }
        };
        if !field.doc.is_empty() {
            one.insert("description".into(), json!(field.doc));
        }
        props.insert(field.name.to_owned(), Value::Object(one));
    }
    schema.insert("properties".into(), Value::Object(props));
    let required: Vec<&str> = fields
        .iter()
        .filter(|f| f.required)
        .map(|f| f.name)
        .collect();
    if !required.is_empty() {
        schema.insert("required".into(), json!(required));
    }
    if closed {
        schema.insert("additionalProperties".into(), json!(false));
    }
    schema
}

fn root(title: String, description: &str, mut body: Map<String, Value>) -> Value {
    let mut schema = Map::new();
    schema.insert("$schema".into(), json!(DIALECT));
    schema.insert("title".into(), json!(title));
    if !description.is_empty() {
        schema.insert("description".into(), json!(description));
    }
    schema.append(&mut body);
    Value::Object(schema)
}

/// The schema of the input object.
pub fn input_schema(command: &Command) -> Value {
    root(
        format!("weaver {} input", command.name()),
        "",
        properties(&command.input, true),
    )
}

/// The schema of the `--json` output.
pub fn output_schema(command: &Command) -> Value {
    root(
        format!("weaver {} output", command.name()),
        "",
        properties(&command.output, false),
    )
}

/// The schema of the command line: the words and the argument of `weaver <object> <verb> ...`.
pub fn command_schema(command: &Command) -> Value {
    let mut props = Map::new();
    props.insert("object".into(), json!({ "const": command.object }));
    let mut required = vec!["object"];
    if !command.is_verbless() {
        props.insert("verb".into(), json!({ "const": command.verb }));
        required.push("verb");
    }
    match command.arg {
        Arg::None => {}
        Arg::Card(_) => {
            props.insert(
                "id".into(),
                json!({ "type": "string", "description": "Identity, unique identity prefix or short number of the card." }),
            );
            required.push("id");
        }
        Arg::Reference => {
            props.insert(
                "ref".into(),
                json!({ "type": "string", "description": "A Model or Requirement reference." }),
            );
        }
        Arg::Job => {
            props.insert(
                "id".into(),
                json!({ "type": "string", "description": "A job identity." }),
            );
        }
        Arg::Folder => {
            props.insert(
                "folder".into(),
                json!({ "type": "string", "description": "The folder to open, joined to the current folder. Without it, the welcome window is shown." }),
            );
        }
    }
    if !command.states.is_empty() {
        props.insert(
            "state".into(),
            json!({ "type": "string", "enum": command.states }),
        );
        required.push("state");
    }
    let mut body = Map::new();
    body.insert("type".into(), json!("object"));
    body.insert("properties".into(), Value::Object(props));
    body.insert("required".into(), json!(required));
    body.insert("additionalProperties".into(), json!(false));
    if !command.example.is_empty() {
        body.insert("examples".into(), json!([command.example]));
    }
    root(command.usage(), command.summary, body)
}

/// `{command, input, output}`, the whole of `--help --json`.
pub fn help_json(command: &Command) -> Value {
    json!({
        "command": command_schema(command),
        "input": input_schema(command),
        "output": output_schema(command),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::*;

    fn sample() -> Command {
        Command::new("task", "move", "Moves a Task.", not_implemented)
            .arg(Arg::Card(weaver_core::project::cards::CardKind::Task))
            .states(&["doing", "todo", "done"])
            .input(vec![
                opt("unchanged", nonblank(), "Closes a fix Task."),
                req("n", list_between(int_min(1), 1, 3), ""),
            ])
            .output(vec![
                req("id", line(), "The identity."),
                req("number", nullable(int_min(1)), "The short number."),
            ])
            .example("weaver task move t3 done")
    }

    #[test]
    fn the_three_schemas_declare_2020_12() {
        let help = help_json(&sample());
        for key in ["command", "input", "output"] {
            assert_eq!(help[key]["$schema"], DIALECT, "{key}");
        }
        assert_eq!(help.as_object().unwrap().len(), 3);
    }

    #[test]
    fn the_input_schema_is_closed_and_lists_required_fields_and_limits() {
        let schema = input_schema(&sample());
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["required"], json!(["n"]));
        assert_eq!(schema["properties"]["unchanged"]["type"], "string");
        assert_eq!(schema["properties"]["unchanged"]["pattern"], r"\S");
        assert_eq!(
            schema["properties"]["unchanged"]["description"],
            "Closes a fix Task."
        );
        assert_eq!(
            schema["properties"]["n"],
            json!({ "type": "array", "items": { "type": "integer", "minimum": 1 }, "minItems": 1, "maxItems": 3 })
        );
    }

    #[test]
    fn the_output_schema_allows_null_numbers_and_is_open() {
        let schema = output_schema(&sample());
        assert!(schema.get("additionalProperties").is_none());
        assert_eq!(
            schema["properties"]["number"]["anyOf"][1],
            json!({ "type": "null" })
        );
    }

    #[test]
    fn the_command_schema_names_the_words_the_argument_and_the_states() {
        let schema = command_schema(&sample());
        assert_eq!(schema["properties"]["object"]["const"], "task");
        assert_eq!(
            schema["properties"]["state"]["enum"],
            json!(["doing", "todo", "done"])
        );
        assert_eq!(schema["required"], json!(["object", "verb", "id", "state"]));
        assert_eq!(
            schema["title"],
            "weaver task move <id> doing|todo|done [--input <json>|-] [--root <dir>] [--json]"
        );
    }
}
