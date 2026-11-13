//! Assignees that are actors, the execution fields and claims.

mod common;

use chrono::{Duration, Utc};
use common::repo;
use fs_tasks_core::model::{ActorType, LoopPass, actor_type_of, exec_issues, is_non_human};
use fs_tasks_core::{Error, Task, TaskItems, TaskQuery, ical};
use serde_json::json;

const RUNNER: &str = "https://local.invalid/actors/runner";
const ADA: &str = "https://local.invalid/people/ada";

#[tokio::test]
async fn an_actor_is_assigned_once_and_found_by_its_iri() {
    let r = repo().await;
    let t = r.create(Task::new("Build")).await.unwrap();
    let e = r
        .assign_actor(t.id, RUNNER, ActorType::Application, "Runner")
        .await
        .unwrap();
    let again = r
        .assign_actor(t.id, RUNNER, ActorType::Service, "CI")
        .await
        .unwrap();
    assert_eq!(e.task.participants.len(), 1);
    assert_eq!(
        again.task.participants.len(),
        1,
        "one participant per actor"
    );
    let p = again.task.participants.values().next().unwrap();
    assert_eq!(actor_type_of(p), Some(ActorType::Service));
    assert_eq!(p.kind.as_deref(), Some("resource"), "exported as non-human");
    r.assign_actor(t.id, ADA, ActorType::Person, "Ada")
        .await
        .unwrap();
    let mine = r
        .query(&TaskQuery::new().assigned_to(RUNNER))
        .await
        .unwrap();
    assert_eq!(mine.len(), 1);
    assert_eq!(
        r.query(&TaskQuery::new().assigned_to("nobody"))
            .await
            .unwrap(),
        []
    );
}

#[test]
fn a_participant_is_read_back_as_far_as_its_kind_says() {
    let mut t = Task::new("x");
    t.assign_actor(ADA, ActorType::Person, "Ada");
    t.assign_actor(RUNNER, ActorType::Application, "Runner");
    let kinds: Vec<_> = t
        .participants
        .values()
        .map(|p| p.kind.clone().unwrap())
        .collect();
    assert_eq!(kinds, ["individual", "resource"]);
    // as another program wrote it: only `kind`, none of our members
    let mut foreign = t.clone();
    for p in foreign.participants.values_mut() {
        p.extra.clear();
    }
    let people: Vec<_> = foreign.participants.values().map(actor_type_of).collect();
    assert_eq!(
        people,
        [Some(ActorType::Person), None],
        "a resource says non-human, not which kind"
    );
    assert!(foreign.participants.values().any(is_non_human));
    assert!(!is_non_human(foreign.participants.values().next().unwrap()));
}

#[test]
fn the_execution_fields_are_checked_for_their_form_only() {
    let mut t = Task::new("Run");
    t.set_run(json!({"call": "install:1@fs", "with": {"app": "wiki"}, "risk": 2}));
    t.set_result(json!({"sha": "abc"}));
    t.set_attempts(2);
    t.set_error("https://freesynergy.net/problems/timeout");
    t.set_claim(RUNNER, Utc::now() + Duration::minutes(5));
    t.set_vendor(
        "freesynergy.net:template",
        json!({"id": "release", "version": "1", "path": "do/0", "index": 0}),
    );
    t.set_loop_pass(&LoopPass {
        id: "p/loop".into(),
        iteration: 2,
        limit: 5,
    });
    assert_eq!(exec_issues(&t), []);
    assert_eq!((t.attempts(), t.loop_pass().unwrap().iteration), (2, 2));

    let mut bad = Task::new("Bad");
    bad.set_run(json!({"with": {}}));
    bad.set_vendor("freesynergy.net:attempts", json!("two"));
    bad.set_error("no uri");
    bad.set_vendor("freesynergy.net:claim", json!({"by": RUNNER}));
    bad.set_vendor(
        "freesynergy.net:loop",
        json!({"id": "x", "iteration": 0, "limit": 3}),
    );
    bad.set_result(json!("x".repeat(70_000)));
    assert_eq!(exec_issues(&bad).len(), 6, "{:?}", exec_issues(&bad));
    assert!(fs_tasks_core::validate::validate(&bad).is_err());
}

#[tokio::test]
async fn a_claim_holds_against_others_until_it_ends() {
    let r = repo().await;
    let t = r.create(Task::new("Take me")).await.unwrap();
    let until = Utc::now() + Duration::minutes(10);
    let first = r.claim(t.id, RUNNER, until).await.unwrap();
    assert_eq!(first.task.claim().unwrap().by, RUNNER);
    // the holder may renew; somebody else is refused and nothing changes
    r.claim(t.id, RUNNER, until + Duration::minutes(1))
        .await
        .unwrap();
    let refused = r.claim(t.id, ADA, until).await;
    assert!(matches!(refused, Err(Error::Conflict(m)) if m.contains(RUNNER)));
    assert!(r.release_claim(t.id, ADA).await.is_err());
    let freed = r.release_claim(t.id, RUNNER).await.unwrap();
    assert!(freed.task.claim().is_none());
    // then ada may take it; an end in the past is refused
    r.claim(t.id, ADA, until).await.unwrap();
    assert!(
        r.claim(t.id, ADA, Utc::now() - Duration::minutes(1))
            .await
            .is_err()
    );
}

#[test]
fn the_fields_survive_the_json_round_trip_and_name_what_the_ical_does_with_them() {
    let mut t = Task::new("Run");
    t.uid = "run-1".into();
    t.set_result(json!({"ok": true}));
    t.assign_actor(RUNNER, ActorType::Application, "Runner");
    let back = Task::from_value(t.to_value()).unwrap();
    assert_eq!(back.result(), t.result());
    assert_eq!(back.actors(), [RUNNER]);
    let ics = ical::to_ics(&[t]).unwrap();
    assert!(ics.contains("UID:run-1"));
    // an assignee without an e-mail address is an attendee of type RESOURCE, not nothing
    assert!(ics.contains("CUTYPE=RESOURCE"), "{ics}");
    let (again, _) = ical::parse_ics(&ics).unwrap();
    assert_eq!(again[0].actors(), [RUNNER]);
    assert_eq!(again[0].result(), Some(&json!({"ok": true})));
    let p = again[0].participants.values().next().unwrap();
    assert_eq!(actor_type_of(p), Some(ActorType::Application));
    assert!(is_non_human(p));
}

/// Two actors claim the same task at the same moment: the first wins, the second gets a conflict that names the holder,
/// and the task carries exactly the winner's claim.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn of_two_simultaneous_claims_one_wins_and_the_other_is_told_who_holds_it() {
    for _round in 0..25 {
        let repo = repo().await;
        let task = repo.create(Task::new("Take me")).await.unwrap();
        let until = Utc::now() + Duration::minutes(10);
        let (one, two) = (repo.clone(), repo.clone());
        let id = task.id;
        let first = tokio::spawn(async move { one.claim(id, RUNNER, until).await });
        let second = tokio::spawn(async move { two.claim(id, ADA, until).await });
        let results = [first.await.unwrap(), second.await.unwrap()];
        let wins: Vec<_> = results.iter().filter(|res| res.is_ok()).collect();
        assert_eq!(wins.len(), 1, "exactly one claim wins");
        let winner = if results[0].is_ok() { RUNNER } else { ADA };
        let loser = results.iter().find_map(|res| res.as_ref().err()).unwrap();
        match loser {
            Error::Conflict(m) => assert!(m.contains(winner), "the conflict names the holder: {m}"),
            other => panic!("{other:?}"),
        }
        let held = repo.get(id).await.unwrap().unwrap().task.claim().unwrap();
        assert_eq!(held.by, winner);
    }
}

#[tokio::test]
async fn an_expired_claim_can_be_taken_over_by_another() {
    let r = repo().await;
    let t = r.create(Task::new("Take me")).await.unwrap();
    r.claim(t.id, RUNNER, Utc::now() + Duration::seconds(1))
        .await
        .unwrap();
    assert!(
        r.claim(t.id, ADA, Utc::now() + Duration::minutes(5))
            .await
            .is_err(),
        "still held"
    );
    tokio::time::sleep(std::time::Duration::from_millis(2200)).await;
    let taken = r
        .claim(t.id, ADA, Utc::now() + Duration::minutes(5))
        .await
        .unwrap();
    assert_eq!(taken.task.claim().unwrap().by, ADA);
}

#[tokio::test]
async fn an_actor_iri_a_name_and_the_end_of_a_claim_are_bounded() {
    let r = repo().await;
    let t = r.create(Task::new("Bounded")).await.unwrap();
    let iri = |n: usize| format!("https://local.invalid/{}", "a".repeat(n - 22));
    assert_eq!(iri(2048).len(), 2048);
    r.assign_actor(t.id, &iri(2048), ActorType::Person, &"n".repeat(200))
        .await
        .unwrap();
    for (actor, name) in [(iri(2049), "x".to_string()), (ADA.into(), "n".repeat(201))] {
        let refused = r.assign_actor(t.id, &actor, ActorType::Person, &name).await;
        assert!(matches!(refused, Err(Error::Conflict(_))), "{refused:?}");
    }
    assert!(matches!(
        r.claim(t.id, &iri(2049), Utc::now() + Duration::minutes(1))
            .await,
        Err(Error::Conflict(_))
    ));
    assert!(matches!(
        r.claim(t.id, RUNNER, Utc::now() + Duration::hours(25))
            .await,
        Err(Error::Conflict(_))
    ));
    r.claim(t.id, RUNNER, Utc::now() + Duration::hours(23))
        .await
        .unwrap();
}
