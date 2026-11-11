//! Task model: lossless round trip, validation, 1.0 -> 2.0 upgrade.

use fs_tasks_core::validate::{issues, validate};
use fs_tasks_core::{Task, upgrade};
use serde_json::json;

#[test]
fn unknown_and_vendor_properties_survive() {
    let v = json!({"@type":"Task","version":"2.0","uid":"t","due":"2026-01-01T10:00:00",
        "freesynergy.net:taskStatus":"waiting","percentComplete":10,"future":{"a":1},
        "locations":{"l":{"name":"Here","example.com:geo":[1,2]}},
        "participants":{"p":{"name":"N","sendTo":{"imip":"mailto:n@x.org"},"vendor:x":true}},
        "relatedTo":{"other":{"@type":"Relation","relation":{"depends-on":true}}}});
    let t = Task::from_value(v.clone()).unwrap();
    assert_eq!(t.to_value(), v);
    assert_eq!(t.task_status(), Some("waiting"));
    validate(&t).unwrap();
    // already 2.0: the upgrade changes nothing
    let up = upgrade(v.clone());
    assert_eq!(up.value, v);
    assert_eq!(up.notes, [] as [std::string::String; 0]);
}

#[test]
fn validation_rules() {
    let ok = Task::new("T");
    assert_eq!(issues(&ok), [] as [fs_tasks_core::Issue; 0]);
    let mut bad = ok.clone();
    bad.version = Some("1.0".into());
    bad.percent_complete = Some(101);
    bad.progress = Some("sleeping".into());
    bad.due = Some("2026-11-24T10:00:00Z".into());
    bad.estimated_duration = Some("1h".into());
    bad.progress_updated = Some("yesterday".into());
    bad.time_zone = Some("/Custom/Zone".into());
    bad.recurrence_rule = Some(
        serde_json::from_value(
            json!({"frequency":"daily","count":3,"until":"2026-12-01T00:00:00"}),
        )
        .unwrap(),
    );
    bad.recurrence_overrides.insert("nope".into(), json!({}));
    bad.extra.insert("recurrenceRules".into(), json!([]));
    bad.assign("B", "b@x.org");
    bad.participants.get_mut("p1").unwrap().participation_status = Some("yes".into());
    let paths: Vec<_> = issues(&bad).into_iter().map(|i| i.path).collect();
    for p in [
        "/version",
        "/percentComplete",
        "/progress",
        "/due",
        "/estimatedDuration",
        "/progressUpdated",
        "/timeZone",
        "/recurrenceRule",
        "/recurrenceOverrides/nope",
        "/recurrenceRules",
        "/participants/p1/participationStatus",
    ] {
        assert!(paths.iter().any(|x| x == p), "{p} in {paths:?}");
    }
    let mut wrong = Task::new("T");
    wrong.type_ = Some("Event".into());
    wrong.uid = " ".into();
    let paths: Vec<_> = issues(&wrong).into_iter().map(|i| i.path).collect();
    assert!(paths.contains(&"/@type".to_string()) && paths.contains(&"/uid".to_string()));
}

#[test]
fn commands_edit_the_value() {
    use fs_tasks_core::Relation;
    let mut t = Task::new("T");
    let k = t.assign("Ben", "Ben@Example.org");
    assert_eq!(
        t.assign("Ben", "ben@example.org"),
        k,
        "same address, same participant"
    );
    assert_eq!(t.participants.len(), 1);
    assert_eq!(t.participants[&k].roles.get("owner"), Some(&true));
    assert_eq!(t.participant_by_email("mailto:BEN@example.org"), Some("p1"));
    t.relate("urn:uuid:other", Relation::DependsOn);
    t.relate("urn:uuid:other", Relation::FinishToStart);
    assert_eq!(
        t.related_to["urn:uuid:other"],
        json!({"@type":"Relation","relation":{"depends-on":true,"finishtostart":true}})
    );
    assert_eq!(
        t.relations(),
        [
            ("urn:uuid:other".to_string(), Relation::DependsOn),
            ("urn:uuid:other".to_string(), Relation::FinishToStart)
        ]
    );
    t.label("urgent");
    assert_eq!(t.keywords.get("urgent"), Some(&true));
    t.set_task_status("pending");
    assert_eq!(
        t.vendor("freesynergy.net:taskStatus"),
        Some(&json!("pending"))
    );
    validate(&t).unwrap();
    for r in Relation::ALL {
        assert_eq!(Relation::parse(r.as_str()), Some(r));
    }
    assert_eq!(Relation::parse("sibling"), None);
    assert_eq!(
        Relation::parse("FINISHTOSTART"),
        Some(Relation::FinishToStart)
    );
    assert_eq!(Relation::parse("DEPENDS-ON"), Some(Relation::DependsOn));
    // an imported capital spelling and a new relation of the same kind are one key
    let mut u = Task::new("U");
    u.related_to.insert(
        "x".into(),
        json!({"@type":"Relation","relation":{"FINISHTOSTART":true}}),
    );
    u.relate("x", Relation::FinishToStart);
    assert_eq!(u.related_to["x"]["relation"].as_object().unwrap().len(), 1);
}

#[test]
fn upgrade_from_rfc_8984() {
    let up = upgrade(
        json!({"@type":"Task","uid":"t","start":"2026-11-01T08:00:00","due":"2026-11-12T17:00:00Z",
        "recurrenceRules":[{"frequency":"weekly","count":3},{"frequency":"monthly"}],
        "participants":{"p1":{"name":"B","language":"de"}}}),
    );
    let v = &up.value;
    assert_eq!(
        (
            v["due"].as_str(),
            v["timeZone"].as_str(),
            v["version"].as_str()
        ),
        (Some("2026-11-12T17:00:00"), Some("Etc/UTC"), Some("2.0"))
    );
    assert_eq!(v["recurrenceRule"], json!({"frequency":"weekly","count":3}));
    assert_eq!(
        v["freesynergy.net:legacy"]["recurrenceRules"],
        json!([{"frequency":"monthly"}])
    );
    assert_eq!(
        v["freesynergy.net:legacy"]["participants/p1/language"],
        "de"
    );
    assert!(
        up.notes
            .iter()
            .any(|n| n.contains("further recurrence rule"))
    );
    let task = Task::from_value(v.clone()).unwrap();
    validate(&task).unwrap();
    assert_eq!(upgrade(up.value.clone()).value, up.value, "idempotent");
    // custom zone
    let up = upgrade(
        json!({"@type":"Task","uid":"c","due":"2026-01-01T10:00:00","timeZone":"/Custom/Zone"}),
    );
    assert!(up.value.get("timeZone").is_none());
    assert_eq!(
        up.value["freesynergy.net:legacy"]["timeZone"],
        "/Custom/Zone"
    );
    // fractions
    let up = upgrade(json!({"@type":"Task","uid":"f","due":"2026-01-01T10:00:00.500"}));
    assert_eq!(up.value["due"], "2026-01-01T10:00:00");
}
