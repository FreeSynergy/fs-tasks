//! The due date is judged as it is typed, by the rule of the save.

mod common;

use common::World;
use fs_components_workbenches::tasks::TaskSource;

#[test]
fn nonsense_is_named_and_a_good_time_or_nothing_is_fine() {
    let w = World::new();
    let said = w.adapter.check("due", "tomorrow").unwrap();
    assert!(said.contains("2026-11-30"), "{said}");
    assert!(w.adapter.check("due", "2026-11-30 17:00").is_none());
    assert!(w.adapter.check("due", "2026-11-30 17:00:30").is_none());
    assert!(
        w.adapter.check("due", "").is_none(),
        "empty means no due date"
    );
    assert!(
        w.adapter.check("title", "anything").is_none(),
        "other fields are not judged here"
    );
}
