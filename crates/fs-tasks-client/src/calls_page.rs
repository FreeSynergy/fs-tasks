//! One page of `list` or `query`, and the loop that reads them all.
//!
//! Pattern: Adapter — one request with a `cursor` in, one `TaskPage` read back as values of the
//! core. `pages` is the walk to the end that `list` and `query` of `TaskItems` use; `TaskPages` takes
//! one `page` at a time.

use fs_interfaces::tasks_capnp::items::Client;
use fs_interfaces::tasks_capnp::task_page;
use fs_tasks_core::contract::write_filter;
use fs_tasks_core::{Result, TaskQuery};

use crate::problems::{Subject, error_of};
use crate::read::{revised, unreadable};
use crate::revised::Revised;

/// A page and the cursor of the next one (empty at the end).
pub(crate) type Step = (Vec<Revised>, Vec<u8>);

/// The page at `cursor` (empty for the first) of `filter`; no filter is `list`.
pub(crate) async fn page(c: &Client, filter: Option<&TaskQuery>, cursor: &[u8]) -> Result<Step> {
    let mut found: Vec<Revised> = Vec::new();
    let next = match filter {
        None => {
            let mut req = c.list_request();
            req.get().init_cursor().set_opaque(cursor);
            let answer = req.send().promise.await;
            let answer = answer.map_err(|e| error_of(&e, Subject::None))?;
            take(answer.get().and_then(|r| r.get_page()), &mut found)?
        }
        Some(filter) => {
            let mut req = c.query_request();
            write_filter(filter, req.get().init_filter());
            req.get().init_cursor().set_opaque(cursor);
            let answer = req.send().promise.await;
            let answer = answer.map_err(|e| error_of(&e, Subject::None))?;
            take(answer.get().and_then(|r| r.get_page()), &mut found)?
        }
    };
    Ok((found, next))
}

/// Every page until the cursor is empty. A page that is empty but names a next one cannot be
/// followed to an end, so it is an error instead of a silently short list.
pub(crate) async fn pages(c: &Client, filter: Option<&TaskQuery>) -> Result<Vec<Revised>> {
    let mut found: Vec<Revised> = Vec::new();
    let mut cursor: Vec<u8> = Vec::new();
    loop {
        let (items, next) = page(c, filter, &cursor).await?;
        let empty = items.is_empty();
        found.extend(items);
        if next.is_empty() {
            return Ok(found);
        }
        if empty {
            return Err(unreadable("an empty page that names a next one"));
        }
        cursor = next;
    }
}

/// The entries of one page onto `found`; answers the cursor of the next page.
fn take(page: capnp::Result<task_page::Reader<'_>>, found: &mut Vec<Revised>) -> Result<Vec<u8>> {
    let page = page.map_err(unreadable)?;
    for item in page.get_items().map_err(unreadable)? {
        found.push(revised(item)?);
    }
    Ok(page
        .get_next()
        .and_then(|n| n.get_opaque())
        .map_err(unreadable)?
        .to_vec())
}
