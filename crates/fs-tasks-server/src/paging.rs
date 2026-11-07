//! A page of tasks on the wire.
//!
//! Pattern: Adapter — a [`TaskPage`] from the provider becomes a `TaskPage` of `tasks.capnp`,
//! cut by weight so the answer always fits a message, with the cursor of the place the page ends
//! at. The page was cut in the provider's store; here only its weight is looked at.

use fs_interfaces::rpc::{MAX_ITEM_BYTES, MAX_PAGE_BYTES, Problem, TOO_LARGE, envelope_weight};
use fs_interfaces::tasks_capnp;
use fs_tasks_core::{Placed, TaskItems, TaskPage, TaskQuery};

use crate::cursor::{decode, encode};
use crate::errors::rpc_error;
use crate::wire::write_task;

fn weight(p: &Placed) -> usize {
    p.entry.task.to_value().to_string().len() + envelope_weight(&p.entry.envelope)
}

/// How many of the items go into this answer: all, or fewer if they weigh more than a message
/// carries. An item that cannot be sent even alone is a `too-large` problem whose `next` is the
/// cursor past it (none if it was the last), so what is behind it stays readable.
fn fitting(page: &TaskPage) -> capnp::Result<usize> {
    let (mut taken, mut bytes) = (0usize, 0usize);
    for (i, item) in page.items.iter().enumerate() {
        let heavy = weight(item);
        bytes = bytes.saturating_add(heavy);
        let full = bytes > MAX_PAGE_BYTES;
        if full && taken > 0 {
            break;
        }
        if heavy > MAX_ITEM_BYTES {
            let later = i + 1 < page.items.len() || page.more;
            let detail = format!("one item of {heavy} bytes, at most {MAX_ITEM_BYTES} fit");
            // With a cursor no extensions: the numbers are in the detail anyway.
            let problem = if later {
                Problem::new(TOO_LARGE, "Too large", detail, 413)
            } else {
                Problem::too_large(detail, heavy as u64, MAX_ITEM_BYTES as u64)
            };
            return Err(problem
                .with_next(later.then(|| encode(&item.place)))
                .into_error());
        }
        taken += 1;
        if full {
            break;
        }
    }
    Ok(taken)
}

/// The page of `query` that `cursor` asks for.
pub(crate) async fn read_page(
    items: &dyn TaskItems,
    query: &TaskQuery,
    cursor: &[u8],
    size: usize,
) -> capnp::Result<TaskPage> {
    let after = decode(cursor)?;
    items
        .page(query, after.as_ref(), size)
        .await
        .map_err(rpc_error)
}

/// Write the page.
pub(crate) fn write_page(
    mut out: tasks_capnp::task_page::Builder<'_>,
    page: &TaskPage,
) -> capnp::Result<()> {
    let taken = fitting(page)?;
    let count = u32::try_from(taken).map_err(|_| capnp::Error::failed("list too long".into()))?;
    let mut items = out.reborrow().init_items(count);
    for (i, item) in (0u32..).zip(&page.items[..taken]) {
        write_task(items.reborrow().get(i), &item.entry);
    }
    let next = match page.items[..taken].last() {
        Some(last) if taken < page.items.len() || page.more => encode(&last.place),
        _ => Vec::new(),
    };
    out.init_next().set_opaque(&next);
    Ok(())
}
