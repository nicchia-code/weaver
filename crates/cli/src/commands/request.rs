//! `weaver request new`: the agent of Chat writes a design request with what it agreed with the
//! developer and sends it. The app writes the request and starts its job
//! (`requirements/cli#design-request`); only the app can run it, through
//! [`crate::exec::Authority::run`].

use super::{app_only, in_app};
use crate::spec::*;

pub fn commands() -> Vec<Command> {
    vec![Command::new("request", "new", "Writes a design request with the given text and sends it, starting a design job on it (a setup job when the Model is empty).", app_only)
        .notes(&[
            "Run it only when the developer explicitly asked Chat for a Proposal. Weaver then starts a job that explores, asks the developer its own question rounds and hands the Proposal over; the agent of Chat does not wait for it and writes no Proposal.",
            "`text` is the whole request and must stand alone, because the job sees only the request and never the conversation: an H1 with a short title, then what to add or change, every decision agreed with the developer, and the alternatives discussed with the developer's choice. Name cards only by permanent identity, never by short number. Send no images: a request sent this way has none.",
            "Weaver writes the request as the developer's Send does: the text as given, with LF line endings and a final newline, and recognises the tags of assets in it at that moment. The request and the job are created together: when the job cannot start, nothing is written and the reason is printed (exit 1).",
        ])
        .access(Access::Chat)
        .input(vec![req("text", nonblank(), "The whole request, in Markdown; not blank.")])
        .output(vec![
            req("request", line(), "The identity of the request."),
            req("job", line(), "The identity of the job started on it."),
            req("kind", one_of(&["design", "setup"]), "The kind of job started: `setup` when the Model is empty."),
            req("title", line(), "The title of the request."),
        ])
        .place(in_app)
        .example(r##"weaver request new --input '{"text":"# Cancel an order\n\nLet a customer cancel an order that has not shipped."}'"##)]
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Map};

    use super::*;
    use crate::exec::check_input;

    fn command() -> &'static Command {
        crate::registry::find("request", "new").unwrap()
    }

    #[test]
    fn only_chat_may_run_it_and_only_the_app_can() {
        let command = command();
        assert_eq!(command.access, Access::Chat);
        assert_eq!(command.place_for(&Map::new(), None), Place::App);
        assert_eq!(command.arg, Arg::None);
    }

    #[test]
    fn the_text_must_not_be_blank_after_trimming() {
        let command = command();
        let check = |value: serde_json::Value| check_input(command, value.as_object().unwrap());
        assert!(check(json!({ "text": "# Orders\n\nAdd orders" })).is_ok());
        for blank in [json!({ "text": "" }), json!({ "text": " \n\t " })] {
            let error = check(blank).unwrap_err();
            assert_eq!(error.exit_code(), 2);
            assert!(error.message.contains("text"));
        }
        assert_eq!(check(json!({})).unwrap_err().exit_code(), 2);
        assert_eq!(
            check(json!({ "text": "x", "extra": 1 }))
                .unwrap_err()
                .exit_code(),
            2
        );
    }

    #[test]
    fn without_the_app_it_says_weaver_is_not_running() {
        let command = command();
        let image = weaver_core::project::image::ProjectImage::empty(std::path::Path::new("/p"));
        let input = Map::new();
        let caller = Caller::default();
        let call = Call {
            command,
            root: std::path::Path::new("/p"),
            image: &image,
            arg: None,
            state: None,
            input: &input,
            caller: &caller,
        };
        let error = (command.run)(&call).unwrap_err();
        assert_eq!(error.code, weaver_core::command::code::APP_NOT_RUNNING);
        assert!(error.message.contains("weaver request new"));
    }

    #[test]
    fn the_help_says_when_to_use_it_and_what_the_text_holds() {
        let notes = command().notes.join(" ");
        for needle in [
            "explicitly asked Chat for a Proposal",
            "stand alone",
            "an H1 with a short title",
            "permanent identity",
            "Send no images",
            "nothing is written",
        ] {
            assert!(notes.contains(needle), "{needle}");
        }
    }
}
