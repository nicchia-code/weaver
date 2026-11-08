//! Why an omp command failed, in words for the developer (`requirements/tech#omp`, "Failure
//! reasons"): the last non-empty line of stderr, else `exit code N`, `could not start the agent` or
//! `timed out`. [`classify`] and [`explanation`] tell whether a failure text is a certificate or
//! an HTTP error that a proxy inspecting HTTPS traffic may cause, and say it in plain words.

use crate::process::timed::Failure;

/// Codes of a certificate error, matched with their capitals. A longer code comes before the one
/// it starts with, so that the code reported is the exact one.
pub const CERTIFICATE_CODES: [&str; 8] = [
    "UNABLE_TO_GET_ISSUER_CERT_LOCALLY",
    "UNABLE_TO_GET_ISSUER_CERT",
    "UNABLE_TO_VERIFY_LEAF_SIGNATURE",
    "SELF_SIGNED_CERT_IN_CHAIN",
    "DEPTH_ZERO_SELF_SIGNED_CERT",
    "CERT_HAS_EXPIRED",
    "CERT_UNTRUSTED",
    "ERR_TLS_CERT_ALTNAME_INVALID",
];

/// Phrases of a certificate error, in lower case, matched ignoring case.
pub const CERTIFICATE_PHRASES: [&str; 6] = [
    "unable to get local issuer certificate",
    "self signed certificate in certificate chain",
    "self-signed certificate in certificate chain",
    "certificate verify failed",
    "ssl certificate problem",
    "curl: (60)",
];

/// The last non-empty line of `stderr`, trimmed.
pub fn last_line(stderr: &str) -> Option<&str> {
    stderr.lines().map(str::trim).rfind(|line| !line.is_empty())
}

/// What a failure text says about a proxy that inspects HTTPS traffic
/// (`requirements/rules#system-certificates`, "Certificate error" and "HTTP error").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyProblem {
    /// An error about a server's certificate; `marker` is the code or phrase that matched (a code
    /// with its capitals, a phrase in lower case).
    Certificate { marker: &'static str },
    /// An HTTP refusal: `403`, `407` or a code from `500` to `599`.
    Http { code: u16 },
}

/// Which kind of proxy problem `text` describes, if any. `text` is the whole stderr of a failed
/// omp command, an ACP error message, or the stderr tail of a job or General, never only the
/// line kept as the reason. A certificate error wins over an HTTP code.
pub fn classify(text: &str) -> Option<ProxyProblem> {
    if let Some(marker) = certificate_marker(text) {
        return Some(ProxyProblem::Certificate { marker });
    }
    http_code(text).map(|code| ProxyProblem::Http { code })
}

/// The explanation of `problem` in plain words, with no final full stop so that each place can
/// embed it (`requirements/ui#system-certificates`, "Proxy explanations"). With
/// `system_certificates_on` the text does not suggest turning System Certificates on.
pub fn explanation(problem: ProxyProblem, system_certificates_on: bool) -> String {
    match problem {
        ProxyProblem::Certificate { .. } => {
            certificate_explanation(system_certificates_on).to_owned()
        }
        ProxyProblem::Http { code } if system_certificates_on => {
            format!("the agent received an HTTP {code} error; {HTTP_BLOCKING}")
        }
        ProxyProblem::Http { code } => {
            format!("the agent received an HTTP {code} error; {HTTP_BLOCKING}, {TURN_ON}")
        }
    }
}

/// The explanation of a certificate error, for the setting as it is.
fn certificate_explanation(system_certificates_on: bool) -> &'static str {
    if system_certificates_on {
        CERTIFICATE_ON
    } else {
        CERTIFICATE_OFF
    }
}

/// [`classify`] then [`explanation`]: the explanation that replaces the reason of a failure whose
/// `text` is a certificate or HTTP error, else `None`.
pub fn explain(text: &str, system_certificates_on: bool) -> Option<String> {
    classify(text).map(|problem| explanation(problem, system_certificates_on))
}

/// The text of `Missing` after a failed installer (`requirements/ui#system-certificates`, "Failed
/// installer"), by the exit code the installer wrote in its marker, with the final full stop.
/// `91` and any code the installer does not give also say, with the setting off, that turning
/// System Certificates on may fix it.
pub fn installer_failed_text(exit_code: i32, system_certificates_on: bool) -> String {
    let suffix = if system_certificates_on {
        ""
    } else {
        " Turning on System Certificates in Weaver's settings may fix it."
    };
    match exit_code {
        90 => "The agent installer failed (exit code 90): the download of the agent was blocked on both of its download sites, probably by a corporate proxy that filters downloads. See the terminal.".to_owned(),
        91 => format!("The agent installer failed (exit code 91): the agent could not verify a server's certificate; a corporate proxy that inspects HTTPS traffic is the usual cause. See the terminal.{suffix}"),
        92 => "The agent installer failed (exit code 92): the downloaded agent was incomplete or altered, so it was not installed; a proxy may have changed it. See the terminal.".to_owned(),
        93 => "The agent installer failed (exit code 93): the downloaded agent does not start. See the terminal.".to_owned(),
        other => format!("The agent installer failed (exit code {other}); see the terminal. A corporate proxy that inspects HTTPS traffic is a common cause.{suffix}"),
    }
}

/// How the background download of an update ended without success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadFailure {
    /// The download exited with this code; `94` (the release is not newer) is not a failure of
    /// this kind: its reason stays the one of `omp update`.
    Exit(i32),
    /// The download went past its time limit.
    TimedOut,
}

/// The `(<reason>)` of a failed update whose background download failed
/// (`requirements/ui#system-certificates`, "Failed download of an update"), with no final full
/// stop. `91` is the certificate explanation for the setting as it is.
pub fn download_failed_reason(failure: DownloadFailure, system_certificates_on: bool) -> String {
    match failure {
        DownloadFailure::TimedOut => "the download of the agent timed out".to_owned(),
        DownloadFailure::Exit(90) => {
            "the download was blocked, probably by a corporate proxy that filters downloads"
                .to_owned()
        }
        DownloadFailure::Exit(91) => certificate_explanation(system_certificates_on).to_owned(),
        DownloadFailure::Exit(92) => "the downloaded agent was incomplete or altered".to_owned(),
        DownloadFailure::Exit(93) => "the downloaded agent did not start".to_owned(),
        DownloadFailure::Exit(other) => format!("the download of the agent failed (exit code {other})"),
    }
}

const CERTIFICATE_OFF: &str = "the agent could not verify a server's certificate; a corporate proxy that inspects HTTPS traffic is the usual cause, and turning on System Certificates in Weaver's settings may fix it";
const CERTIFICATE_ON: &str = "the agent could not verify a server's certificate, even with System Certificates on; a corporate proxy that inspects HTTPS traffic is the usual cause";
const HTTP_BLOCKING: &str =
    "a corporate proxy that inspects HTTPS traffic may be blocking the request";
const TURN_ON: &str = "and turning on System Certificates in Weaver's settings may fix it";

fn certificate_marker(text: &str) -> Option<&'static str> {
    if let Some(code) = CERTIFICATE_CODES
        .iter()
        .copied()
        .find(|code| text.contains(code))
    {
        return Some(code);
    }
    let lower = text.to_lowercase();
    CERTIFICATE_PHRASES
        .iter()
        .copied()
        .find(|phrase| lower.contains(phrase))
}

/// Whether `code` is one the rules call an HTTP error.
fn is_refusing(code: u16) -> bool {
    matches!(code, 403 | 407 | 500..=599)
}

/// The standard reason phrase of `code`, in lower case, when the rules list one.
fn standard_phrase(code: u16) -> Option<&'static str> {
    Some(match code {
        403 => "forbidden",
        407 => "proxy authentication required",
        500 => "internal server error",
        501 => "not implemented",
        502 => "bad gateway",
        503 => "service unavailable",
        504 => "gateway timeout",
        505 => "http version not supported",
        506 => "variant also negotiates",
        507 => "insufficient storage",
        508 => "loop detected",
        510 => "not extended",
        511 => "network authentication required",
        _ => return None,
    })
}

/// The forms that put the word before the code: `HTTP <code>`, `status <code>`, `status: <code>`,
/// `status code <code>`, `statusCode: <code>` and `returned error: <code>`, in lower case.
/// `HTTP/<version> <code>` is checked apart.
const CODE_PREFIXES: [&str; 6] = [
    "http",
    "status",
    "status:",
    "status code",
    "statuscode:",
    "returned error:",
];

/// The first code of the text, by position, that is written in one of the forms of the rules.
fn http_code(text: &str) -> Option<u16> {
    let lower = text.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        if !bytes[at].is_ascii_digit() {
            at += 1;
            continue;
        }
        let start = at;
        while at < bytes.len() && bytes[at].is_ascii_digit() {
            at += 1;
        }
        // Only a whole number of three digits, not part of a longer or decimal number.
        if at - start != 3 || is_inside_decimal(bytes, start, at) {
            continue;
        }
        let Ok(code) = lower[start..at].parse::<u16>() else {
            continue;
        };
        if is_refusing(code)
            && (written_after_word(&lower[..start]) || has_phrase(code, &lower[at..]))
        {
            return Some(code);
        }
    }
    None
}

/// A digit run joined to other digits by a point (`1.503`, `503.2`).
fn is_inside_decimal(bytes: &[u8], start: usize, end: usize) -> bool {
    let joined_before = start >= 2 && bytes[start - 1] == b'.' && bytes[start - 2].is_ascii_digit();
    let joined_after =
        bytes.get(end) == Some(&b'.') && bytes.get(end + 1).is_some_and(u8::is_ascii_digit);
    joined_before || joined_after
}

/// Whether a character that does not continue a word (or nothing) ends `before`.
fn ends_a_word(before: &str) -> bool {
    before
        .chars()
        .next_back()
        .is_none_or(|c| !c.is_ascii_alphanumeric())
}

/// Whether `before` (the lower-cased text up to a code) ends with one of the words that name a
/// code, then at least one blank.
fn written_after_word(before: &str) -> bool {
    let word_end = before.trim_end_matches([' ', '\t']);
    if word_end.len() == before.len() {
        return false;
    }
    if CODE_PREFIXES
        .iter()
        .any(|prefix| word_end.strip_suffix(prefix).is_some_and(ends_a_word))
    {
        return true;
    }
    // `HTTP/<version> <code>`.
    let without_version = word_end.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
    without_version.len() < word_end.len()
        && without_version
            .strip_suffix("http/")
            .is_some_and(ends_a_word)
}

/// Whether `after` (the lower-cased text after a code) is a blank and the standard phrase of
/// `code`, as whole words.
fn has_phrase(code: u16, after: &str) -> bool {
    let Some(phrase) = standard_phrase(code) else {
        return false;
    };
    let phrase_start = after.trim_start_matches([' ', '\t']);
    phrase_start.len() < after.len()
        && phrase_start.strip_prefix(phrase).is_some_and(|rest| {
            rest.chars()
                .next()
                .is_none_or(|c| !c.is_ascii_alphanumeric())
        })
}

impl Failure {
    /// The reason in the words of the requirements: the last stderr line, else `exit code N`,
    /// `could not start the agent` or `timed out`.
    pub fn reason(&self) -> String {
        match self {
            Failure::NotStarted => "could not start the agent".to_owned(),
            Failure::TimedOut { .. } => "timed out".to_owned(),
            Failure::Exit { code, stderr } => match (last_line(stderr), code) {
                (Some(line), _) => line.to_owned(),
                (None, Some(code)) => format!("exit code {code}"),
                (None, None) => "the agent was stopped".to_owned(),
            },
        }
    }

    /// The explanation of a certificate or HTTP error in the whole stderr
    /// (`requirements/rules#system-certificates`), chosen with the setting as it is now
    /// (`system_certificates_on`). Other failures have none.
    pub fn explained(&self, system_certificates_on: bool) -> Option<String> {
        match self {
            Failure::Exit { stderr, .. } => explain(stderr, system_certificates_on),
            _ => None,
        }
    }

    /// The reason of the failure: its explanation when it is a certificate or HTTP error, else
    /// [`Failure::reason`]. Decide it when the failure happens and never write it again.
    pub fn reason_for(&self, system_certificates_on: bool) -> String {
        self.explained(system_certificates_on)
            .unwrap_or_else(|| self.reason())
    }

    /// The reason in the words of the status box of `Missing`: `the agent did not answer within 15 s`,
    /// `the agent exited with code <N>`, else the last line of stderr.
    pub fn status_text(&self) -> String {
        match self {
            Failure::TimedOut { limit } => {
                format!("the agent did not answer within {} s", limit.as_secs())
            }
            Failure::Exit { code, stderr } if last_line(stderr).is_none() => match code {
                Some(code) => format!("the agent exited with code {code}"),
                None => "the agent was stopped".to_owned(),
            },
            other => other.reason(),
        }
    }

    /// [`Failure::status_text`], or the explanation of a certificate or HTTP error.
    pub fn status_text_for(&self, system_certificates_on: bool) -> String {
        self.explained(system_certificates_on)
            .unwrap_or_else(|| self.status_text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn exit(code: i32, stderr: &str) -> Failure {
        Failure::Exit {
            code: Some(code),
            stderr: stderr.to_owned(),
        }
    }

    #[test]
    fn the_reason_is_the_last_non_empty_line_of_stderr() {
        assert_eq!(
            exit(1, "first\nsecond problem  \n\n  \n").reason(),
            "second problem"
        );
    }

    #[test]
    fn without_stderr_the_reason_is_the_exit_code_or_the_way_it_failed() {
        assert_eq!(exit(3, "").reason(), "exit code 3");
        assert_eq!(exit(3, " \n").reason(), "exit code 3");
        assert_eq!(Failure::NotStarted.reason(), "could not start the agent");
        assert_eq!(
            Failure::TimedOut {
                limit: Duration::from_secs(15)
            }
            .reason(),
            "timed out"
        );
    }

    #[test]
    fn the_status_box_texts_for_missing() {
        assert_eq!(
            Failure::TimedOut {
                limit: Duration::from_secs(15)
            }
            .status_text(),
            "the agent did not answer within 15 s"
        );
        assert_eq!(exit(2, "").status_text(), "the agent exited with code 2");
        assert_eq!(exit(2, "bad flag").status_text(), "bad flag");
    }

    #[test]
    fn a_certificate_or_http_error_in_the_whole_stderr_replaces_the_reason() {
        let failure = exit(
            1,
            "error: self signed\nSELF_SIGNED_CERT_IN_CHAIN\nat host\n",
        );
        assert_eq!(failure.reason(), "at host");
        assert_eq!(
            failure.reason_for(false),
            explanation(
                ProxyProblem::Certificate {
                    marker: "SELF_SIGNED_CERT_IN_CHAIN"
                },
                false
            )
        );
        assert!(failure
            .reason_for(false)
            .contains("turning on System Certificates"));
        assert!(!failure
            .reason_for(true)
            .contains("turning on System Certificates"));
        assert!(failure
            .reason_for(true)
            .contains("even with System Certificates on"));
        assert_eq!(
            exit(22, "request failed: HTTP 407\nretrying").reason_for(true),
            "the agent received an HTTP 407 error; a corporate proxy that inspects HTTPS traffic may be blocking the request"
        );
    }

    #[test]
    fn the_status_text_of_missing_is_replaced_the_same_way() {
        let failure = exit(1, "curl: (60) SSL certificate problem\nclosing");
        assert_eq!(failure.status_text(), "closing");
        assert_eq!(
            failure.status_text_for(false),
            explain("curl: (60) SSL certificate problem", false).unwrap()
        );
    }

    #[test]
    fn other_failures_keep_their_reason() {
        assert_eq!(exit(2, "bad flag").reason_for(false), "bad flag");
        assert_eq!(exit(2, "HTTP 404").reason_for(false), "HTTP 404");
        assert_eq!(exit(2, "").reason_for(true), "exit code 2");
        assert_eq!(Failure::NotStarted.reason_for(false), "could not start the agent");
        assert_eq!(
            Failure::TimedOut {
                limit: Duration::from_secs(15)
            }
            .status_text_for(false),
            "the agent did not answer within 15 s"
        );
    }

    fn http(text: &str) -> Option<u16> {
        match classify(text) {
            Some(ProxyProblem::Http { code }) => Some(code),
            _ => None,
        }
    }

    #[test]
    fn every_certificate_code_and_phrase_is_a_certificate_error() {
        for code in CERTIFICATE_CODES {
            assert_eq!(
                classify(&format!("fetch failed: {code} at host")),
                Some(ProxyProblem::Certificate { marker: code }),
                "{code}"
            );
        }
        for phrase in CERTIFICATE_PHRASES {
            for text in [
                phrase.to_owned(),
                phrase.to_uppercase(),
                format!("Error: {phrase}."),
            ] {
                assert_eq!(
                    classify(&text),
                    Some(ProxyProblem::Certificate { marker: phrase }),
                    "{text}"
                );
            }
        }
        assert_eq!(
            classify("Error: Self-Signed Certificate in certificate chain"),
            Some(ProxyProblem::Certificate {
                marker: "self-signed certificate in certificate chain"
            })
        );
    }

    #[test]
    fn certificate_codes_match_their_capitals_only() {
        assert_eq!(classify("unable_to_get_issuer_cert_locally"), None);
        assert_eq!(classify("Cert_Has_Expired"), None);
        assert_eq!(classify("CERT_HAS_EXPIRE"), None);
    }

    #[test]
    fn the_longer_code_is_the_one_reported() {
        assert_eq!(
            classify("UNABLE_TO_GET_ISSUER_CERT_LOCALLY"),
            Some(ProxyProblem::Certificate {
                marker: "UNABLE_TO_GET_ISSUER_CERT_LOCALLY"
            })
        );
        assert_eq!(
            classify("UNABLE_TO_GET_ISSUER_CERT"),
            Some(ProxyProblem::Certificate {
                marker: "UNABLE_TO_GET_ISSUER_CERT"
            })
        );
    }

    #[test]
    fn every_http_form_and_code_is_an_http_error() {
        let codes = [
            403, 407, 500, 501, 502, 503, 504, 505, 506, 507, 508, 509, 510, 511, 599,
        ];
        for code in codes {
            for form in [
                format!("HTTP {code}"),
                format!("http {code}"),
                format!("HTTP/1.1 {code}"),
                format!("HTTP/2 {code}"),
                format!("status {code}"),
                format!("Status: {code}"),
                format!("status code {code}"),
                format!("statusCode: {code}"),
                format!("returned error: {code}"),
                format!("Request failed, got HTTP {code}."),
                format!("({code} again)\nHTTP {code}"),
            ] {
                assert_eq!(http(&form), Some(code), "{form}");
            }
        }
    }

    #[test]
    fn every_standard_phrase_gives_its_code() {
        for (code, phrase) in [
            (403, "Forbidden"),
            (407, "Proxy Authentication Required"),
            (500, "Internal Server Error"),
            (501, "Not Implemented"),
            (502, "Bad Gateway"),
            (503, "Service Unavailable"),
            (504, "Gateway Timeout"),
            (505, "HTTP Version Not Supported"),
            (506, "Variant Also Negotiates"),
            (507, "Insufficient Storage"),
            (508, "Loop Detected"),
            (510, "Not Extended"),
            (511, "Network Authentication Required"),
        ] {
            for text in [
                format!("{code} {phrase}"),
                format!("got {code} {} from the proxy", phrase.to_lowercase()),
                format!("<h1>{code} {phrase}</h1>"),
                format!("{code} {phrase}."),
            ] {
                assert_eq!(http(&text), Some(code), "{text}");
            }
        }
        // A phrase of another code, or words that are not the phrase.
        assert_eq!(http("403 Bad Gateway"), None);
        assert_eq!(http("502 something went wrong"), None);
        assert_eq!(http("503 Service Unavailables"), None);
        assert_eq!(http("502Bad Gateway"), None);
    }

    #[test]
    fn the_first_match_in_the_text_gives_the_code() {
        assert_eq!(http("HTTP 503 after 502 Bad Gateway"), Some(503));
        assert_eq!(http("502 Bad Gateway, then HTTP 403"), Some(502));
        assert_eq!(http("HTTP 404 then status 407"), Some(407));
        assert_eq!(http("code 503 and HTTP 504"), Some(504));
    }

    #[test]
    fn other_codes_and_bare_numbers_are_not_http_errors() {
        for text in [
            "HTTP 401",
            "HTTP 404",
            "status: 429",
            "HTTP 200",
            "HTTP 600",
            "HTTP 499",
            "HTTP 402",
            "503",
            "exit code 403",
            "error 500 happened",
            "403 files, 407 lines",
            "HTTP 4030",
            "HTTP 5030",
            "HTTP 1503",
            "status 5003",
            "read 5000 bytes",
            "v1.503",
            "HTTP 503.2",
            "xstatus 503",
            "https 503",
            "HTTP:503",
            "",
        ] {
            assert_eq!(classify(text), None, "{text}");
        }
    }

    #[test]
    fn the_certificate_wins_over_the_http_code_wherever_it_stands() {
        assert_eq!(
            classify("HTTP 502 then CERT_HAS_EXPIRED"),
            Some(ProxyProblem::Certificate {
                marker: "CERT_HAS_EXPIRED"
            })
        );
        assert_eq!(
            classify("curl: (60) first\nHTTP 403"),
            Some(ProxyProblem::Certificate {
                marker: "curl: (60)"
            })
        );
    }

    #[test]
    fn the_whole_text_is_tested_not_only_its_last_line() {
        let stderr =
            "fetch failed\n  at node:internal\n  code: 'SELF_SIGNED_CERT_IN_CHAIN'\nretrying later";
        assert!(matches!(
            classify(stderr),
            Some(ProxyProblem::Certificate { .. })
        ));
        assert_eq!(http("HTTP 502\nsome other line\nlast line"), Some(502));
    }

    #[test]
    fn the_explanations_are_the_exact_texts() {
        let certificate = ProxyProblem::Certificate {
            marker: "CERT_HAS_EXPIRED",
        };
        assert_eq!(
            explanation(certificate, false),
            "the agent could not verify a server's certificate; a corporate proxy that inspects HTTPS traffic is the usual cause, and turning on System Certificates in Weaver's settings may fix it"
        );
        assert_eq!(
            explanation(certificate, true),
            "the agent could not verify a server's certificate, even with System Certificates on; a corporate proxy that inspects HTTPS traffic is the usual cause"
        );
        let refusal = ProxyProblem::Http { code: 407 };
        assert_eq!(
            explanation(refusal, false),
            "the agent received an HTTP 407 error; a corporate proxy that inspects HTTPS traffic may be blocking the request, and turning on System Certificates in Weaver's settings may fix it"
        );
        assert_eq!(
            explanation(refusal, true),
            "the agent received an HTTP 407 error; a corporate proxy that inspects HTTPS traffic may be blocking the request"
        );
    }

    #[test]
    fn explain_answers_only_for_a_proxy_problem() {
        assert_eq!(explain("HTTP 404", false), None);
        assert_eq!(
            explain("HTTP 503", true).as_deref(),
            Some("the agent received an HTTP 503 error; a corporate proxy that inspects HTTPS traffic may be blocking the request")
        );
        assert_eq!(
            explain("SSL certificate problem", false),
            Some(explanation(
                ProxyProblem::Certificate {
                    marker: "ssl certificate problem"
                },
                false
            ))
        );
    }

    #[test]
    fn the_failed_installer_text_is_by_exit_code() {
        for on in [false, true] {
            assert_eq!(
                installer_failed_text(90, on),
                "The agent installer failed (exit code 90): the download of the agent was blocked on both of its download sites, probably by a corporate proxy that filters downloads. See the terminal."
            );
            assert_eq!(
                installer_failed_text(92, on),
                "The agent installer failed (exit code 92): the downloaded agent was incomplete or altered, so it was not installed; a proxy may have changed it. See the terminal."
            );
            assert_eq!(
                installer_failed_text(93, on),
                "The agent installer failed (exit code 93): the downloaded agent does not start. See the terminal."
            );
        }
        assert_eq!(
            installer_failed_text(91, true),
            "The agent installer failed (exit code 91): the agent could not verify a server's certificate; a corporate proxy that inspects HTTPS traffic is the usual cause. See the terminal."
        );
        assert_eq!(
            installer_failed_text(91, false),
            "The agent installer failed (exit code 91): the agent could not verify a server's certificate; a corporate proxy that inspects HTTPS traffic is the usual cause. See the terminal. Turning on System Certificates in Weaver's settings may fix it."
        );
    }

    #[test]
    fn the_failed_installer_text_of_any_other_code_depends_on_the_setting() {
        assert_eq!(
            installer_failed_text(22, true),
            "The agent installer failed (exit code 22); see the terminal. A corporate proxy that inspects HTTPS traffic is a common cause."
        );
        assert_eq!(
            installer_failed_text(1, false),
            "The agent installer failed (exit code 1); see the terminal. A corporate proxy that inspects HTTPS traffic is a common cause. Turning on System Certificates in Weaver's settings may fix it."
        );
        // 94 is only for updates: for an installer it is any other code.
        assert_eq!(
            installer_failed_text(94, true),
            "The agent installer failed (exit code 94); see the terminal. A corporate proxy that inspects HTTPS traffic is a common cause."
        );
    }

    #[test]
    fn the_download_of_an_update_fails_with_a_reason_by_code() {
        for on in [false, true] {
            assert_eq!(
                download_failed_reason(DownloadFailure::Exit(90), on),
                "the download was blocked, probably by a corporate proxy that filters downloads"
            );
            assert_eq!(
                download_failed_reason(DownloadFailure::Exit(92), on),
                "the downloaded agent was incomplete or altered"
            );
            assert_eq!(
                download_failed_reason(DownloadFailure::Exit(93), on),
                "the downloaded agent did not start"
            );
            assert_eq!(
                download_failed_reason(DownloadFailure::Exit(7), on),
                "the download of the agent failed (exit code 7)"
            );
            assert_eq!(
                download_failed_reason(DownloadFailure::TimedOut, on),
                "the download of the agent timed out"
            );
        }
    }

    #[test]
    fn the_certificate_reason_of_a_download_follows_the_setting_as_it_is() {
        assert_eq!(
            download_failed_reason(DownloadFailure::Exit(91), false),
            "the agent could not verify a server's certificate; a corporate proxy that inspects HTTPS traffic is the usual cause, and turning on System Certificates in Weaver's settings may fix it"
        );
        assert_eq!(
            download_failed_reason(DownloadFailure::Exit(91), true),
            "the agent could not verify a server's certificate, even with System Certificates on; a corporate proxy that inspects HTTPS traffic is the usual cause"
        );
    }

    #[test]
    fn no_cause_text_ends_with_a_full_stop_in_a_reason_or_names_a_product() {
        let mut texts = Vec::new();
        for on in [false, true] {
            for code in [90, 91, 92, 93, 7] {
                texts.push((
                    download_failed_reason(DownloadFailure::Exit(code), on),
                    false,
                ));
                texts.push((installer_failed_text(code, on), true));
            }
            texts.push((download_failed_reason(DownloadFailure::TimedOut, on), false));
        }
        for (text, stops) in texts {
            assert_eq!(text.ends_with('.'), stops, "{text}");
            let lower = text.to_lowercase();
            for product in ["github", "zscaler", "netskope", "cisco", "cloudflare"] {
                assert!(!lower.contains(product), "{text}");
            }
        }
    }

    #[test]
    fn no_explanation_ends_with_a_full_stop_or_names_a_product() {
        let problems = [
            ProxyProblem::Certificate { marker: "x" },
            ProxyProblem::Http { code: 502 },
        ];
        for problem in problems {
            for on in [false, true] {
                let text = explanation(problem, on);
                assert!(!text.ends_with('.'), "{text}");
                let lower = text.to_lowercase();
                for product in [
                    "zscaler",
                    "netskope",
                    "forcepoint",
                    "fortinet",
                    "cisco",
                    "palo alto",
                    "cloudflare",
                    "symantec",
                    "blue coat",
                ] {
                    assert!(!lower.contains(product), "{text}");
                }
            }
        }
    }
}
