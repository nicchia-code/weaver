//! The text of the user's `Path` on Windows: `;`-separated entries, compared the way the system
//! compares folders (variables expanded, case ignored, a trailing `\` ignored).
//!
//! Pure text, so every rule is tested on any system; the registry itself is in `registry`.

/// Splits `Path` into its entries, as written (empty ones included).
pub fn entries(value: &str) -> impl Iterator<Item = &str> {
    value.split(';')
}

/// The form two entries are compared in: variables expanded, a trailing `\` removed, lowercase.
fn comparable(entry: &str, expand: &dyn Fn(&str) -> String) -> String {
    expand(entry.trim())
        .trim_end_matches('\\')
        .to_lowercase()
}

/// Whether `value` holds `dir`.
pub fn contains(value: &str, dir: &str, expand: &dyn Fn(&str) -> String) -> bool {
    let wanted = comparable(dir, expand);
    entries(value).any(|entry| !entry.trim().is_empty() && comparable(entry, expand) == wanted)
}

/// `value` with `dir` added at the end, every other entry untouched.
pub fn with_added(value: &str, dir: &str) -> String {
    if value.trim().is_empty() {
        dir.to_owned()
    } else if value.ends_with(';') {
        format!("{value}{dir}")
    } else {
        format!("{value};{dir}")
    }
}

/// `value` without the entries that are `dir`, every other entry untouched.
pub fn without(value: &str, dir: &str, expand: &dyn Fn(&str) -> String) -> String {
    let wanted = comparable(dir, expand);
    entries(value)
        .filter(|entry| entry.trim().is_empty() || comparable(entry, expand) != wanted)
        .collect::<Vec<_>>()
        .join(";")
}

/// Replaces each `%NAME%` of `text` with what `lookup` gives for `NAME`; a name `lookup` does not
/// know, and a lone `%`, stay as written (the system does the same).
pub fn expand_variables(text: &str, lookup: &dyn Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('%') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('%') {
            Some(close) if close > 0 => {
                let name = &after[..close];
                match lookup(name) {
                    Some(value) => out.push_str(&value),
                    None => {
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                rest = &after[close + 1..];
            }
            _ => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none(text: &str) -> String {
        text.to_owned()
    }

    fn with_local_app_data(text: &str) -> String {
        expand_variables(text, &|name| {
            name.eq_ignore_ascii_case("LOCALAPPDATA").then(|| r"C:\Users\Ada\AppData\Local".to_owned())
        })
    }

    #[test]
    fn a_folder_is_found_ignoring_case_and_a_trailing_backslash() {
        let value = r"C:\Tools;c:\users\ada\appdata\local\dev.nicchia.weaver\BIN\;D:\Other";
        let dir = r"C:\Users\Ada\AppData\Local\dev.nicchia.weaver\bin";
        assert!(contains(value, dir, &none));
    }

    #[test]
    fn a_folder_is_found_after_expanding_variables() {
        let value = r"%LOCALAPPDATA%\dev.nicchia.weaver\bin;C:\Tools";
        let dir = r"C:\Users\Ada\AppData\Local\dev.nicchia.weaver\bin";
        assert!(contains(value, dir, &with_local_app_data));
        assert!(!contains(value, dir, &none));
    }

    #[test]
    fn another_folder_or_a_longer_one_is_not_the_folder() {
        let dir = r"C:\Weaver\bin";
        assert!(!contains(r"C:\Weaver\bin2;C:\Weaver", dir, &none));
        assert!(!contains(r"C:\Weaver\bin\sub", dir, &none));
        assert!(!contains("", dir, &none));
        assert!(!contains(";;", dir, &none));
    }

    #[test]
    fn adding_appends_and_keeps_every_other_entry() {
        assert_eq!(with_added("", r"C:\W\bin"), r"C:\W\bin");
        assert_eq!(with_added(r"C:\A;%X%\B", r"C:\W\bin"), r"C:\A;%X%\B;C:\W\bin");
        assert_eq!(with_added(r"C:\A;", r"C:\W\bin"), r"C:\A;C:\W\bin");
    }

    #[test]
    fn removing_deletes_only_that_entry() {
        let value = r"C:\A;C:\W\bin;%X%\B;c:\w\BIN\";
        assert_eq!(without(value, r"C:\W\bin", &none), r"C:\A;%X%\B");
        assert_eq!(without(r"C:\A;%X%\B", r"C:\W\bin", &none), r"C:\A;%X%\B");
        assert_eq!(without(r"C:\W\bin", r"C:\W\bin", &none), "");
    }

    #[test]
    fn adding_then_removing_gives_back_the_original_value() {
        let original = r"%SystemRoot%\system32;C:\Tools\";
        let added = with_added(original, r"C:\W\bin");
        assert_eq!(without(&added, r"C:\W\bin", &none), original);
    }

    #[test]
    fn variables_are_expanded_and_unknown_ones_stay() {
        let lookup = |name: &str| (name == "A").then(|| "1".to_owned());
        assert_eq!(expand_variables("%A%\\x", &lookup), "1\\x");
        assert_eq!(expand_variables("%B%\\x", &lookup), "%B%\\x");
        assert_eq!(expand_variables("50%", &lookup), "50%");
        assert_eq!(expand_variables("%%A%", &lookup), "%1");
        assert_eq!(expand_variables("a%A%b%A%c", &lookup), "a1b1c");
    }
}
