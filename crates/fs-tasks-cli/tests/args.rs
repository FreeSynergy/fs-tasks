//! The words of the command line: every spelling without a server.

use fs_tasks_cli::{Command, NewTask, Query, To, Work, parse};

fn words(s: &str) -> Vec<String> {
    s.split_whitespace().map(String::from).collect()
}

fn command(s: &str) -> Command {
    parse(&words(s)).unwrap().command.unwrap()
}

#[test]
fn the_options_come_before_the_command() {
    let a = parse(&words("--socket /x/y --data /d list")).unwrap();
    assert_eq!((a.socket, a.data), (Some("/x/y".into()), Some("/d".into())));
    assert_eq!(a.command, Some(Command::List(Query::default())));
    assert_eq!(command("--socket /s show abc"), Command::Show("abc".into()));
    assert_eq!(parse(&words("-h")).unwrap().command, None);
}

#[test]
fn add_takes_a_title_and_options() {
    let c = command(
        "add Plan --due 2026-12-01T10:00:00 --zone Europe/Berlin --label work --project p --objective true --as w1 --json",
    );
    assert_eq!(
        c,
        Command::Add(NewTask {
            title: "Plan".into(),
            due: Some("2026-12-01T10:00:00".into()),
            zone: Some("Europe/Berlin".into()),
            label: Some("work".into()),
            project: Some("p".into()),
            objective: Some("true".into()),
            subjective: false,
            creator: Some("w1".into()),
            json: true,
        })
    );
    assert!(c.json());
}

#[test]
fn after_the_command_every_word_is_the_commands_own() {
    // `-h` and `--socket` are values here, not options.
    assert!(matches!(command("add -h"), Command::Add(n) if n.title == "-h"));
    assert_eq!(command("show --socket"), Command::Show("--socket".into()));
}

#[test]
fn the_worker_commands() {
    let w = |c: Command| match c {
        Command::Work { work, json } => (work, json),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        w(command("claim T1 --as w --until 2h --json")),
        (
            Work::Claim {
                id: "T1".into(),
                name: "w".into(),
                until: Some("2h".into())
            },
            true
        )
    );
    assert_eq!(
        w(command("release T1 --as w")).0,
        Work::Release {
            id: "T1".into(),
            name: "w".into()
        }
    );
    assert_eq!(
        w(command("next --as w --project p")).0,
        Work::Next {
            name: "w".into(),
            project: Some("p".into())
        }
    );
    assert_eq!(
        w(command("status T1 failed --note x --report /r --cost 1.5")).0,
        Work::Status {
            id: "T1".into(),
            to: To::Failed,
            note: Some("x".into()),
            report: Some("/r".into()),
            cost: Some("1.5".into()),
        }
    );
    assert_eq!(
        w(command("note T1 hello")).0,
        Work::Note {
            id: "T1".into(),
            text: "hello".into()
        }
    );
    assert_eq!(w(command("check T1")).0, Work::Check("T1".into()));
    assert_eq!(
        command("list --project p --status failed --json"),
        Command::List(Query {
            project: Some("p".into()),
            status: Some("failed".into()),
            json: true
        })
    );
}

#[test]
fn mistakes_are_named() {
    for bad in [
        "",
        "frob",
        "show",
        "show a b",
        "add",
        "add --due x",
        "add T --due",
        "add T --x 1",
        "add T --zone Z",
        "add T --objective true --subjective",
        "--socket",
        "claim T1",
        "claim --as w",
        "release T1",
        "next",
        "status T1",
        "status T1 paused",
        "status T1 done --cost -1",
        "status T1 done --cost lots",
        "note T1",
        "note T1 two words",
        "check",
        "list --status paused",
        "list extra",
    ] {
        assert!(parse(&words(bad)).is_err(), "{bad:?}");
    }
}
