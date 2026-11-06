//! Labels as one line of text in the form.
//!
//! Pattern: Value parser — separated by commas; a label that holds a comma or a quote stands in quotes,
//! a quote in it doubled, so every label can be read back as itself and be taken out or put in.

use std::collections::BTreeSet;

/// The line for these labels.
pub(super) fn write_labels(labels: &[String]) -> String {
    let one = |label: &String| {
        if label.contains([',', '"']) {
            format!("\"{}\"", label.replace('"', "\"\""))
        } else {
            label.clone()
        }
    };
    labels.iter().map(one).collect::<Vec<_>>().join(", ")
}

/// The labels in this line. A quote that is never closed takes the rest of the line.
pub(super) fn read_labels(line: &str) -> BTreeSet<String> {
    let mut labels = BTreeSet::new();
    let mut chars = line.chars().peekable();
    loop {
        while chars.next_if(|c| c.is_whitespace() || *c == ',').is_some() {}
        let Some(first) = chars.peek().copied() else {
            return labels;
        };
        let mut label = String::new();
        if first == '"' {
            chars.next();
            while let Some(c) = chars.next() {
                if c != '"' {
                    label.push(c);
                } else if chars.next_if_eq(&'"').is_some() {
                    label.push('"');
                } else {
                    break;
                }
            }
        } else {
            while let Some(c) = chars.next_if(|c| *c != ',') {
                label.push(c);
            }
        }
        let label = label.trim();
        if !label.is_empty() {
            labels.insert(label.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_label_reads_back_as_itself() {
        let labels: Vec<String> = ["a b", "with,comma", "say \"hi\"", "\"", ","]
            .iter()
            .map(ToString::to_string)
            .collect();
        let back = read_labels(&write_labels(&labels));
        assert_eq!(back, labels.iter().cloned().collect());
    }

    #[test]
    fn what_a_person_types_is_read_kindly() {
        let got = read_labels(" a ,, b,\"c, d\" , \"open, end");
        let want: BTreeSet<String> = ["a", "b", "c, d", "open, end"]
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(got, want);
        assert!(read_labels("  ,  ").is_empty());
    }
}
