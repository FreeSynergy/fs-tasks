//! Keeping secrets out of tasks.
//!
//! Pattern: Specification — a few obvious marks of a key, a token or a private key. This is a guard against a slip
//! of a worker, not a scanner: a secret in other words passes. A task is shared; a secret in it is published.

const MARKS: [&str; 8] = [
    "sk-ant-",
    "ghp_",
    "gho_",
    "github_pat_",
    "glpat-",
    "xoxb-",
    "-----BEGIN",
    "Bearer ",
];

/// Refuse `text` (named `what` in the message) if it carries an obvious secret. The message names the mark, never
/// the text.
///
/// # Errors
/// The text holds one of the marks.
pub fn refuse_secret(what: &str, text: &str) -> Result<(), String> {
    match MARKS.iter().find(|m| text.contains(**m)) {
        Some(mark) => Err(format!(
            "{what} looks like it holds a secret (`{}`); not stored: tasks are shared",
            mark.trim_end()
        )),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_marks_are_refused_and_plain_words_are_not() {
        for bad in [
            "key sk-ant-abc",
            "ghp_x",
            "-----BEGIN PRIVATE KEY-----",
            "Authorization: Bearer abc",
        ] {
            let e = refuse_secret("note", bad).unwrap_err();
            assert!(e.starts_with("note looks like"), "{e}");
            assert!(!e.contains("abc"), "the secret is not echoed: {e}");
        }
        assert!(refuse_secret("note", "bearer of news, ask ant").is_ok());
    }
}
