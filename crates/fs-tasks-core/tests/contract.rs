//! The committed Cap'n Proto code matches the Rust interface types (feature `contract`).
#![cfg(feature = "contract")]

use fs_tasks_core::contract::{
    read_filter, relation_from_wire, relation_to_wire, write_capabilities, write_filter,
};
use fs_tasks_core::tasks_capnp::task_filter;
use fs_tasks_core::{CAPABILITIES, Relation, TaskQuery, common_capnp, utc};

#[test]
fn filter_roundtrips_through_the_wire() {
    let q = TaskQuery::new()
        .due_between(
            utc("2026-11-01T00:00:00Z").unwrap(),
            utc("2026-12-01T00:00:00Z").unwrap(),
        )
        .with_progress("in-process")
        .with_label("urgent");
    for q in [q, TaskQuery::new()] {
        let mut msg = capnp::message::Builder::new_default();
        write_filter(&q, msg.init_root::<task_filter::Builder>());
        let bytes = capnp::serialize::write_message_to_words(&msg);
        let back = capnp::serialize::read_message(
            &mut bytes.as_slice(),
            capnp::message::ReaderOptions::new(),
        )
        .unwrap();
        let got = read_filter(back.get_root::<task_filter::Reader>().unwrap()).unwrap();
        assert_eq!(got, q);
    }
}

#[test]
fn relations_and_capabilities_map_both_ways() {
    for r in Relation::ALL {
        assert_eq!(relation_from_wire(relation_to_wire(r)), r);
    }
    let mut msg = capnp::message::Builder::new_default();
    write_capabilities(msg.init_root::<common_capnp::capability_list::Builder>());
    let list = msg
        .get_root_as_reader::<common_capnp::capability_list::Reader>()
        .unwrap();
    let ids: Vec<_> = list
        .get_ids()
        .unwrap()
        .iter()
        .map(|s| s.unwrap().to_string().unwrap())
        .collect();
    assert_eq!(ids, CAPABILITIES);
}
