//! The texts Weaver installs for the agent: one list, embedded in `weaver-core`, so that the
//! app and the `weaver` command line write the same bytes.
//!
//! The sources live in `crates/core/assets/` and are embedded with `include_dir`. Their
//! layout mirrors the installed one with the dots left out of the folder names:
//!
//! | Source (`crates/core/assets/`) | Installed in the project |
//! |--------------------------------|--------------------------|
//! | `skills/<name>/SKILL.md` and the reference files next to it | `.agents/skills/<name>/…` |
//! | `omp/agents/<name>.md` (sub-agent definitions) | `.omp/agents/<name>.md` |
//! | `omp/WATCHDOG.yml` (instructions of the advisor of the Advisor Scanner) | `.omp/WATCHDOG.yml` |
//!
//! The generated `weaver-req-*` skills are not in this list: they come from the Requirements
//! of each project. Writing and comparing the files in a project is not done here; this module
//! only says which files exist and what each one holds.
//!
//! # The command reference
//!
//! The skills point to one generated reference of every `weaver` command,
//! [`COMMAND_REFERENCE_ASSET`]. The command registry of the CLI crate is the source of that
//! text, and a test of that crate fails when [`command_reference`] differs from what the
//! registry renders. The registry must therefore expose:
//!
//! ```text
//! weaver_cli::registry::command_reference() -> String
//! ```
//!
//! returning the whole file: Markdown, LF, a final newline. The CLI crate also owns the way
//! the file is regenerated (it writes [`COMMAND_REFERENCE_ASSET`], relative to the folder of
//! `weaver-core`, i.e. `crates/core/assets/skills/weaver-design/COMMANDS.md`).
//! Nothing in this crate depends on the format, only on the reference naming every command
//! as `weaver <object> <verb>` (see the tests).

use std::sync::LazyLock;

use include_dir::{include_dir, Dir, DirEntry};

use crate::project::text;

static ASSETS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/assets");

/// The folder of the installed skills, relative to the project root. Always this one: there is
/// no setting for it, so the app and the command line find the same folder.
pub const SKILLS_DIR: &str = ".agents/skills";

/// The folder of the omp files (sub-agent definitions and advisor instructions), relative to the
/// project root.
pub const OMP_DIR: &str = ".omp";

/// The installed skills, one folder each with a `SKILL.md`.
pub const SKILL_NAMES: [&str; 8] = [
    "weaver-setup",
    "weaver-design",
    "weaver-import",
    "weaver-scan",
    "weaver-compare",
    "weaver-implement",
    "weaver-resolve-drifts",
    "weaver-resolve-conflicts",
];

/// A sub-agent definition is smaller than this, in bytes (6 KB).
pub const SUBAGENT_DEFINITION_LIMIT: usize = 6 * 1024;

/// Where the generated command reference sits in the sources, relative to `crates/core/assets/`.
pub const COMMAND_REFERENCE_ASSET: &str = "skills/weaver-design/COMMANDS.md";

/// Where the generated command reference is installed, relative to the project root.
pub const COMMAND_REFERENCE_PATH: &str = ".agents/skills/weaver-design/COMMANDS.md";

/// One file Weaver installs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledFile {
    /// Path relative to the project root, always with `/`.
    pub path: String,
    /// The content, LF-normalised: it is what the file must hold, byte for byte, once written.
    pub contents: String,
}

impl InstalledFile {
    /// Whether this is a sub-agent definition (`.omp/agents/*.md`).
    pub fn is_subagent_definition(&self) -> bool {
        self.path.starts_with(".omp/agents/") && self.path.ends_with(".md")
    }
}

/// Where the embedded file `asset` (relative to `crates/core/assets/`) is installed.
fn install_path(asset: &str) -> Option<String> {
    if let Some(rest) = asset.strip_prefix("skills/") {
        Some(format!("{SKILLS_DIR}/{rest}"))
    } else {
        asset
            .strip_prefix("omp/")
            .map(|rest| format!("{OMP_DIR}/{rest}"))
    }
}

fn collect(dir: &'static Dir<'static>, out: &mut Vec<(&'static str, &'static [u8])>) {
    for entry in dir.entries() {
        match entry {
            DirEntry::Dir(sub) => collect(sub, out),
            DirEntry::File(file) => {
                if let Some(path) = file.path().to_str() {
                    out.push((path, file.contents()));
                }
            }
        }
    }
}

static FILES: LazyLock<Vec<InstalledFile>> = LazyLock::new(|| {
    let mut sources = Vec::new();
    collect(&ASSETS, &mut sources);
    let mut files: Vec<InstalledFile> = sources
        .into_iter()
        .filter_map(|(asset, bytes)| {
            let path = install_path(&asset.replace('\\', "/"))?;
            let contents = text::decode(bytes).ok()?;
            Some(InstalledFile { path, contents })
        })
        .collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files
});

/// Every file Weaver installs, sorted by path. The list is built once.
pub fn installed_files() -> &'static [InstalledFile] {
    &FILES
}

/// The content installed at `path` (relative to the project root, with `/`), if it is in the list.
pub fn installed_file(path: &str) -> Option<&'static InstalledFile> {
    installed_files().iter().find(|file| file.path == path)
}

/// The embedded generated command reference, as installed.
pub fn command_reference() -> &'static str {
    installed_file(COMMAND_REFERENCE_PATH).map_or("", |file| file.contents.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// The objects and verbs of `requirements/cli#General`: the commands that exist. The command
    /// registry of the CLI crate must offer exactly these; its own tests compare it with them.
    const REGISTRY: [(&str, &[&str]); 9] = [
        ("model", &["show", "list", "check"]),
        ("project", &["new", "edit", "show"]),
        ("proposal", &["new", "show", "list", "edit", "move"]),
        ("task", &["new", "show", "list", "move", "delete"]),
        ("drift", &["new", "show", "list", "edit", "move"]),
        ("history", &["new", "edit", "show", "list"]),
        ("announcement", &["new"]),
        ("job", &["new", "show", "list", "edit"]),
        ("question", &["new"]),
    ];

    /// Words and phrases of the features Weaver does not have; none may remain in an installed text.
    const OUT_OF_SCOPE_WORDS: [&str; 11] = [
        "comparison",
        "handoff",
        "hand-off",
        "unify",
        "unified",
        "terminal",
        "hook",
        "timing",
        "team",
        "diagram",
        "diagrams",
    ];
    const OUT_OF_SCOPE_PHRASES: [&str; 12] = [
        "trama-merge",
        "pr check",
        "pre-flight",
        "preflight",
        "\"progress\"",
        "\"kind\":\"check\"",
        "announcement move",
        "announcement edit",
        "weaver task check",
        "weaver merge",
        "weaver timing",
        "weaver comparison",
    ];

    fn all_words(text: &str) -> impl Iterator<Item = &str> {
        text.split(|c: char| !(c.is_alphanumeric() || c == '\''))
            .filter(|w| !w.is_empty())
    }

    fn files() -> &'static [InstalledFile] {
        installed_files()
    }

    #[test]
    fn every_embedded_source_has_an_installed_path() {
        let mut sources = Vec::new();
        collect(&ASSETS, &mut sources);
        assert_eq!(
            sources.len(),
            files().len(),
            "a source has no installed path or is not UTF-8"
        );
        let paths: BTreeSet<_> = files().iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths.len(),
            files().len(),
            "two sources share an installed path"
        );
    }

    #[test]
    fn the_list_is_exactly_the_one_the_requirement_names() {
        let mut expected: Vec<String> = SKILL_NAMES
            .iter()
            .map(|n| format!("{SKILLS_DIR}/{n}/SKILL.md"))
            .collect();
        expected.extend([
            ".agents/skills/weaver-design/FORMAT.md".to_owned(),
            COMMAND_REFERENCE_PATH.to_owned(),
            ".omp/agents/weaver-analysis.md".to_owned(),
            ".omp/agents/weaver-implementer.md".to_owned(),
            ".omp/WATCHDOG.yml".to_owned(),
        ]);
        expected.sort();
        let found: Vec<&str> = files().iter().map(|f| f.path.as_str()).collect();
        assert_eq!(found, expected);
    }

    #[test]
    fn every_installed_text_is_lf_utf8_with_a_final_newline() {
        for file in files() {
            assert!(!file.contents.contains('\r'), "{} has a CR", file.path);
            assert!(
                !file.contents.starts_with('\u{feff}'),
                "{} has a BOM",
                file.path
            );
            assert!(
                file.contents.ends_with('\n') && !file.contents.ends_with("\n\n"),
                "{} final newline",
                file.path
            );
            assert!(!file.contents.contains("\t"), "{} has a tab", file.path);
        }
    }

    #[test]
    fn every_installed_text_is_english() {
        const ITALIAN: [&str; 14] = [
            "il", "della", "delle", "degli", "che", "sono", "una", "nel", "nella", "dei", "gli",
            "questo", "quando", "perché",
        ];
        const ALLOWED_NON_ASCII: [char; 5] = ['→', '—', '…', '–', '↔'];
        for file in files() {
            for c in file.contents.chars().filter(|c| !c.is_ascii()) {
                assert!(
                    ALLOWED_NON_ASCII.contains(&c),
                    "{}: unexpected character {c:?}",
                    file.path
                );
            }
            for word in all_words(&file.contents) {
                assert!(
                    !ITALIAN.contains(&word.to_lowercase().as_str()),
                    "{}: Italian word {word:?}",
                    file.path
                );
            }
        }
    }

    #[test]
    fn no_installed_text_uses_the_first_person() {
        for file in files() {
            for word in all_words(&file.contents) {
                let lower = word.to_lowercase();
                assert!(
                    !matches!(
                        lower.as_str(),
                        "we" | "my"
                            | "our"
                            | "ours"
                            | "me"
                            | "i'm"
                            | "i'll"
                            | "i've"
                            | "we'll"
                            | "we're"
                    ) && word != "I",
                    "{}: first person {word:?}",
                    file.path
                );
            }
        }
    }

    #[test]
    fn no_installed_text_names_the_product_it_came_from() {
        for file in files() {
            for word in all_words(&file.contents) {
                assert_ne!(word, "Trama", "{} names the old product", file.path);
            }
        }
    }

    #[test]
    fn every_sub_agent_definition_is_under_6_kb() {
        let definitions: Vec<_> = files()
            .iter()
            .filter(|f| f.is_subagent_definition())
            .collect();
        assert_eq!(definitions.len(), 2);
        for file in definitions {
            assert!(
                file.contents.len() < SUBAGENT_DEFINITION_LIMIT,
                "{}: {} bytes",
                file.path,
                file.contents.len()
            );
        }
    }

    #[test]
    fn no_installed_text_mentions_a_feature_weaver_does_not_have() {
        for file in files() {
            let lower = file.contents.to_lowercase();
            for phrase in OUT_OF_SCOPE_PHRASES {
                assert!(
                    !lower.contains(phrase),
                    "{}: out of scope {phrase:?}",
                    file.path
                );
            }
            for word in all_words(&lower) {
                assert!(
                    !OUT_OF_SCOPE_WORDS.contains(&word),
                    "{}: out of scope word {word:?}",
                    file.path
                );
            }
        }
    }

    /// Every `weaver <object> <verb>` an installed text quotes.
    fn quoted_commands(text: &str) -> Vec<(String, Option<String>, String)> {
        let mut found = Vec::new();
        for (line_no, line) in text.lines().enumerate() {
            for (at, _) in line.match_indices("weaver ") {
                // A command is quoted in a code span (or starts a line of a code block).
                let before = line[..at].trim_end_matches([' ', '$']);
                if !((before.ends_with('`') && line[..at].ends_with('`')) || before.is_empty()) {
                    continue;
                }
                let mut tokens = line[at + "weaver ".len()..].split_whitespace().map(|t| {
                    t.trim_matches(|c: char| {
                        matches!(c, '`' | '.' | ',' | ';' | ':' | ')' | '(' | '"')
                    })
                });
                let Some(object) = tokens.next() else {
                    continue;
                };
                if object.starts_with('-')
                    || object.is_empty()
                    || !object.chars().all(|c| c.is_ascii_lowercase())
                {
                    continue;
                }
                let verb = tokens
                    .next()
                    .filter(|v| !v.is_empty() && v.chars().all(|c| c.is_ascii_lowercase()))
                    .map(str::to_owned);
                found.push((
                    object.to_owned(),
                    verb,
                    format!("line {}: {}", line_no + 1, line.trim()),
                ));
            }
        }
        found
    }

    #[test]
    fn every_quoted_command_is_in_the_registry() {
        let mut quoted = 0;
        for file in files() {
            for (object, verb, place) in quoted_commands(&file.contents) {
                let Some((_, verbs)) = REGISTRY.iter().find(|(name, _)| *name == object) else {
                    panic!(
                        "{}: `weaver {object}` is not a command ({place})",
                        file.path
                    );
                };
                let Some(verb) = verb else {
                    panic!("{}: `weaver {object}` has no verb ({place})", file.path)
                };
                assert!(
                    verbs.contains(&verb.as_str()),
                    "{}: `weaver {object} {verb}` is not a command ({place})",
                    file.path
                );
                quoted += 1;
            }
        }
        assert!(
            quoted > 100,
            "the quoted commands were not found ({quoted})"
        );
    }

    #[test]
    fn the_command_reference_names_every_command() {
        let reference = command_reference();
        assert!(!reference.is_empty(), "{COMMAND_REFERENCE_PATH} is missing");
        for (object, verbs) in REGISTRY {
            for verb in verbs {
                assert!(
                    reference.contains(&format!("weaver {object} {verb}")),
                    "reference lacks `weaver {object} {verb}`"
                );
            }
        }
        let cited: BTreeSet<(String, String)> = quoted_commands(reference)
            .into_iter()
            .filter_map(|(object, verb, _)| verb.map(|verb| (object, verb)))
            .collect();
        let known: BTreeSet<(String, String)> = REGISTRY
            .iter()
            .flat_map(|(object, verbs)| {
                verbs
                    .iter()
                    .map(move |verb| ((*object).to_owned(), (*verb).to_owned()))
            })
            .collect();
        assert_eq!(cited, known);
    }

    fn text_of(path: &str) -> String {
        installed_file(path)
            .unwrap_or_else(|| panic!("{path} is not installed"))
            .contents
            .clone()
    }

    #[test]
    fn the_skills_are_the_eight_the_requirement_names_in_that_order() {
        assert_eq!(
            SKILL_NAMES,
            [
                "weaver-setup",
                "weaver-design",
                "weaver-import",
                "weaver-scan",
                "weaver-compare",
                "weaver-implement",
                "weaver-resolve-drifts",
                "weaver-resolve-conflicts",
            ]
        );
    }

    /// `requirements/rules#project`, **Merge texts**: `weaver-resolve-conflicts`.
    #[test]
    fn the_conflicts_skill_reads_the_merge_resolves_asks_once_marks_with_git_add_and_never_ends_the_merge(
    ) {
        let skill = text_of(".agents/skills/weaver-resolve-conflicts/SKILL.md");
        for needle in [
            "name: weaver-resolve-conflicts",
            ".weaver/merge/merge.json",
            "git diff --name-only --diff-filter=U",
            "**Resolve the obvious ones.**",
            "one round only",
            "`weaver question new`",
            "names the file",
            "the two versions",
            "`git add <path>`",
            "If conflicts are left when the job ends, the merge stays in progress",
            "\n## Language\n",
            "\n## Sub-agents context sharing\n",
            "\n## Asking\n",
            "`timeout: 0`",
        ] {
            assert!(skill.contains(needle), "the skill lacks {needle:?}");
        }
        // Never ends or loses the merge.
        let rules = skill
            .split("## Hard rules")
            .nth(1)
            .and_then(|rest| rest.split("\n## ").next())
            .expect("hard rules");
        for forbidden in [
            "NEVER run `git commit`",
            "`git merge --abort`",
            "`git reset`",
            "`git checkout` of a branch",
            "`git stash`",
        ] {
            assert!(
                rules.contains(forbidden),
                "the hard rules lack {forbidden:?}"
            );
        }
        // It is the skill of the job whose instruction the core composes.
        assert!(skill.starts_with("---\nname: weaver-resolve-conflicts\n"));
    }

    /// `requirements/rules#project`, **Merge texts**: the scan after a merge.
    #[test]
    fn the_scan_texts_say_that_a_scan_after_a_merge_files_merge_drifts_only_for_the_merged_files() {
        let scan = text_of(".agents/skills/weaver-scan/SKILL.md");
        let compare = text_of(".agents/skills/weaver-compare/SKILL.md");
        let analysis = text_of(".omp/agents/weaver-analysis.md");
        for (name, text) in [
            ("scan", &scan),
            ("compare", &compare),
            ("analysis", &analysis),
        ] {
            assert!(
                text.contains("\"merge\":true"),
                "{name} does not file `merge`"
            );
            assert!(text.contains("merge"), "{name}");
        }
        assert!(scan.contains("\n## After a merge\n"));
        assert!(compare.contains("\n## After a merge\n"));
        assert!(scan.contains(
            "Use the weaver-scan skill on the whole System after a merge: read the files the merge brought with `weaver job show`."
        ));
        assert!(scan.contains("`weaver job show`") && scan.contains("`merged`"));
        assert!(scan.contains("Pass the list to every `weaver-analysis` sub-agent"));
        assert!(scan.contains("no other Drift"));
        assert!(scan.contains("Only this scan may set `merge`"));
        assert!(scan.contains("a dismissed Drift whose text has not changed is never filed again"));
        assert!(compare.contains("Only a scan after a merge may set `merge`"));
        assert!(
            analysis.len() < SUBAGENT_DEFINITION_LIMIT,
            "{} bytes",
            analysis.len()
        );
    }

    /// `requirements/rules#project`, **Merge texts**: `weaver-resolve-drifts`.
    #[test]
    fn the_resolve_skill_says_a_merge_drifts_prepared_question_has_the_proposal_option_recommended()
    {
        let skill = text_of(".agents/skills/weaver-resolve-drifts/SKILL.md");
        let paragraph = skill
            .lines()
            .find(|line| line.starts_with("- **A Drift from a merge**"))
            .expect("the paragraph of a merge Drift");
        for needle in [
            "the decision `proposal`",
            "attached Draft",
            "`recommended`",
            "Fix options stay, in their order",
            "none is preselected",
        ] {
            assert!(paragraph.contains(needle), "{needle:?}");
        }
    }

    #[test]
    fn every_skill_points_to_the_reference_and_has_the_sections_the_instructions_name() {
        for name in SKILL_NAMES {
            let file = installed_file(&format!("{SKILLS_DIR}/{name}/SKILL.md")).expect("skill");
            assert!(
                file.contents
                    .starts_with(&format!("---\nname: {name}\ndescription: ")),
                "{name}: front matter"
            );
            assert!(
                file.contents.contains("../weaver-design/COMMANDS.md"),
                "{name}: no command reference"
            );
            assert!(
                file.contents.contains("\n## Language\n"),
                "{name}: no Language section"
            );
            assert!(
                file.contents.contains("\n## Sub-agents context sharing\n"),
                "{name}: no context sharing section"
            );
        }
        for name in [
            "weaver-setup",
            "weaver-design",
            "weaver-import",
            "weaver-scan",
            "weaver-implement",
            "weaver-resolve-drifts",
            "weaver-resolve-conflicts",
        ] {
            let file = installed_file(&format!("{SKILLS_DIR}/{name}/SKILL.md")).expect("skill");
            assert!(
                file.contents.contains("\n## Asking\n"),
                "{name}: no Asking section"
            );
            assert!(
                file.contents.contains("`timeout: 0`"),
                "{name}: no waiting rule"
            );
        }
    }

    #[test]
    fn the_sub_agent_definitions_have_the_models_the_requirement_gives() {
        let analysis = installed_file(".omp/agents/weaver-analysis.md").expect("analysis");
        assert!(analysis.contents.contains("\nmodel: \"@slow\"\n"));
        assert!(analysis.contents.contains("weaver-compare"));
        let implementer = installed_file(".omp/agents/weaver-implementer.md").expect("implementer");
        let front_matter = implementer
            .contents
            .split("\n---\n")
            .next()
            .expect("front matter");
        assert!(
            !front_matter.contains("\nmodel:"),
            "the implementer inherits the model of its Run"
        );
        assert!(implementer.contents.contains("(one to three Tasks)"));
    }

    #[test]
    fn the_advisor_instructions_define_the_two_reviewers() {
        let file = installed_file(".omp/WATCHDOG.yml").expect("watchdog");
        let doc: serde_yaml::Value = serde_yaml::from_str(&file.contents).expect("valid YAML");
        let instructions = doc["instructions"].as_str().expect("instructions");
        assert!(instructions.contains("run `weaver job show` before any other command"));
        assert!(instructions.contains("stay silent"));
        let advisors = doc["advisors"].as_sequence().expect("advisors");
        assert_eq!(advisors.len(), 2);
        let expect = [("turn", "off"), ("agent-end", "strict")];
        for (advisor, (mode, backlog)) in advisors.iter().zip(expect) {
            assert_eq!(advisor["reviewMode"].as_str(), Some(mode));
            assert_eq!(advisor["syncBacklog"].as_str(), Some(backlog));
            let tools: Vec<_> = advisor["tools"]
                .as_sequence()
                .expect("tools")
                .iter()
                .filter_map(|t| t.as_str())
                .collect();
            assert_eq!(tools, ["read", "grep", "glob", "bash"]);
            assert!(advisor.get("model").is_none(), "an advisor names no model");
        }
    }
}
