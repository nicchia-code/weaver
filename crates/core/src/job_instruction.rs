//! The one-line instruction Weaver sends to the agent of a job.
//!
//! It is one line of English built only from fixed text, permanent identities, project-relative
//! paths in backticks, numbers and the language name (`requirements/rules#job`). It never holds
//! a short number or the text of a request: the request is named by its file.
//!
//! The first sentence depends on the job. Then, in this order, always: the sub-agent limit, the
//! sub-agents context sharing, the question rules; and the language only when one is set. The
//! always-on features (advisor, prepared Drift questions, parallel slots) add no sentence: the
//! skills always do them.
//!
//! [`compose`] is pure and validates its input, so a caller can never produce a line that holds a
//! short number, a line break or a backtick that closes a path early.

use serde::Serialize;

use crate::agent_files::SKILL_NAMES;
use crate::assets::tags;
use crate::project::{ids, requests};

const SETUP: &str = SKILL_NAMES[0];
const DESIGN: &str = SKILL_NAMES[1];
const IMPORT: &str = SKILL_NAMES[2];
const SCAN: &str = SKILL_NAMES[3];
const IMPLEMENT: &str = SKILL_NAMES[5];
const RESOLVE: &str = SKILL_NAMES[6];
const RESOLVE_CONFLICTS: &str = SKILL_NAMES[7];

/// One job and what its first sentence names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Job {
    /// The setup of an empty Model, from its written request (identity of the request file);
    /// `assets` are the names of the assets the request tags when it is sent, each told to the
    /// agent as a file to read. They live in memory with the job until its instruction is sent.
    Setup {
        request: String,
        assets: Vec<String>,
    },
    /// A design, from its written request (identity of the request file), with the assets it tags.
    Design {
        request: String,
        assets: Vec<String>,
    },
    /// An import; `excluded` holds the project-relative paths the developer left out (none: every
    /// folder is imported).
    Import { excluded: Vec<String> },
    /// A scan of the whole System.
    Scan,
    /// A scan of the whole System that Weaver starts when a merge is concluded: the agent reads
    /// the files the merge brought with `weaver job show` (they are in the job's registry
    /// record, not in the instruction).
    ScanAfterMerge,
    /// The resolution of the conflicts of the merge in progress, recorded in
    /// `.weaver/merge/merge.json`.
    MergeConflicts,
    /// A Run of one Task.
    RunTask { task: String },
    /// A Run of all the runnable Tasks, or of the group of one model.
    RunAll { filter: RunAllFilter },
    /// A resolve of one Drift, with the answer already given to its prepared question, if any.
    Resolve {
        drift: String,
        answer: Option<ResolveAnswer>,
    },
    /// The rework of a rejected Proposal.
    Rework {
        proposal: String,
        kind: ProposalKind,
    },
    /// The refinement of a stale Proposal.
    Refinement {
        proposal: String,
        kind: ProposalKind,
    },
}

/// Which Tasks of a Run all, by the model they need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunAllFilter {
    /// Every runnable Task.
    All,
    /// A model group: the Tasks of these Proposals (their chosen model), in card order.
    Proposals(Vec<String>),
    /// The execution-model group: every Task but those of these Proposals, in card order.
    NotProposals(Vec<String>),
}

/// The answer of the developer to a prepared question, as it goes into the instruction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolveAnswer {
    /// The labels of the options the developer chose.
    pub selected: Vec<String>,
    /// The free text next to the choice, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// The kind of the Proposal being reworked or refined; it picks the skill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalKind {
    /// An ordinary change: `weaver-design`.
    Change,
    /// A setup: `weaver-setup` while the Model is empty, `weaver-design` otherwise.
    Setup { model_is_empty: bool },
    /// An import: `weaver-import`.
    Import,
    /// A resolution: `weaver-resolve-drifts`, on its Drifts still open (never none).
    Resolution { open_drifts: Vec<String> },
}

/// The settings that add sentences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// The sub-agent limit: for a Run all group, its share (the same number is the `max` of
    /// `weaver task list`). At least 1.
    pub max_agents: u32,
    /// The language name of the developer's texts, when one is set.
    pub language: Option<String>,
}

/// Why no instruction can be composed from the input.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InstructionError {
    #[error("`{field}` is not a permanent identity: {value:?}")]
    NotAnIdentity { field: &'static str, value: String },
    #[error("`{0:?}` is not a path that fits in a one-line instruction")]
    BadPath(String),
    #[error("the language name is empty or has a control character")]
    BadLanguage,
    #[error("the sub-agent limit is at least 1")]
    NoAgents,
    #[error("a resolution is reworked or refined on at least one open Drift")]
    NoDrifts,
}

/// Composes the instruction of `job`.
pub fn compose(job: &Job, settings: &Settings) -> Result<String, InstructionError> {
    if settings.max_agents == 0 {
        return Err(InstructionError::NoAgents);
    }
    let language = match &settings.language {
        Some(language) => {
            let language = language.trim();
            if language.is_empty() || language.chars().any(char::is_control) {
                return Err(InstructionError::BadLanguage);
            }
            Some(language)
        }
        None => None,
    };
    let mut line = first_sentence(job, settings.max_agents)?;
    line.push_str(&format!(
        " Run at most {} sub-agents at once.",
        settings.max_agents
    ));
    line.push_str(
        " Pass each sub-agent what you have already read that concerns its part, as the \"Sub-agents context sharing\" section of your skill says.",
    );
    line.push_str(
        " Ask the developer only with `weaver question new`, as the skill says, never with your own question tool; run it with no time limit and wait until it ends.",
    );
    if let Some(language) = language {
        line.push_str(&format!(
            " Write the texts for the developer in {language}, as the Language section of your skill says."
        ));
    }
    debug_assert!(!line.contains(['\n', '\r']));
    Ok(line)
}

fn identity(field: &'static str, value: &str) -> Result<(), InstructionError> {
    if ids::is_identity(value) && !ids::looks_like_short_number(value) {
        Ok(())
    } else {
        Err(InstructionError::NotAnIdentity {
            field,
            value: value.to_owned(),
        })
    }
}

fn request_identity(value: &str) -> Result<(), InstructionError> {
    if requests::id::is_request_id(value) {
        Ok(())
    } else {
        Err(InstructionError::NotAnIdentity {
            field: "request",
            value: value.to_owned(),
        })
    }
}

fn identities(field: &'static str, values: &[String]) -> Result<(), InstructionError> {
    values.iter().try_for_each(|value| identity(field, value))
}

fn path_in_backticks(path: &str) -> Result<String, InstructionError> {
    if path.trim().is_empty() || path.chars().any(|c| c == '`' || c.is_control()) {
        return Err(InstructionError::BadPath(path.to_owned()));
    }
    Ok(format!("`{path}`"))
}

fn backticked(field: &'static str, values: &[String]) -> Result<String, InstructionError> {
    identities(field, values)?;
    Ok(values
        .iter()
        .map(|v| format!("`{v}`"))
        .collect::<Vec<_>>()
        .join(", "))
}

/// The sentences of the assets a request tags, each after a space; a name that cannot be written
/// between backticks in the line is refused.
fn asset_sentences(assets: &[String]) -> Result<String, InstructionError> {
    if let Some(bad) = assets.iter().find(|name| !tags::is_quotable(name)) {
        return Err(InstructionError::BadPath(bad.clone()));
    }
    Ok(tags::instruction_sentences(assets))
}

fn first_sentence(job: &Job, max: u32) -> Result<String, InstructionError> {
    Ok(match job {
        Job::Setup { request, assets } => {
            request_identity(request)?;
            format!(
                "Use the {SETUP} skill on the setup request in `.weaver/requests/{request}.md`.{}",
                asset_sentences(assets)?
            )
        }
        Job::Design { request, assets } => {
            request_identity(request)?;
            format!(
                "Use the {DESIGN} skill on the design request in `.weaver/requests/{request}.md`.{}",
                asset_sentences(assets)?
            )
        }
        Job::Import { excluded } if excluded.is_empty() => {
            format!("Use the {IMPORT} skill on this repository. Import every folder and do not ask which folders to import.")
        }
        Job::Import { excluded } => {
            let paths = excluded
                .iter()
                .map(|p| path_in_backticks(p))
                .collect::<Result<Vec<_>, _>>()?
                .join(", ");
            format!(
                "Use the {IMPORT} skill on this repository. Do not import these paths and do not ask which folders to import: {paths}."
            )
        }
        Job::Scan => format!("Use the {SCAN} skill on the whole System."),
        Job::ScanAfterMerge => format!(
            "Use the {SCAN} skill on the whole System after a merge: read the files the merge brought with `weaver job show`."
        ),
        Job::MergeConflicts => format!(
            "Use the {RESOLVE_CONFLICTS} skill on the merge in progress recorded in `.weaver/merge/merge.json`."
        ),
        Job::RunTask { task } => {
            identity("task", task)?;
            format!("Use the {IMPLEMENT} skill on Task {task}.")
        }
        Job::RunAll { filter } => {
            let filter = match filter {
                RunAllFilter::All => String::new(),
                RunAllFilter::Proposals(ids) => {
                    identities("proposals", ids)?;
                    format!(",\"proposals\":{}", json(ids))
                }
                RunAllFilter::NotProposals(ids) => {
                    identities("notProposals", ids)?;
                    format!(",\"notProposals\":{}", json(ids))
                }
            };
            format!(
                "Use the {IMPLEMENT} skill on the runnable Tasks: take them from `weaver task list --input '{{\"runnable\":true,\"max\":{max},\"groups\":true{filter}}}'`, one batch at a time, until it says there is none."
            )
        }
        Job::Resolve { drift, answer } => {
            identity("drift", drift)?;
            let mut sentence = format!("Use the {RESOLVE} skill on Drift {drift}.");
            if let Some(answer) = answer {
                sentence.push_str(&format!(
                    " The developer already answered its prepared question: {}.",
                    serde_json::to_string(answer).expect("an answer serialises")
                ));
            }
            sentence
        }
        Job::Rework { proposal, kind } => {
            identity("proposal", proposal)?;
            let (skill, drifts) = skill_of(kind)?;
            let drifts = if drifts.is_empty() {
                String::new()
            } else {
                format!(" on the Drifts {drifts}")
            };
            format!("Use the {skill} skill to rework the rejected Proposal {proposal} with the developer's feedback{drifts}.")
        }
        Job::Refinement { proposal, kind } => {
            identity("proposal", proposal)?;
            let (skill, drifts) = skill_of(kind)?;
            let drifts = if drifts.is_empty() {
                String::new()
            } else {
                format!(", on the Drifts {drifts}")
            };
            format!("Use the {skill} skill to refine the stale Proposal {proposal} on the current Model and Requirements{drifts}.")
        }
    })
}

/// The skill that follows the kind of a Proposal, shared by a rework and a refinement, with the
/// Drifts of a resolution (backticked, comma separated; empty for any other kind). A resolution
/// with no Drift still open has no instruction.
fn skill_of(kind: &ProposalKind) -> Result<(&'static str, String), InstructionError> {
    Ok(match kind {
        ProposalKind::Change => (DESIGN, String::new()),
        ProposalKind::Setup {
            model_is_empty: true,
        } => (SETUP, String::new()),
        ProposalKind::Setup {
            model_is_empty: false,
        } => (DESIGN, String::new()),
        ProposalKind::Import => (IMPORT, String::new()),
        ProposalKind::Resolution { open_drifts } => {
            if open_drifts.is_empty() {
                return Err(InstructionError::NoDrifts);
            }
            (RESOLVE, backticked("drifts", open_drifts)?)
        }
    })
}

fn json(ids: &[String]) -> String {
    serde_json::to_string(ids).expect("a list of strings serialises")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUBAGENTS: &str = " Run at most 4 sub-agents at once. Pass each sub-agent what you have already read that concerns its part, as the \"Sub-agents context sharing\" section of your skill says. Ask the developer only with `weaver question new`, as the skill says, never with your own question tool; run it with no time limit and wait until it ends.";
    const LANGUAGE: &str = " Write the texts for the developer in Italian, as the Language section of your skill says.";

    fn settings(language: Option<&str>) -> Settings {
        Settings {
            max_agents: 4,
            language: language.map(str::to_owned),
        }
    }

    fn id(n: u8) -> String {
        format!("abcdef{n:02}")
    }

    /// The id of a request: the local time of its creation.
    fn request(n: u8) -> String {
        format!("2026-11-02T09-30-{n:02}")
    }

    /// Every job with the first sentence the table of `requirements/rules#job` gives it.
    fn cases() -> Vec<(Job, String)> {
        vec![
            (
                Job::Setup { request: request(1), assets: vec![] },
                "Use the weaver-setup skill on the setup request in `.weaver/requests/2026-11-02T09-30-01.md`.".into(),
            ),
            (
                Job::Design { request: request(2), assets: vec![] },
                "Use the weaver-design skill on the design request in `.weaver/requests/2026-11-02T09-30-02.md`.".into(),
            ),
            (
                Job::Import { excluded: vec![] },
                "Use the weaver-import skill on this repository. Import every folder and do not ask which folders to import.".into(),
            ),
            (
                Job::Import { excluded: vec!["node_modules".into(), "docs/old drafts".into()] },
                "Use the weaver-import skill on this repository. Do not import these paths and do not ask which folders to import: `node_modules`, `docs/old drafts`.".into(),
            ),
            (Job::Scan, "Use the weaver-scan skill on the whole System.".into()),
            (
                Job::ScanAfterMerge,
                "Use the weaver-scan skill on the whole System after a merge: read the files the merge brought with `weaver job show`.".into(),
            ),
            (
                Job::MergeConflicts,
                "Use the weaver-resolve-conflicts skill on the merge in progress recorded in `.weaver/merge/merge.json`.".into(),
            ),
            (Job::RunTask { task: id(3) }, "Use the weaver-implement skill on Task abcdef03.".into()),
            (
                Job::RunAll { filter: RunAllFilter::All },
                "Use the weaver-implement skill on the runnable Tasks: take them from `weaver task list --input '{\"runnable\":true,\"max\":4,\"groups\":true}'`, one batch at a time, until it says there is none.".into(),
            ),
            (
                Job::RunAll { filter: RunAllFilter::Proposals(vec![id(4), id(5)]) },
                "Use the weaver-implement skill on the runnable Tasks: take them from `weaver task list --input '{\"runnable\":true,\"max\":4,\"groups\":true,\"proposals\":[\"abcdef04\",\"abcdef05\"]}'`, one batch at a time, until it says there is none.".into(),
            ),
            (
                Job::RunAll { filter: RunAllFilter::NotProposals(vec![id(4), id(5)]) },
                "Use the weaver-implement skill on the runnable Tasks: take them from `weaver task list --input '{\"runnable\":true,\"max\":4,\"groups\":true,\"notProposals\":[\"abcdef04\",\"abcdef05\"]}'`, one batch at a time, until it says there is none.".into(),
            ),
            (Job::Resolve { drift: id(6), answer: None }, "Use the weaver-resolve-drifts skill on Drift abcdef06.".into()),
            (
                Job::Resolve {
                    drift: id(6),
                    answer: Some(ResolveAnswer { selected: vec!["Keep the code".into()], text: Some("Only for \"guests\"\nplease".into()) }),
                },
                "Use the weaver-resolve-drifts skill on Drift abcdef06. The developer already answered its prepared question: {\"selected\":[\"Keep the code\"],\"text\":\"Only for \\\"guests\\\"\\nplease\"}.".into(),
            ),
            (
                Job::Resolve { drift: id(6), answer: Some(ResolveAnswer { selected: vec![], text: None }) },
                "Use the weaver-resolve-drifts skill on Drift abcdef06. The developer already answered its prepared question: {\"selected\":[]}.".into(),
            ),
            (
                Job::Rework { proposal: id(7), kind: ProposalKind::Change },
                "Use the weaver-design skill to rework the rejected Proposal abcdef07 with the developer's feedback.".into(),
            ),
            (
                Job::Rework { proposal: id(7), kind: ProposalKind::Setup { model_is_empty: true } },
                "Use the weaver-setup skill to rework the rejected Proposal abcdef07 with the developer's feedback.".into(),
            ),
            (
                Job::Rework { proposal: id(7), kind: ProposalKind::Setup { model_is_empty: false } },
                "Use the weaver-design skill to rework the rejected Proposal abcdef07 with the developer's feedback.".into(),
            ),
            (
                Job::Rework { proposal: id(7), kind: ProposalKind::Import },
                "Use the weaver-import skill to rework the rejected Proposal abcdef07 with the developer's feedback.".into(),
            ),
            (
                Job::Rework { proposal: id(7), kind: ProposalKind::Resolution { open_drifts: vec![id(8), id(9)] } },
                "Use the weaver-resolve-drifts skill to rework the rejected Proposal abcdef07 with the developer's feedback on the Drifts `abcdef08`, `abcdef09`.".into(),
            ),
            (
                Job::Refinement { proposal: id(10), kind: ProposalKind::Change },
                "Use the weaver-design skill to refine the stale Proposal abcdef10 on the current Model and Requirements.".into(),
            ),
            (
                Job::Refinement { proposal: id(10), kind: ProposalKind::Setup { model_is_empty: true } },
                "Use the weaver-setup skill to refine the stale Proposal abcdef10 on the current Model and Requirements.".into(),
            ),
            (
                Job::Refinement { proposal: id(10), kind: ProposalKind::Setup { model_is_empty: false } },
                "Use the weaver-design skill to refine the stale Proposal abcdef10 on the current Model and Requirements.".into(),
            ),
            (
                Job::Refinement { proposal: id(10), kind: ProposalKind::Import },
                "Use the weaver-import skill to refine the stale Proposal abcdef10 on the current Model and Requirements.".into(),
            ),
            (
                Job::Refinement { proposal: id(10), kind: ProposalKind::Resolution { open_drifts: vec![id(8), id(9)] } },
                "Use the weaver-resolve-drifts skill to refine the stale Proposal abcdef10 on the current Model and Requirements, on the Drifts `abcdef08`, `abcdef09`.".into(),
            ),
        ]
    }

    #[test]
    fn every_job_gets_its_sentence_and_the_always_on_sentences_without_a_language() {
        for (job, first) in cases() {
            assert_eq!(
                compose(&job, &settings(None)).unwrap(),
                format!("{first}{SUBAGENTS}"),
                "{job:?}"
            );
        }
    }

    #[test]
    fn a_language_adds_one_last_sentence() {
        for (job, first) in cases() {
            assert_eq!(
                compose(&job, &settings(Some("Italian"))).unwrap(),
                format!("{first}{SUBAGENTS}{LANGUAGE}"),
                "{job:?}"
            );
        }
    }

    #[test]
    fn the_language_name_is_trimmed() {
        let line = compose(&Job::Scan, &settings(Some("  Italian\t"))).unwrap();
        assert!(line.ends_with(LANGUAGE));
    }

    #[test]
    fn the_instruction_is_one_line_with_no_short_number() {
        let short_number = |word: &str| {
            let mut chars = word.chars();
            matches!(chars.next(), Some('P' | 'T' | 'D' | 'p' | 't' | 'd'))
                && chars.clone().count() > 0
                && chars.all(|c| c.is_ascii_digit())
        };
        for (job, _) in cases() {
            let line = compose(&job, &settings(Some("Italian"))).unwrap();
            assert!(!line.contains(['\n', '\r']), "{line}");
            assert!(
                !line.split(|c: char| !c.is_alphanumeric()).any(short_number),
                "{line}"
            );
        }
    }

    #[test]
    fn the_sub_agent_limit_is_the_max_of_the_run_all_command() {
        let job = Job::RunAll {
            filter: RunAllFilter::All,
        };
        let line = compose(
            &job,
            &Settings {
                max_agents: 7,
                language: None,
            },
        )
        .unwrap();
        assert!(line.contains("\"max\":7"));
        assert!(line.contains(" Run at most 7 sub-agents at once."));
    }

    #[test]
    fn a_short_number_is_never_accepted_for_an_identity() {
        for bad in [
            "T3",
            "t3",
            "P12",
            "abc",
            "ABCDEF01",
            "abcdef0!",
            "abcdef012",
            "a1234567",
            "",
        ] {
            let err = compose(&Job::RunTask { task: bad.into() }, &settings(None)).unwrap_err();
            assert!(
                matches!(err, InstructionError::NotAnIdentity { field: "task", .. }),
                "{bad:?}: {err:?}"
            );
        }
        let err = compose(
            &Job::RunAll {
                filter: RunAllFilter::Proposals(vec![id(1), "P3".into()]),
            },
            &settings(None),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            InstructionError::NotAnIdentity {
                field: "proposals",
                ..
            }
        ));
        let err = compose(
            &Job::Setup {
                request: "../x".into(),
                assets: vec![],
            },
            &settings(None),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            InstructionError::NotAnIdentity {
                field: "request",
                ..
            }
        ));
    }

    #[test]
    fn the_assets_a_request_tags_follow_the_first_sentence_on_the_same_line() {
        let assets = vec!["logo.png".to_owned(), "my notes.txt".to_owned()];
        let design = compose(
            &Job::Design {
                request: request(2),
                assets: assets.clone(),
            },
            &settings(None),
        )
        .unwrap();
        assert_eq!(
            design,
            format!("Use the weaver-design skill on the design request in `.weaver/requests/2026-11-02T09-30-02.md`. Read the file `.weaver/assets/logo.png`. Read the file `.weaver/assets/my notes.txt`.{SUBAGENTS}")
        );
        let setup = compose(
            &Job::Setup {
                request: request(1),
                assets: assets[..1].to_vec(),
            },
            &settings(Some("Italian")),
        )
        .unwrap();
        assert_eq!(
            setup,
            format!("Use the weaver-setup skill on the setup request in `.weaver/requests/2026-11-02T09-30-01.md`. Read the file `.weaver/assets/logo.png`.{SUBAGENTS}{LANGUAGE}")
        );
        assert!(!design.contains('\n'));
        // An asset name that would break the line is refused, like a path.
        for bad in ["a`b", "a\nb", ""] {
            let err = compose(
                &Job::Design {
                    request: request(2),
                    assets: vec![bad.into()],
                },
                &settings(None),
            )
            .unwrap_err();
            assert_eq!(err, InstructionError::BadPath(bad.into()));
        }
    }

    #[test]
    fn a_path_that_would_break_the_line_is_refused() {
        for bad in ["", "  ", "a`b", "a\nb", "a\u{0}b"] {
            let err = compose(
                &Job::Import {
                    excluded: vec![bad.into()],
                },
                &settings(None),
            )
            .unwrap_err();
            assert_eq!(err, InstructionError::BadPath(bad.into()));
        }
    }

    #[test]
    fn an_unusable_setting_is_refused() {
        assert_eq!(
            compose(
                &Job::Scan,
                &Settings {
                    max_agents: 0,
                    language: None
                }
            ),
            Err(InstructionError::NoAgents)
        );
        for bad in ["", "   ", "Ita\nlian"] {
            assert_eq!(
                compose(&Job::Scan, &settings(Some(bad))),
                Err(InstructionError::BadLanguage),
                "{bad:?}"
            );
        }
        for job in [
            Job::Rework {
                proposal: id(1),
                kind: ProposalKind::Resolution {
                    open_drifts: vec![],
                },
            },
            Job::Refinement {
                proposal: id(1),
                kind: ProposalKind::Resolution {
                    open_drifts: vec![],
                },
            },
        ] {
            assert_eq!(
                compose(&job, &settings(None)),
                Err(InstructionError::NoDrifts),
                "{job:?}"
            );
        }
    }

    #[test]
    fn the_skills_named_are_installed_and_describe_the_sentences_that_name_their_sections() {
        use crate::agent_files::{installed_file, SKILLS_DIR};
        let skill = |name: &str| {
            installed_file(&format!("{SKILLS_DIR}/{name}/SKILL.md"))
                .expect("skill")
                .contents
                .as_str()
        };
        for (job, _) in cases() {
            let line = compose(&job, &settings(None)).unwrap();
            let name = line.split(' ').nth(2).expect("skill name").to_owned();
            assert!(line.starts_with(&format!("Use the {name} skill")));
            let text = skill(&name);
            assert!(text.contains("## Language"), "{name}");
            assert!(text.contains("## Sub-agents context sharing"), "{name}");
        }
        // The rework and refinement sentences name sections the skills must have.
        assert!(skill("weaver-design").contains("## Reworking a rejected Proposal"));
        assert!(skill("weaver-design").contains("## Refining a stale Proposal"));
        assert!(skill("weaver-setup").contains("## Reworking a rejected setup Proposal"));
        assert!(skill("weaver-import").contains("## Reworking a rejected import Proposal"));
        assert!(skill("weaver-resolve-drifts").contains("## Reworking a rejected resolution"));
        assert!(
            skill("weaver-resolve-drifts").contains("with the developer's feedback on the Drifts")
        );
        assert!(skill("weaver-resolve-drifts")
            .contains("The developer already answered its prepared question:"));
        assert!(skill("weaver-import")
            .contains("Do not import these paths and do not ask which folders to import"));
        assert!(skill("weaver-import")
            .contains("Import every folder and do not ask which folders to import"));
        // The refinement sentence names the section of each skill that refines its kind.
        assert!(skill("weaver-setup").contains("## Refining a stale setup Proposal"));
        assert!(skill("weaver-import").contains("## Refining a stale import Proposal"));
        assert!(skill("weaver-resolve-drifts").contains("## Refining a stale resolution"));
        assert!(skill("weaver-resolve-drifts")
            .contains("on the current Model and Requirements, on the Drifts"));
    }
}
