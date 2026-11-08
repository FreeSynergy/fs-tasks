//! The cursor of a page: a [`Place`] in the order of the tasks, as bytes a client hands back.
//!
//! Pattern: Adapter — the byte form lives in `fs_interfaces::rpc` (`encode_place`, `decode_place`)
//! since `fs-interfaces` v0.49.1; this file only maps the tasks' place to it and back.

use fs_interfaces::rpc::{PlaceParts, decode_place, encode_place};
use fs_tasks_core::Place;

/// The cursor that continues after `place`.
pub(crate) fn encode(place: &Place) -> Vec<u8> {
    encode_place(&PlaceParts {
        key: place.order.clone(),
        name: place.name.clone(),
        id: place.id,
    })
}

/// The place a cursor names; empty = the start.
///
/// # Errors
/// The cursor is not one `encode` made.
pub(crate) fn decode(cursor: &[u8]) -> capnp::Result<Option<Place>> {
    Ok(decode_place(cursor)?.map(|p| Place {
        order: p.key,
        name: p.name,
        id: p.id,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_place_comes_back_as_it_went() {
        let place = Place {
            order: "k \u{0}".into(),
            name: "Äpfel".into(),
            id: fs_objects::Envelope::new("X", "x").id,
        };
        assert_eq!(decode(&encode(&place)).unwrap(), Some(place));
        assert!(decode(b"xx").is_err());
    }
}
