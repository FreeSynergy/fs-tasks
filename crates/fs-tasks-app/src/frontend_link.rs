//! TEMPORARY: the description and the findings in the types of the shipped Admin frontend.
//!
//! Pattern: Adapter. `fs-admin-frontend` rev 4688cee is built on `fs-types` v0.0.2, the core reports v0.0.11 (it has
//! `supported` and the `Edit` of kind `change`). Cargo cannot unify two tags of one repository, so the types are two
//! sets of the same shape; this file converts field by field. It also gives the frontend what it cannot read from a
//! v0.0.11 description: the form of `change` (it does not know `Edit`: the record id and the fields of the type) and
//! drops `supported` (it cannot show the lock; the backend refuses such a command, see `DESIGN.md`).
//! Delete this file when Thread A's frontend is tagged on the new `fs-types`.

use fs_admin_frontend as front;
use fs_interfaces::values::ValueProblem as ValueProblemNew;
use fs_interfaces_frontend::values::ValueProblem as ValueProblemOld;
use fs_types as new;
use fs_types_frontend as old;

fn basic(b: new::BasicType) -> old::BasicType {
    old::BasicType::try_from(u16::from(b)).unwrap_or(old::BasicType::Unknown)
}

fn kind(k: new::CommandKind) -> old::CommandKind {
    old::CommandKind::try_from(u16::from(k)).unwrap_or(old::CommandKind::Unknown)
}

fn field(f: &new::FieldDescription) -> old::FieldDescription {
    let mut out = old::FieldDescription::new(&f.name, basic(f.basic)).label(&f.label);
    out.required = f.required;
    out.multiple = f.multiple;
    out.target.clone_from(&f.target);
    out.choices.clone_from(&f.choices);
    out
}

/// The description for the shipped frontend. `hidden` are fields of the type the form of `change` does not offer.
pub fn description(d: &new::TypeDescription, hidden: &[&str]) -> old::TypeDescription {
    let mut out = old::TypeDescription::new(&d.name, &d.program, &d.standard, &d.label);
    out.fields = d.fields.iter().map(field).collect();
    for c in &d.commands {
        let mut command = old::CommandDescription::new(&c.name, kind(c.kind), &c.label);
        command.input = c.input.iter().map(field).collect();
        command.result = c.result.iter().map(field).collect();
        if c.kind == new::CommandKind::Change && c.input.is_empty() {
            let id = old::FieldDescription::new("id", old::BasicType::Reference)
                .target(&d.name)
                .required()
                .label(&format!("{}.id", d.label));
            command.input.push(id);
            let shown = d
                .fields
                .iter()
                .filter(|f| !hidden.contains(&f.name.as_str()));
            command.input.extend(shown.map(field));
        }
        out.commands.push(command);
    }
    out
}

fn value_problem(p: ValueProblemNew) -> ValueProblemOld {
    use ValueProblemNew as N;
    use ValueProblemOld as O;
    match p {
        N::Empty => O::Empty,
        N::MissingAt => O::MissingAt,
        N::ManyAt => O::ManyAt,
        N::EmptyLocal => O::EmptyLocal,
        N::EmptyDomain => O::EmptyDomain,
        N::LocalCharacter => O::LocalCharacter,
        N::LocalTooLong => O::LocalTooLong,
        N::BadDomain => O::BadDomain,
        N::DomainWithoutDot => O::DomainWithoutDot,
        N::NoCountryCode => O::NoCountryCode,
        N::PhoneCharacter => O::PhoneCharacter,
        N::AddressTooLong => O::AddressTooLong,
        N::PhoneLength => O::PhoneLength,
    }
}

fn type_problem(p: new::TypeProblem) -> old::TypeProblem {
    use new::TypeProblem as N;
    use old::TypeProblem as O;
    match p {
        N::Empty => O::Empty,
        N::TooLong => O::TooLong,
        N::Character => O::Character,
        N::Contact(v) => O::Contact(value_problem(v)),
        N::BadLanguage => O::BadLanguage,
        N::BadUri => O::BadUri,
        N::UnsafeScheme => O::UnsafeScheme,
        N::NotAbsolute => O::NotAbsolute,
        N::BadDate => O::BadDate,
        N::BadDateTime => O::BadDateTime,
        N::BadPlace => O::BadPlace,
        N::PlaceRange => O::PlaceRange,
        N::BadFingerprint => O::BadFingerprint,
        N::NotInList => O::NotInList,
        N::NotImage => O::NotImage,
        N::BadKeyword => O::BadKeyword,
        N::NoParts => O::NoParts,
        N::BadCountryCode => O::BadCountryCode,
    }
}

/// A finding in the types of the shipped frontend. The three problems only an `Edit` can have (wrong type, nested
/// path, duplicate path) cannot come from its form; they are shown as an unknown field.
pub fn issue(i: &new::RecordIssue) -> old::RecordIssue {
    use new::RecordProblem as N;
    use old::RecordProblem as O;
    let problem = match i.problem {
        N::UnknownField | N::WrongType | N::NestedPath | N::DuplicatePath => O::UnknownField,
        N::Missing => O::Missing,
        N::NotMultiple => O::NotMultiple,
        N::Value(v) => O::Value(type_problem(v)),
    };
    old::RecordIssue {
        field: i.field.clone(),
        index: i.index,
        problem,
    }
}

/// The refusal of the frontend for these findings.
pub fn rejected(issues: &[new::RecordIssue]) -> front::BackendError {
    front::BackendError::Rejected(issues.iter().map(issue).collect())
}

#[cfg(test)]
#[path = "frontend_link_tests.rs"]
mod tests;
