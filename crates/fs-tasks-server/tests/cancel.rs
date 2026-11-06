//! A client that goes away cannot leave a write half done.

mod common;

use std::time::Duration;

use common::{ICS, items_of, with_server};

#[tokio::test(flavor = "current_thread")]
async fn a_dropped_import_still_finishes() {
    with_server(|root| async move {
        let items = items_of(&root).await;
        let mut import = items.import_ics_request();
        import.get().set_text(ICS);
        drop(import.send().promise);
        tokio::time::sleep(Duration::from_millis(300)).await;
        let list = items.list_request().send().promise.await.unwrap();
        let page = list.get().unwrap().get_page().unwrap();
        assert_eq!(page.get_items().unwrap().len(), 1);
    })
    .await;
}
