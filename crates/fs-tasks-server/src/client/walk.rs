//! Walking all pages of `list` / `query` for [`super::items`].
//!
//! Pattern: Iterator — follow the cursor page by page until it comes back empty.

use fs_interfaces::tasks_capnp::items;
use fs_tasks_core::contract::write_filter;
use fs_tasks_core::{Error, Result, TaskEntry, TaskQuery};

use super::decode::{entries, wire};

/// Entries of one page and the cursor of the next (empty = the end).
macro_rules! page_of {
    ($answer:expr, $get:path) => {{
        let page = $answer.get().and_then($get).map_err(|e| wire(&e))?;
        let next = page
            .get_next()
            .and_then(fs_interfaces::common_capnp::cursor::Reader::get_opaque)
            .map_err(|e| wire(&e))?
            .to_vec();
        (entries(page.get_items().map_err(|e| wire(&e))?)?, next)
    }};
}

pub(super) async fn all(client: items::Client, query: Option<TaskQuery>) -> Result<Vec<TaskEntry>> {
    let (mut out, mut cursor) = (Vec::new(), Vec::<u8>::new());
    loop {
        let (found, next) = if let Some(query) = &query {
            let mut req = client.query_request();
            write_filter(query, req.get().init_filter());
            req.get().init_cursor().set_opaque(&cursor);
            let answer = req.send().promise.await.map_err(|e| wire(&e))?;
            page_of!(answer, items::query_results::Reader::get_page)
        } else {
            let mut req = client.list_request();
            req.get().init_cursor().set_opaque(&cursor);
            let answer = req.send().promise.await.map_err(|e| wire(&e))?;
            page_of!(answer, items::list_results::Reader::get_page)
        };
        out.extend(found);
        if next.is_empty() {
            return Ok(out);
        }
        if next == cursor {
            return Err(Error::Rejected("the server answered the same cursor twice".into()));
        }
        cursor = next;
    }
}
