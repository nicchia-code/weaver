//! Help texts and the command reference of the skills, both made from the registry.

use crate::registry;
use crate::spec::{Access, Arg, Command, Field, Ty};

pub const SHAPE: &str =
    "weaver <object> <verb> [<id>] [<state>] [--input <json>|-] [--root <dir>] [--json]";

const OPTIONS: [(&str, &str); 4] = [
    ("--input <json>|-", "The input: one camelCase JSON object, inline or from standard input with `-` (a leading BOM is accepted)."),
    ("--root <dir>", "The folder that holds `.weaver/`; the current folder when absent, never a parent."),
    ("--json", "Print one JSON object instead of Markdown."),
    ("-h, --help", "Print help; with `--json`, the JSON Schemas of the command."),
];

const EXIT_CODES: &str =
    "Exit codes: 0 done, 1 refused or failed (nothing written), 2 wrong usage or invalid input.";

/// How a type reads in help: `string`, `integer >= 1`, `list of string (at least 2)`.
pub fn type_text(ty: &Ty) -> String {
    match ty {
        Ty::Str(rule) => {
            let mut parts: Vec<String> = Vec::new();
            if rule.nonblank {
                parts.push("not blank".to_owned());
            }
            if rule.line {
                parts.push("one line".to_owned());
            }
            if let Some(max) = rule.max {
                parts.push(format!("at most {max} characters"));
            }
            if parts.is_empty() {
                "string".to_owned()
            } else {
                format!("string ({})", parts.join(", "))
            }
        }
        Ty::Int { min, max } => match (min, max) {
            (Some(min), Some(max)) => format!("integer {min}..{max}"),
            (Some(min), None) => format!("integer >= {min}"),
            (None, Some(max)) => format!("integer <= {max}"),
            (None, None) => "integer".to_owned(),
        },
        Ty::Bool => "boolean".to_owned(),
        Ty::Enum(values) => values
            .iter()
            .map(|v| format!("`{v}`"))
            .collect::<Vec<_>>()
            .join(" | "),
        Ty::List { item, min, max } => {
            let mut text = format!("list of {}", type_text(item));
            match (*min, max) {
                (0, None) => {}
                (min, None) => text.push_str(&format!(" (at least {min})")),
                (min, Some(max)) => text.push_str(&format!(" ({min} to {max})")),
            }
            text
        }
        Ty::Object(_) => "object".to_owned(),
        Ty::Nullable(inner) => format!("{} or null", type_text(inner)),
        Ty::Any => "any JSON value".to_owned(),
    }
}

/// The fields as indented lines; the fields of an object (or of the objects of a list) follow
/// their field, one level deeper.
fn field_lines(fields: &[Field], depth: usize, out: &mut Vec<String>) {
    let pad = "  ".repeat(depth);
    for field in fields {
        let need = if field.required {
            "required"
        } else {
            "optional"
        };
        let doc = if field.doc.is_empty() {
            String::new()
        } else {
            format!(": {}", field.doc)
        };
        out.push(format!(
            "{pad}- `{}`, {}, {need}{doc}",
            field.name,
            type_text(&field.ty)
        ));
        let mut inner = &field.ty;
        loop {
            match inner {
                Ty::List { item, .. } => inner = item,
                Ty::Nullable(item) => inner = item,
                _ => break,
            }
        }
        if let Ty::Object(nested) = inner {
            field_lines(nested, depth + 1, out);
        }
    }
}

fn access_text(command: &Command) -> Option<&'static str> {
    match command.access {
        Access::Anyone => None,
        Access::Job => Some(
            "Only the agent of a job Weaver started can run it (Weaver sets `WEAVER_JOB` in the environment of that agent). Any other caller is refused with exit 1 and nothing is written.",
        ),
        Access::JobOrUnavailable => Some(
            "Only the agent of a job Weaver started or the agent of the project's Chat during a turn can use it (Weaver sets `WEAVER_JOB` or `WEAVER_GENERAL` in the environment of that agent). For any other caller the outcome is `unavailable`, with exit 0.",
        ),
        Access::General => Some(
            "Only the agent of the project's General or of an Ask more conversation that Weaver started can run it (Weaver sets `WEAVER_GENERAL` or `WEAVER_ASK` in the environment of that agent). Any other caller is refused with exit 1 and nothing is written.",
        ),
        Access::Chat => Some(
            "Only the agent of the project's Chat during one of its turns can run it (Weaver sets `WEAVER_GENERAL` in the environment of that agent). Any other caller, an Ask more conversation and a job included, is refused with exit 1 and nothing is written.",
        ),
    }
}

fn argument_lines(command: &Command) -> Vec<String> {
    let mut lines = Vec::new();
    match command.arg {
        Arg::None => {}
        Arg::Card(_) => lines.push("`<id>`: the card, by identity, unique identity prefix or short number (`P3`, `t3`: a letter and digits is always a short number, and its letter must match the type).".to_owned()),
        Arg::Reference => lines.push("`<ref>`: a Model element (`order`, `order/Paid`, `user.CANCEL`, `order/Paid/user.CANCEL`), `system`, a Requirement (`requirements/<area>`, `requirements/<area>#General`, `requirements/<area>#<element>`) or the Specs (`specs`, `specs#<field>`). Without it, the System is shown.".to_owned()),
        Arg::Job => lines.push("`<id>`: a job identity. Without it, the job of the caller is shown.".to_owned()),
    }
    if !command.states.is_empty() {
        let states: Vec<String> = command.states.iter().map(|s| format!("`{s}`")).collect();
        lines.push(format!(
            "`{}`: the state to move to: {}.",
            command.states.join("|"),
            states.join(", ")
        ));
    }
    lines
}

/// `weaver --help`.
pub fn top_help() -> String {
    let mut text = String::new();
    text.push_str("weaver: the command line of Weaver. It is how the agent reaches Weaver; it also works without the app for every command not bound to a job.\n\n");
    text.push_str(&format!(
        "Usage: {SHAPE}\n       weaver <object> <verb> --help [--json]\n\n"
    ));
    text.push_str("Objects and verbs:\n");
    for (object, commands) in registry::objects() {
        let verbs: Vec<&str> = commands.iter().map(|c| c.verb).collect();
        text.push_str(&format!("  {object:<14}{}\n", verbs.join(", ")));
    }
    text.push_str("\nOptions:\n");
    for (name, doc) in OPTIONS {
        text.push_str(&format!("  {name:<18}{doc}\n"));
    }
    text.push_str(&format!("\n{EXIT_CODES}\n"));
    text.push_str("Run `weaver <object> <verb> --help` for the usage, the fields, an example and the output of a command.\n");
    text
}

/// `weaver <object> --help`: the verbs of one object, or `None` when there is no such object.
pub fn object_help(object: &str) -> Option<String> {
    let commands = registry::objects()
        .into_iter()
        .find(|(name, _)| *name == object)?
        .1;
    let mut text = format!("weaver {object}: ");
    let verbs: Vec<&str> = commands.iter().map(|c| c.verb).collect();
    text.push_str(&format!("{}\n\nUsage: weaver {object} <verb> [<id>] [<state>] [--input <json>|-] [--root <dir>] [--json]\n\nVerbs:\n", verbs.join(", ")));
    for command in commands {
        text.push_str(&format!("  {:<8}{}\n", command.verb, command.summary));
    }
    text.push_str(&format!(
        "\nRun `weaver {object} <verb> --help` for the fields of a verb.\n"
    ));
    Some(text)
}

/// `weaver <object> <verb> --help`.
pub fn command_help(command: &Command) -> String {
    let mut text = format!(
        "{}: {}\n\nUsage: {}\n",
        command.name_with_prefix(),
        command.summary,
        command.usage()
    );
    for note in command.notes {
        text.push_str(&format!("\n{note}\n"));
    }
    if let Some(who) = access_text(command) {
        text.push_str(&format!("\nWho: {who}\n"));
    }
    let arguments = argument_lines(command);
    if !arguments.is_empty() {
        text.push_str("\nArguments:\n");
        for line in arguments {
            text.push_str(&format!("  {line}\n"));
        }
    }
    text.push_str("\nInput (a JSON object):\n");
    if command.input.is_empty() {
        text.push_str("  none: only `{}` or no input is accepted.\n");
    } else {
        let mut lines = Vec::new();
        field_lines(&command.input, 1, &mut lines);
        for line in lines {
            text.push_str(&format!("{line}\n"));
        }
    }
    text.push_str("\nOutput (the object printed with `--json`):\n");
    if command.output.is_empty() {
        text.push_str("  an object with no fields.\n");
    } else {
        let mut lines = Vec::new();
        field_lines(&command.output, 1, &mut lines);
        for line in lines {
            text.push_str(&format!("{line}\n"));
        }
    }
    if command.json_only {
        text.push_str("\nThe output is always JSON, with or without `--json`.\n");
    }
    if !command.example.is_empty() {
        text.push_str(&format!("\nExample:\n  {}\n", command.example));
    }
    text.push_str(&format!("\n{EXIT_CODES}\n"));
    text
}

impl Command {
    /// `weaver <object> <verb>`.
    pub fn name_with_prefix(&self) -> String {
        format!("weaver {} {}", self.object, self.verb)
    }
}

// ---------------------------------------------------------------------------------------------
// The command reference of the installed skills

/// The Markdown reference of every command, installed with the `weaver-design` skill as
/// `COMMANDS.md`: LF, with a final newline. A test of this crate fails when it differs from the
/// copy embedded in `weaver-core`.
pub fn command_reference() -> String {
    let mut text = String::new();
    text.push_str("# Commands of `weaver`\n\n");
    text.push_str("Generated from the command registry of the `weaver` command line: do not edit it by hand.\n\n");
    text.push_str(&format!(
        "Shape: `{SHAPE}`. `-h` and `--help` print help (with `--json`, the JSON Schemas of the command); no other option exists. Input is one camelCase JSON object, inline or from standard input with `--input -`; a field marked optional may be left out, and a command without fields takes only `{{}}` or nothing. Cards are named by identity, unique identity prefix or short number. Output is Markdown, or one JSON object with `--json`. Exit codes: `0` done, `1` refused or failed, `2` wrong usage or invalid input.\n\n"
    ));
    text.push_str("| Object | Verbs |\n|---|---|\n");
    for (object, commands) in registry::objects() {
        let verbs: Vec<String> = commands.iter().map(|c| format!("`{}`", c.verb)).collect();
        text.push_str(&format!("| `{object}` | {} |\n", verbs.join(", ")));
    }
    for (object, commands) in registry::objects() {
        text.push_str(&format!("\n## {object}\n"));
        for command in commands {
            text.push_str(&format!(
                "\n### `{}`\n\n{}\n",
                command.usage(),
                command.summary
            ));
            for note in command.notes {
                text.push_str(&format!("\n{note}\n"));
            }
            if let Some(who) = access_text(command) {
                text.push_str(&format!("\nWho: {who}\n"));
            }
            let arguments = argument_lines(command);
            if !arguments.is_empty() {
                text.push('\n');
                for line in arguments {
                    text.push_str(&format!("- Argument {line}\n"));
                }
            }
            if !command.input.is_empty() {
                text.push_str("\nInput:\n\n");
                let mut lines = Vec::new();
                field_lines(&command.input, 0, &mut lines);
                for line in lines {
                    text.push_str(&format!("{line}\n"));
                }
            }
            if !command.output.is_empty() {
                let names: Vec<String> = command
                    .output
                    .iter()
                    .map(|f| format!("`{}`", f.name))
                    .collect();
                text.push_str(&format!(
                    "\nOutput with `--json`: {}. `--help` gives their types.\n",
                    names.join(", ")
                ));
            }
            if command.json_only {
                text.push_str("\nThe output is always JSON, with or without `--json`.\n");
            }
            if !command.example.is_empty() {
                text.push_str(&format!("\nExample: `{}`\n", command.example));
            }
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_help_lists_every_object_with_its_verbs_and_the_options() {
        let help = top_help();
        assert!(help.contains("Usage: weaver <object> <verb>"));
        for (object, commands) in registry::objects() {
            let verbs: Vec<&str> = commands.iter().map(|c| c.verb).collect();
            assert!(
                help.contains(&format!("  {object:<14}{}\n", verbs.join(", "))),
                "{object}"
            );
        }
        assert!(help.contains("-h, --help"));
        assert!(!help.contains(" I "), "no first person");
    }

    #[test]
    fn command_help_has_usage_fields_example_and_output() {
        let command = registry::find("model", "list").unwrap();
        let help = command_help(command);
        assert!(help.contains("Usage: weaver model list [--input <json>|-]"));
        assert!(help.contains("- `text`, string (not blank), optional"));
        assert!(help.contains("Example:\n  weaver model list"));
        assert!(help.contains("Output (the object printed with `--json`):"));
    }

    #[test]
    fn nested_fields_are_listed_below_their_field() {
        let command = registry::find("question", "new").unwrap();
        let help = command_help(command);
        assert!(help.contains("  - `questions`, list of object (at least 1), required"));
        assert!(help.contains("    - `options`, list of object (at least 2), required"));
        assert!(help.contains("      - `label`, string (not blank, one line), required"));
    }

    #[test]
    fn an_unknown_object_has_no_help() {
        assert!(object_help("nothing").is_none());
        assert!(object_help("task").unwrap().contains("move"));
    }
}
