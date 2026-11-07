//! The cursor of a page: a [`Place`] in the order of the tasks, as bytes a client hands back.
//!
//! Pattern: none (two pure functions). A client only returns what it got (`Cursor.opaque`). The first
//! byte is the form: 1 was a position in the whole list (`fs-interfaces` `Page`), 2 is a place in
//! the sort key. Form 1 is not read any more: the position of a task moves when another is added or
//! removed, a place does not.

use fs_interfaces::rpc::invalid_request;
use fs_store::ObjectId;
use fs_tasks_core::Place;

const FORM: u8 = 2;

/// The cursor that continues after `place`.
pub(crate) fn encode(place: &Place) -> Vec<u8> {
    let id = place.id.to_string();
    let mut out = vec![FORM];
    for part in [place.order.as_str(), place.name.as_str(), id.as_str()] {
        let len = u32::try_from(part.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(part.as_bytes());
    }
    out
}

/// The place a cursor names; empty = the start.
///
/// # Errors
/// The cursor is not one `encode` made.
pub(crate) fn decode(cursor: &[u8]) -> capnp::Result<Option<Place>> {
    if cursor.is_empty() {
        return Ok(None);
    }
    let invalid = || invalid_request("invalid cursor");
    let (&form, mut rest) = cursor.split_first().ok_or_else(invalid)?;
    if form != FORM {
        return Err(invalid());
    }
    let mut parts: Vec<&str> = Vec::with_capacity(3);
    for _ in 0..3 {
        let (len, tail) = rest.split_first_chunk::<4>().ok_or_else(invalid)?;
        let len = usize::try_from(u32::from_le_bytes(*len)).map_err(|_| invalid())?;
        let (part, tail) = tail.split_at_checked(len).ok_or_else(invalid)?;
        parts.push(std::str::from_utf8(part).map_err(|_| invalid())?);
        rest = tail;
    }
    if !rest.is_empty() {
        return Err(invalid());
    }
    let id: ObjectId = parts[2].parse().map_err(|_| invalid())?;
    Ok(Some(Place {
        order: parts[0].into(),
        name: parts[1].into(),
        id,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_place_comes_back_as_it_went_and_nothing_else_does() {
        let place = Place {
            order: "2026-12-01T09:00:00Z".into(),
            name: "Äpfel \u{0}\u{1} mixed Case".into(),
            id: fs_store::Envelope::new("Task", "x").id,
        };
        let bytes = encode(&place);
        assert_eq!(decode(&bytes).unwrap(), Some(place));
        assert_eq!(decode(&[]).unwrap(), None);
        for bad in [
            &bytes[..bytes.len() - 1],
            &[1, 0, 0, 0, 0, 0, 0, 0, 0],
            b"xx",
        ] {
            assert!(decode(bad).is_err(), "{bad:?}");
        }
        let mut longer = bytes;
        longer.push(0);
        assert!(decode(&longer).is_err());
    }
}
