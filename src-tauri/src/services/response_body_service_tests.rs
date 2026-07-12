use std::{fs, path::PathBuf};

use super::response_body_service::{ResponseBodyStore, ResponsePresentation};
use tokio::io::AsyncWriteExt;

fn test_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("postnot-{name}-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&path).expect("create response body test directory");
    path
}

#[tokio::test]
async fn stores_large_text_as_a_file_and_reads_bounded_rows() {
    let root = test_dir("body-window");
    let store = ResponseBodyStore::new(root.clone());
    let body = b"first\nsecond\nthird\n";

    let stored = store
        .store_bytes(body, Some("application/json; charset=utf-8"))
        .await
        .expect("store body");

    assert_eq!(stored.presentation, ResponsePresentation::Json);
    assert_eq!(stored.size_bytes, body.len() as u64);

    let window = store
        .read_window(&stored.handle_id, 1, 1, 1024)
        .await
        .expect("read body window");
    assert_eq!(window.total_rows, 3);
    assert_eq!(window.rows.len(), 1);
    assert_eq!(window.rows[0].text, "second");

    store.release(&stored.handle_id).expect("release body");
    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn searches_across_internal_read_boundaries_with_case_control() {
    let root = test_dir("body-search");
    let store = ResponseBodyStore::new(root.clone());
    let mut body = vec![b'x'; 65_535];
    body.extend_from_slice(b"Needle then needle");
    let stored = store
        .store_bytes(&body, Some("text/plain"))
        .await
        .expect("store body");

    let insensitive = store
        .search(&stored.handle_id, "needle", false)
        .await
        .expect("case-insensitive search");
    assert_eq!(insensitive.total_matches, 2);

    let sensitive = store
        .search(&stored.handle_id, "Needle", true)
        .await
        .expect("case-sensitive search");
    assert_eq!(sensitive.total_matches, 1);
    assert_eq!(sensitive.matches[0].byte_offset, 65_535);
    assert_eq!(sensitive.matches[0].row_index, 0);

    store.release(&stored.handle_id).expect("release body");
    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn searches_unicode_case_insensitively_across_a_read_boundary() {
    let root = test_dir("body-unicode-search");
    let store = ResponseBodyStore::new(root.clone());
    let mut body = vec![b'x'; 65_533];
    body.extend_from_slice("ПРИВЕТ".as_bytes());
    let stored = store
        .store_bytes(&body, Some("text/plain; charset=utf-8"))
        .await
        .expect("store body");
    let result = store
        .search(&stored.handle_id, "привет", false)
        .await
        .expect("search unicode");
    assert_eq!(result.total_matches, 1);
    assert_eq!(result.matches[0].byte_offset, 65_533);
    store.release(&stored.handle_id).expect("release body");
    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn navigates_matches_beyond_the_retained_search_offset_cap() {
    let root = test_dir("body-search-cap");
    let store = ResponseBodyStore::new(root.clone());
    let body = vec![b'a'; 100_005];
    let stored = store
        .store_bytes(&body, Some("text/plain"))
        .await
        .expect("store body");
    let search = store
        .search(&stored.handle_id, "a", true)
        .await
        .expect("search body");
    assert!(search.capped);
    assert_eq!(search.matches.len(), 100_000);
    let next = store
        .find_directional_match(
            &stored.handle_id,
            "a",
            true,
            search.matches.last().unwrap().byte_offset,
            true,
            true,
        )
        .await
        .expect("find next")
        .expect("next match");
    assert_eq!(next.byte_offset, 100_000);
    let wrapped_previous = store
        .find_directional_match(&stored.handle_id, "a", true, 0, false, true)
        .await
        .expect("find previous")
        .expect("wrapped match");
    assert_eq!(wrapped_previous.byte_offset, 100_004);
    store.release(&stored.handle_id).expect("release body");
    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn formats_json_without_changing_string_contents() {
    let root = test_dir("body-format");
    let store = ResponseBodyStore::new(root.clone());
    let stored = store
        .store_bytes(
            br#"{"message":"a,b:{c}","items":[1,2]}"#,
            Some("application/json"),
        )
        .await
        .expect("store body");

    let formatted = store
        .format_json(&stored.handle_id)
        .await
        .expect("format response");
    let text = store
        .read_all_text(&formatted.handle_id)
        .await
        .expect("read formatted response");

    assert!(text.contains("\n  \"message\": \"a,b:{c}\","));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&text).unwrap()["items"][1],
        2
    );

    store.release(&stored.handle_id).expect("release source");
    store
        .release(&formatted.handle_id)
        .expect("release formatted");
    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn renders_binary_windows_as_offset_hex_and_ascii_rows() {
    let root = test_dir("body-hex");
    let store = ResponseBodyStore::new(root.clone());
    let stored = store
        .store_bytes(
            &[0, 1, 2, b'A', b'B', 255],
            Some("application/octet-stream"),
        )
        .await
        .expect("store body");
    let window = store
        .read_hex_window(&stored.handle_id, 0, 1)
        .await
        .expect("read hex window");
    assert_eq!(window.total_rows, 1);
    assert!(window.rows[0]
        .text
        .starts_with("00000000  00 01 02 41 42 ff"));
    assert!(window.rows[0].text.ends_with("...AB."));
    store.release(&stored.handle_id).expect("release body");
    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn decodes_declared_legacy_charset_for_visible_rows() {
    let root = test_dir("body-charset");
    let store = ResponseBodyStore::new(root.clone());
    let stored = store
        .store_bytes(b"price: \x80", Some("text/plain; charset=windows-1252"))
        .await
        .expect("store body");
    let window = store
        .read_window(&stored.handle_id, 0, 1, 1024)
        .await
        .expect("read row");
    assert_eq!(window.rows[0].text, "price: €");
    store.release(&stored.handle_id).expect("release body");
    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn searches_the_decoded_legacy_charset_representation() {
    let root = test_dir("body-charset-search");
    let store = ResponseBodyStore::new(root.clone());
    let stored = store
        .store_bytes(b"price: \x80", Some("text/plain; charset=windows-1252"))
        .await
        .expect("store body");
    let result = store
        .search(&stored.handle_id, "€", true)
        .await
        .expect("search decoded text");
    assert_eq!(result.total_matches, 1);
    assert_eq!(result.matches[0].row_index, 0);
    store.release(&stored.handle_id).expect("release body");
    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn long_utf8_lines_are_segmented_only_at_character_boundaries() {
    let root = test_dir("body-utf8-segments");
    let store = ResponseBodyStore::new(root.clone());
    let text = format!("{}€tail", "x".repeat(64 * 1024 - 1));
    let stored = store
        .store_bytes(text.as_bytes(), Some("text/plain; charset=utf-8"))
        .await
        .expect("store body");
    let window = store
        .read_window(&stored.handle_id, 0, 2, 128 * 1024)
        .await
        .expect("read rows");
    assert_eq!(window.total_rows, 2);
    assert_eq!(
        window
            .rows
            .iter()
            .map(|row| row.text.as_str())
            .collect::<String>(),
        text
    );
    assert!(!window.rows.iter().any(|row| row.text.contains('�')));
    store.release(&stored.handle_id).expect("release body");
    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn defers_history_file_deletion_until_all_leases_are_released() {
    let root = test_dir("body-leases");
    let store = ResponseBodyStore::new(root.clone());
    let stored = store
        .store_bytes(b"leased", Some("text/plain"))
        .await
        .expect("store body");
    let path = store.path_for(&stored.handle_id).expect("body path");
    store
        .mark_history_owned(&stored.handle_id, path.clone())
        .expect("mark history owned");
    store.retain(&stored.handle_id).expect("second lease");
    assert!(store
        .delete_path_when_released(&path)
        .expect("defer delete"));
    store
        .release(&stored.handle_id)
        .expect("release first lease");
    assert!(path.exists());
    store
        .release(&stored.handle_id)
        .expect("release final lease");
    assert!(!path.exists());
    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn defers_shared_history_file_deletion_until_every_handle_is_released() {
    let root = test_dir("body-shared-handles");
    let path = root.join("shared.body");
    fs::write(&path, b"shared").expect("write shared body");
    let store = ResponseBodyStore::new(root.clone());
    let first = store
        .register_existing(path.clone(), Some("text/plain".into()), b"shared", 6)
        .expect("first handle");
    let second = store
        .register_existing(path.clone(), Some("text/plain".into()), b"shared", 6)
        .expect("second handle");
    assert!(store
        .delete_path_when_released(&path)
        .expect("defer shared deletion"));
    store
        .release(&first.handle_id)
        .expect("release first handle");
    assert!(path.exists(), "the second handle still leases the file");
    store
        .release(&second.handle_id)
        .expect("release second handle");
    assert!(!path.exists());
    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn startup_reconciliation_preserves_history_and_removes_orphans() {
    let root = test_dir("body-reconcile");
    let history = root.join("history.body");
    let orphan = root.join("orphan.body");
    fs::write(&history, b"history").expect("history file");
    fs::write(&orphan, b"orphan").expect("orphan file");
    let store = ResponseBodyStore::new(root.clone());
    store
        .reconcile(std::slice::from_ref(&history))
        .await
        .expect("reconcile files");
    assert!(history.exists());
    assert!(!orphan.exists());
    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn handles_64_mib_file_through_bounded_window_and_search_apis() {
    let root = test_dir("body-64mib");
    let path = root.join("large.body");
    write_large_fixture(&path, 64).await;
    let store = ResponseBodyStore::new(root.clone());
    let stored = store
        .register_existing(
            path.clone(),
            Some("text/plain".into()),
            b"POSTNOT-EARLY-MATCH\n",
            64 * 1024 * 1024,
        )
        .expect("register body");
    let window = store
        .read_window(&stored.handle_id, 0, 4, 4096)
        .await
        .expect("read window");
    assert!(window.rows.len() <= 4);
    assert!(window.rows[0].text.contains("POSTNOT-EARLY-MATCH"));
    let search = store
        .search(&stored.handle_id, "POSTNOT-EARLY-MATCH", true)
        .await
        .expect("search body");
    assert_eq!(search.total_matches, 1);
    let index_size = fs::metadata(path.with_extension("idx"))
        .expect("persisted sparse index")
        .len();
    assert!(
        index_size < 1024 * 1024,
        "sparse index should stay bounded, got {index_size} bytes"
    );
    store.release(&stored.handle_id).expect("release body");
    fs::remove_dir_all(root).ok();
}

#[tokio::test]
#[ignore = "1 GiB performance acceptance scenario"]
async fn handles_one_gib_file_without_full_body_reads() {
    let root = test_dir("body-1gib");
    let path = root.join("one-gib.body");
    write_large_fixture(&path, 1024).await;
    let store = ResponseBodyStore::new(root.clone());
    let stored = store
        .register_existing(
            path,
            Some("text/plain".into()),
            b"POSTNOT-EARLY-MATCH\n",
            1024 * 1024 * 1024,
        )
        .expect("register body");
    let window = store
        .read_window(&stored.handle_id, 0, 8, 8192)
        .await
        .expect("read window");
    assert!(window.rows.len() <= 8);
    let search = store
        .search(&stored.handle_id, "POSTNOT-EARLY-MATCH", true)
        .await
        .expect("search body");
    assert_eq!(search.total_matches, 1);
    store.release(&stored.handle_id).expect("release body");
    fs::remove_dir_all(root).ok();
}

async fn write_large_fixture(path: &std::path::Path, mebibytes: usize) {
    let mut file = tokio::fs::File::create(path).await.expect("create fixture");
    let mut block = vec![b'x'; 1024 * 1024];
    block[..20].copy_from_slice(b"POSTNOT-EARLY-MATCH\n");
    file.write_all(&block).await.expect("write first block");
    block[..20].fill(b'x');
    for _ in 1..mebibytes {
        file.write_all(&block).await.expect("write fixture block");
    }
    file.flush().await.expect("flush fixture");
}
