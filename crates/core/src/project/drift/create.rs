//! `drift new` and `drift edit`: the files they write.

use serde_yaml::Mapping;

use crate::project::cards::{Card, CardKind};
use crate::project::changes::ChangeSet;
use crate::project::frontmatter;
use crate::project::ids::{card_stem, slugify};
use crate::project::image::ProjectImage;
use crate::project::layout::DRIFTS_DIR;
use crate::project::proposal::data::{self, set, strings, Kind, Status};
use crate::project::random::RandomSource;
use crate::project::refs::parse_reference;
use crate::project::tasks;

use super::prepared::{self, Prepared, PreparedDecision};
use super::{
    drift_options, fingerprint, is_merge, is_open, refs_of, status_of, FixOption, DRIFT_KEYS,
};
use super::{FIX_OPTIONS, HAPPENS, SHOULD, WHERE_IN_CODE};

/// A problem of an input, with the JSON path of the value it comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub path: String,
    pub message: String,
}

fn problem(path: impl Into<String>, message: impl Into<String>) -> Problem {
    Problem {
        path: path.into(),
        message: message.into(),
    }
}

/// The options of a Drift: two to five.
pub const MIN_OPTIONS: usize = 2;
pub const MAX_OPTIONS: usize = 5;

/// What `drift new` was asked. The Task, the Proposal and the Drift are given by identity.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewDrift {
    pub title: String,
    pub refs: Vec<String>,
    pub happens: String,
    pub should: String,
    pub where_in_code: String,
    /// The fix options, the recommended first; none only with `proposal`.
    pub options: Vec<FixOption>,
    /// The Task during which the Drift was found.
    pub task: Option<String>,
    pub proposal: Option<String>,
    /// The Drift this one repeats; set from the origin when it is a fix Task.
    pub previous: Option<String>,
    /// The job that files it.
    pub job: Option<String>,
    /// The Drift comes from a merge. Only the agent of a scan after a merge may ask for it: the
    /// caller checks that; this module only writes the key.
    pub merge: bool,
}

/// A Drift about to be filed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filed {
    pub id: String,
    /// `drifts/<id>-<slug>.md`, relative to `.weaver/`.
    pub path: String,
    pub changes: ChangeSet,
    /// The earlier Drift it repeats, explicit or taken from the origin.
    pub previous: Option<String>,
}

fn one_line(text: &str) -> bool {
    !text.trim().is_empty() && !text.contains(['\n', '\r'])
}

/// The Proposal `id` as something a Drift may hold: in Draft or Open, not a resolution and not held
/// by another Drift. `drift` is the Drift that would hold it.
fn proposal_problem(image: &ProjectImage, id: &str, drift: Option<&str>) -> Option<String> {
    let Some(proposal) = image.card(CardKind::Proposal, id) else {
        return Some(format!("no Proposal has the identity {id}"));
    };
    if !matches!(
        data::status_of(proposal),
        Some(Status::Draft | Status::Open)
    ) {
        return Some("the Proposal is not in Draft or Open".to_owned());
    }
    if data::kind_of(proposal) == Kind::Resolution {
        return Some("a resolution is not attached to a Drift".to_owned());
    }
    if image
        .cards_of(CardKind::Drift)
        .any(|d| d.str_key("proposal") == Some(id) && Some(d.id.as_str()) != drift)
    {
        return Some("another Drift holds the Proposal".to_owned());
    }
    None
}

/// Plans `drift new`. Refused, with every problem, and then nothing is written.
pub fn plan_new(
    image: &ProjectImage,
    input: &NewDrift,
    created: &str,
    rng: &mut dyn RandomSource,
) -> Result<Filed, Vec<Problem>> {
    let mut problems = Vec::new();
    if !one_line(&input.title) {
        problems.push(problem("title", "the title is one non-blank line"));
    }
    for (path, text) in [
        ("happens", &input.happens),
        ("should", &input.should),
        ("where", &input.where_in_code),
    ] {
        if text.trim().is_empty() {
            problems.push(problem(path, "must not be blank"));
        }
    }
    let resolver = image.resolver();
    let mut refs: Vec<String> = Vec::new();
    for (index, text) in input.refs.iter().enumerate() {
        let path = format!("refs[{index}]");
        match parse_reference(text) {
            Err(message) => problems.push(problem(path, message)),
            Ok(reference) => match resolver.resolve(&reference) {
                Err(message) => problems.push(problem(path, message)),
                Ok(_) => {
                    if !refs.contains(text) {
                        refs.push(text.clone());
                    }
                }
            },
        }
    }
    if input.refs.is_empty() {
        problems.push(problem("refs", "at least one reference"));
    }

    if input.options.is_empty() && input.proposal.is_none() {
        problems.push(problem("options", "a Drift offers 2 to 5 fix options; it offers none only with an attached `proposal` (when no change of the code can respect both the Model and the Requirements)"));
    }
    if !input.options.is_empty() && !(MIN_OPTIONS..=MAX_OPTIONS).contains(&input.options.len()) {
        problems.push(problem(
            "options",
            format!(
                "a Drift offers {MIN_OPTIONS} to {MAX_OPTIONS} fix options (found {})",
                input.options.len()
            ),
        ));
    }
    for (index, option) in input.options.iter().enumerate() {
        if !one_line(&option.title) {
            problems.push(problem(
                format!("options[{index}].title"),
                "the title is one non-blank line",
            ));
        }
        if option.text.trim().is_empty() {
            problems.push(problem(
                format!("options[{index}].text"),
                "must not be blank",
            ));
        }
    }
    if let Some(proposal) = &input.proposal {
        if let Some(why) = proposal_problem(image, proposal, None) {
            problems.push(problem("proposal", why));
        }
    }
    let origin = match &input.task {
        Some(task) => match image.card(CardKind::Task, task) {
            Some(card) => Some(card),
            None => {
                problems.push(problem("task", format!("no Task has the identity {task}")));
                None
            }
        },
        None => None,
    };
    let previous = match &input.previous {
        Some(previous) => {
            if image.card(CardKind::Drift, previous).is_none() {
                problems.push(problem(
                    "previous",
                    format!("no Drift has the identity {previous}"),
                ));
            }
            Some(previous.clone())
        }
        // The origin is a fix Task: the Drift repeats the one the fix Task was made for.
        None => origin
            .filter(|task| tasks::is_fix_task(task))
            .and_then(|task| task.str_key("drift"))
            .map(str::to_owned),
    };
    if !problems.is_empty() {
        return Err(problems);
    }

    let id = image.new_identity(CardKind::Drift, rng);
    let title = input.title.trim();
    let path = format!("{DRIFTS_DIR}/{}.md", card_stem(&id, &slugify(title)));
    let mut front = Mapping::new();
    set(&mut front, "status", "open");
    set(&mut front, "refs", strings(&refs));
    if let Some(task) = &input.task {
        set(&mut front, "origin", task.as_str());
    }
    if let Some(previous) = &previous {
        set(&mut front, "previous", previous.as_str());
    }
    if let Some(proposal) = &input.proposal {
        set(&mut front, "proposal", proposal.as_str());
    }
    if let Some(job) = &input.job {
        set(&mut front, "job", job.as_str());
    }
    if input.merge {
        set(&mut front, "merge", true);
    }
    set(&mut front, "created", created);

    let mut body = format!(
        "# {title}\n\n## {HAPPENS}\n\n{}\n\n## {SHOULD}\n\n{}\n",
        input.happens.trim(),
        input.should.trim()
    );
    if !input.options.is_empty() {
        body.push_str(&format!("\n## {FIX_OPTIONS}\n"));
        for option in &input.options {
            body.push_str(&format!(
                "\n### {}\n\n{}\n",
                option.title.trim(),
                option.text.trim()
            ));
        }
    }
    body.push_str(&format!(
        "\n## {WHERE_IN_CODE}\n\n{}\n",
        input.where_in_code.trim()
    ));
    let mut changes = ChangeSet::new();
    changes.write(&path, frontmatter::render(&front, &DRIFT_KEYS, &body));
    Ok(Filed {
        id,
        path,
        changes,
        previous,
    })
}

// ---------------------------------------------------------------------------------------------
// edit
// ---------------------------------------------------------------------------------------------

/// A prepared question as `drift edit` is given it.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedInput {
    /// One question of `weaver question new`.
    pub question: serde_json::Value,
    pub decisions: Vec<PreparedDecision>,
}

/// What `drift edit` was asked.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EditDrift {
    pub prepared: Option<PreparedInput>,
    /// The identity of a Proposal to attach.
    pub proposal: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edited {
    pub changes: ChangeSet,
    /// The question was recorded.
    pub prepared: bool,
    /// The Proposal attached by this call.
    pub attached: Option<String>,
}

/// Plans `drift edit` on the Drift `id`. The Drift must be Open. A new preparation replaces the
/// earlier one; Weaver adds the fingerprint and the instant. Refused, with every problem, and then
/// nothing is written.
pub fn plan_edit(
    image: &ProjectImage,
    id: &str,
    request: &EditDrift,
    now: &str,
) -> Result<Edited, Vec<Problem>> {
    let Some(drift) = image.card(CardKind::Drift, id) else {
        return Err(vec![problem("", format!("no Drift has the identity {id}"))]);
    };
    let mut problems = Vec::new();
    if !is_open(drift) {
        return Err(vec![problem(
            "",
            format!(
                "the Drift is {}: only an Open Drift changes",
                status_of(drift).unwrap_or("unknown")
            ),
        )]);
    }
    if let Some(proposal) = &request.proposal {
        if drift.str_key("proposal").is_some() {
            problems.push(problem(
                "proposal",
                "the Drift has a Proposal attached already",
            ));
        } else if let Some(why) = proposal_problem(image, proposal, Some(id)) {
            problems.push(problem("proposal", why));
        }
    }
    let attached_now = request.proposal.is_some() && problems.is_empty()
        || prepared::live_attached(image, drift).is_some();
    let mut recorded: Option<Prepared> = None;
    if let Some(input) = &request.prepared {
        match prepared::validate_input(
            id,
            drift_options(drift).len(),
            is_merge(drift),
            attached_now,
            &input.question,
            &input.decisions,
        ) {
            Err(found) => problems.extend(
                found
                    .into_iter()
                    .map(|(path, message)| problem(path, message)),
            ),
            Ok(question) => {
                recorded = Some(Prepared {
                    question,
                    decisions: input.decisions.clone(),
                    fingerprint: fingerprint::stored_of_refs(image, &refs_of(drift)),
                    prepared: now.to_owned(),
                });
            }
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    Ok(Edited {
        changes: write_drift(drift, request.proposal.as_deref(), recorded.as_ref()),
        prepared: recorded.is_some(),
        attached: request.proposal.clone(),
    })
}

fn write_drift(drift: &Card, proposal: Option<&str>, prepared: Option<&Prepared>) -> ChangeSet {
    let mut front = drift.front.clone();
    if let Some(proposal) = proposal {
        set(&mut front, "proposal", proposal);
    }
    let body = match prepared {
        Some(prepared) => prepared::body_with(&drift.body, prepared),
        None => drift.body.clone(),
    };
    let mut changes = ChangeSet::new();
    changes.write(&drift.path, frontmatter::render(&front, &DRIFT_KEYS, &body));
    changes
}
