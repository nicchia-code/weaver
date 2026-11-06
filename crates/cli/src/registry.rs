//! The one command registry: parsing, dispatch, help, the JSON Schemas and the command reference
//! of the installed skills all come from it.

use std::sync::LazyLock;

use crate::commands;
use crate::spec::Command;

static REGISTRY: LazyLock<Vec<Command>> = LazyLock::new(commands::all);

/// Every command, objects in the order of `requirements/cli#General`.
pub fn all() -> &'static [Command] {
    &REGISTRY
}

/// The command `weaver <object> <verb>`.
pub fn find(object: &str, verb: &str) -> Option<&'static Command> {
    REGISTRY
        .iter()
        .find(|c| c.object == object && c.verb == verb)
}

/// The commands grouped by object, in registry order.
pub fn objects() -> Vec<(&'static str, Vec<&'static Command>)> {
    let mut grouped: Vec<(&'static str, Vec<&'static Command>)> = Vec::new();
    for command in REGISTRY.iter() {
        match grouped.last_mut() {
            Some((object, list)) if *object == command.object => list.push(command),
            _ => grouped.push((command.object, vec![command])),
        }
    }
    grouped
}

/// The Markdown command reference installed with the `weaver-design` skill (`COMMANDS.md`).
pub fn command_reference() -> String {
    crate::help::command_reference()
}

/// Where the generated reference sits in the sources: the one the skills embed.
pub fn command_reference_source() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("core")
        .join("assets")
        .join(weaver_core::agent_files::COMMAND_REFERENCE_ASSET)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The objects and verbs of `requirements/cli#General`.
    const TABLE: [(&str, &[&str]); 10] = [
        ("model", &["show", "list", "check"]),
        ("project", &["new", "edit", "show"]),
        ("proposal", &["new", "show", "list", "edit", "move"]),
        ("task", &["new", "show", "list", "move", "delete"]),
        ("drift", &["new", "show", "list", "edit", "move"]),
        ("history", &["new", "edit", "show", "list"]),
        ("announcement", &["new"]),
        ("job", &["new", "show", "list", "edit"]),
        ("question", &["new"]),
        ("request", &["new"]),
    ];

    #[test]
    fn the_registry_has_exactly_the_commands_of_the_requirements() {
        let found: Vec<(&str, Vec<&str>)> = objects()
            .into_iter()
            .map(|(o, cs)| (o, cs.into_iter().map(|c| c.verb).collect()))
            .collect();
        let expected: Vec<(&str, Vec<&str>)> =
            TABLE.iter().map(|(o, vs)| (*o, vs.to_vec())).collect();
        assert_eq!(found, expected);
    }

    #[test]
    fn every_command_has_a_summary_an_example_and_output_without_first_person() {
        for command in all() {
            let name = command.name();
            assert!(!command.summary.is_empty(), "{name}");
            assert!(
                command
                    .example
                    .starts_with(&format!("weaver {} {}", command.object, command.verb)),
                "{name}: example"
            );
            assert!(!command.output.is_empty(), "{name}: output fields");
            let mut texts = vec![command.summary.to_owned()];
            texts.extend(command.notes.iter().map(|n| (*n).to_owned()));
            for text in texts {
                for word in text.split(|c: char| !c.is_alphanumeric() && c != '\'') {
                    assert!(
                        !["I", "we", "We", "my", "our", "My", "Our", "I'm", "I'll"].contains(&word),
                        "{name}: first person in `{text}`"
                    );
                }
            }
        }
    }

    #[test]
    fn decision_commands_tell_the_agent_to_wait_for_the_developer() {
        for (object, verb) in [("proposal", "move"), ("drift", "move"), ("task", "delete")] {
            let notes = find(object, verb).unwrap().notes.join(" ");
            assert!(notes.contains("explicitly asked"), "{object} {verb}");
        }
        assert!(find("job", "new")
            .unwrap()
            .notes
            .join(" ")
            .contains("explicitly asked"));
    }

    /// The reference in the skills is the registry's: edit the commands, then regenerate with
    /// `cargo run -p weaver-cli --example write_command_reference` and build again.
    #[test]
    fn the_command_reference_of_the_skills_is_the_one_the_registry_renders() {
        let rendered = command_reference();
        assert!(rendered.ends_with('\n') && !rendered.contains('\r'));
        assert_eq!(
            rendered,
            weaver_core::agent_files::command_reference(),
            "COMMANDS.md differs from the registry: run `cargo run -p weaver-cli --example write_command_reference`, then build again"
        );
    }

    #[test]
    fn the_source_of_the_reference_is_where_the_core_embeds_it() {
        assert!(command_reference_source().parent().unwrap().is_dir());
    }
}
