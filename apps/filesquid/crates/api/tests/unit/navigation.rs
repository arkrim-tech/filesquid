//! Unit tests of the navigation handles, cursors and the in-memory folder index (Task
//! API-F1.11; API-S-021, API-S-005): the opaque node handle codec, its bounds, and the
//! ordering rules of the listing.

use filesquid_api::navigation::{
    FolderIndex, FolderScope, MemoryFolderIndex, NavigationChange, NodeRef, MAX_HANDLE_BYTES,
};
use filesquid_core::{CollectionId, LibraryId, RepositoryId, VirtualCollectionId};

fn library(id: &str) -> LibraryId {
    LibraryId::new(id).expect("library id")
}

fn collection(id: &str) -> CollectionId {
    CollectionId::new(id).expect("collection id")
}

fn folder(path: &str) -> NodeRef {
    NodeRef::folder(FolderScope::Collection(collection("c-1")), path)
}

#[test]
fn every_kind_of_node_round_trips_through_its_handle() {
    let nodes = [
        NodeRef::Repository(RepositoryId::new("repo").expect("id")),
        NodeRef::Library(library("lib")),
        NodeRef::Collection(collection("col")),
        NodeRef::VirtualCollection(VirtualCollectionId::new("vc").expect("id")),
        folder("2025/summer"),
        NodeRef::folder(FolderScope::Unfiled(library("lib")), "inbox"),
    ];
    for node in nodes {
        let handle = node.to_handle().expect("handle");
        assert!(handle.len() <= MAX_HANDLE_BYTES, "{handle}");
        assert!(handle.is_ascii() && !handle.contains('/'), "{handle}");
        assert_eq!(NodeRef::from_handle(&handle), Some(node));
    }
}

#[test]
fn identifiers_with_separators_and_names_with_unicode_survive() {
    let nodes = [
        NodeRef::Collection(collection("a:b/c d")),
        NodeRef::folder(FolderScope::Collection(collection("1:2:3")), "été/日本/x:y"),
    ];
    for node in nodes {
        let handle = node.to_handle().expect("handle");
        assert_eq!(NodeRef::from_handle(&handle), Some(node));
    }
}

#[test]
fn a_handle_is_the_same_for_the_same_node_and_differs_between_nodes() {
    assert_eq!(folder("a/b").to_handle(), folder("a/b").to_handle());
    assert_ne!(folder("a/b").to_handle(), folder("a/c").to_handle());
}

#[test]
fn other_spellings_and_tampered_handles_are_refused() {
    let handle = folder("a/b").to_handle().expect("handle");
    let tampered = format!("{handle}A");
    let padded = format!("{handle}=");
    let flipped: String = handle
        .chars()
        .enumerate()
        .map(|(i, c)| {
            if i == 5 {
                if c == 'A' {
                    'B'
                } else {
                    'A'
                }
            } else {
                c
            }
        })
        .collect();
    for bad in [
        "", "n1.", "n2.abc", "abc", &tampered, &padded, &flipped, "n1.%%%",
    ] {
        assert_eq!(NodeRef::from_handle(bad), None, "{bad}");
    }
}

#[test]
fn a_folder_path_that_is_not_a_logical_path_has_no_handle() {
    for path in [
        "", "/a", "a/", "a//b", "a/./b", "a/../b", "..", ".", "a\0b", "a\nb",
    ] {
        assert_eq!(folder(path).to_handle(), None, "{path:?}");
    }
}

#[test]
fn a_folder_whose_handle_would_pass_512_bytes_has_none() {
    let deep = vec!["d".repeat(100); 3].join("/");
    assert!(folder(&deep).to_handle().is_some());
    let too_deep = vec!["d".repeat(100); 5].join("/");
    assert_eq!(folder(&too_deep).to_handle(), None);
}

#[test]
fn a_decoded_folder_handle_with_a_traversal_segment_is_refused() {
    // Hand-built: the codec itself must refuse what its encoder never writes.
    let raw = "f".to_string();
    assert_eq!(
        NodeRef::from_handle(&format!("n1.{}", encode_raw(&raw))),
        None
    );
    let escaped = "fc1:c../x".to_string();
    assert_eq!(
        NodeRef::from_handle(&format!("n1.{}", encode_raw(&escaped))),
        None
    );
}

/// Base64url without padding, as the codec writes it (test-side copy to build bad input).
fn encode_raw(text: &str) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let bytes = text.as_bytes();
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | (u32::from(*b) << (16 - 8 * i)));
        let symbols = chunk.len() + 1;
        for i in 0..symbols {
            out.push(char::from(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize]));
        }
    }
    out
}

#[test]
fn the_memory_index_lists_subfolders_in_code_point_order_after_a_name() {
    let index = MemoryFolderIndex::new();
    let scope = FolderScope::Collection(collection("c-1"));
    for path in ["b/x.jpg", "a/y/z.jpg", "É/q.jpg", "a/x.jpg", "root.jpg"] {
        index.add_file(&scope, path);
    }
    let all = index.subfolders(&scope, "", None, 10).expect("names");
    assert_eq!(all, vec!["a".to_string(), "b".to_string(), "É".to_string()]);
    let after = index.subfolders(&scope, "", Some("a"), 1).expect("names");
    assert_eq!(after, vec!["b".to_string()]);
    assert_eq!(
        index.subfolders(&scope, "a", None, 10).expect("names"),
        vec!["y".to_string()]
    );
    assert!(index.folder_exists(&scope, "a/y").expect("exists"));
    assert!(!index.folder_exists(&scope, "root.jpg").expect("exists"));
}

#[test]
fn the_memory_index_leaves_out_hidden_and_deleted_files() {
    let index = MemoryFolderIndex::new();
    let scope = FolderScope::Collection(collection("c-1"));
    index.add_file(&scope, "a/x.jpg");
    index.add_file(&scope, "b/y.jpg");
    index.set_ordinary(&scope, "b/y.jpg", false);
    assert_eq!(
        index.subfolders(&scope, "", None, 10).expect("names"),
        vec!["a".to_string()]
    );
    assert!(!index.folder_exists(&scope, "b").expect("exists"));
    index.set_ordinary(&scope, "b/y.jpg", true);
    assert!(index.folder_exists(&scope, "b").expect("exists"));
}

#[test]
fn a_change_names_one_to_64_parents_or_all() {
    let nodes: Vec<String> = (0..64).map(|n| format!("h{n}")).collect();
    let some = NavigationChange::parents(nodes.clone());
    assert_eq!(
        serde_json::to_value(&some).expect("json")["parents"]
            .as_array()
            .map(Vec::len),
        Some(64)
    );
    let mut more = nodes;
    more.push("h64".to_string());
    assert_eq!(
        serde_json::to_value(NavigationChange::parents(more)).expect("json"),
        serde_json::json!({ "all": true })
    );
    assert_eq!(
        serde_json::to_value(NavigationChange::parents(Vec::new())).expect("json"),
        serde_json::json!({ "all": true }),
        "an empty change is never sent as no parents"
    );
}
