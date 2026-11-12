//! What the wire says the provider can do.
//!
//! Pattern: Adapter (a filter on the provider's report) — the core provider also offers things the contract does not
//! carry yet (boards, comments, claims). A consumer of the socket reads the capabilities to know what it can call, so
//! those ids are reported as not possible here, with the reason, until `tasks.capnp` has the methods.

use fs_interfaces::Capabilities;
use fs_tasks_core::TaskItems;

/// Ids the core provider offers that the contract (`fs-interfaces` v0.0.19) has no method for.
const NOT_ON_THE_WIRE: [&str; 10] = [
    "board.open",
    "board.create",
    "column.create",
    "column.limit",
    "card.move",
    "comment.list",
    "comment.add",
    "comment.edit",
    "comment.delete",
    "task.claim",
];

/// The report of `items` as the socket can serve it.
pub(crate) fn wire_capabilities(items: &dyn TaskItems) -> Capabilities {
    wire_capabilities_of(&items.capabilities())
}

fn wire_capabilities_of(report: &Capabilities) -> Capabilities {
    let mut out = Capabilities::new(report.provider());
    for entry in report.entries() {
        if entry.supported && NOT_ON_THE_WIRE.contains(&entry.capability.as_str()) {
            out.unsupported(
                entry.capability.clone(),
                "offered by the program's core, not on the wire yet: the contract has no method for it, it is possible",
            );
        } else if entry.supported {
            out.supported(entry.capability.clone());
        } else {
            out.unsupported(entry.capability.clone(), entry.reason.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_the_wire_cannot_carry_is_reported_as_not_possible_and_the_rest_is_kept() {
        let mut core = Capabilities::new("somebody");
        core.supported("task.create")
            .supported("card.move")
            .supported("comment.add")
            .supported("dependency.evaluate")
            .unsupported("task.import", "no");
        let wire = wire_capabilities_of(&core);
        assert_eq!(wire.provider(), "somebody");
        assert!(wire.is_supported("task.create") && wire.is_supported("dependency.evaluate"));
        assert!(wire.why_not("card.move").is_some() && wire.why_not("comment.add").is_some());
        assert_eq!(
            wire.why_not("task.import").map(|u| u.reason().to_string()),
            Some("no".into())
        );
    }
}
