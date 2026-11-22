//! The drive routes of Task API-F2.2 (API-S-011, SRS-F-062): copy, the view operation and
//! storage use per collection. Reads take `files.read` and are not journaled when they
//! succeed; a copy takes `files.write` and is journaled with opaque IDs only. Every refusal
//! is journaled once.
//!
//! The view and the collection count read the host's catalog through the navigation service
//! and, for the view, the caller's own journal entries; both may block, so they run on a
//! blocking thread before the file service reads each file's object.
//!
//! Every file row names its library, head version and home collection, and the folder rows
//! of the answered page are counted after paging ([`super::folder_counts`]); `view=recent`
//! is answered from the caller's own journal entries ([`super::recent`]) (Task API-F2.21).

use super::command::{self, FileCommand};
use super::copy::CopyRequest;
use super::drive::{
    check_filter, check_limit, check_view, file_candidate, fingerprint, folder_candidate,
    matches_filter, page, page_rows, recent_candidate, Candidate, FilePlace, MyTimes, PageParams,
    ViewKind, ViewPage, ViewRequest, ViewRow,
};
use super::drive_facts::{CollectionHistorySizeView, CollectionSizeQuery};
use super::drive_scope::{collection_files, my_times, view_scope, ViewScope};
use super::error::FileError;
use super::folder_counts::{apply_count, folder_files, folder_nodes, FolderCount, FolderFiles};
use super::homing;
use super::provenance::Caller;
use super::recent::{recent_scope, RecentScope};
use super::routes::{body, files, query, read_answer, recorded, resumed_entry};
use super::search::{parse_zone, search_files, search_page, Examined, SearchPage, SearchRequest};
use super::search_match::Searcher;
use super::search_query::parse;
use super::search_scope::{search_scope, ScopedFile, SearchScope};
use super::FileOperations;
use super::{COLLECTION_HISTORY_SIZE_OPERATION, COPY_OPERATION, SEARCH_OPERATION, VIEW_OPERATION};
use crate::auth::{Action, Principal};
use crate::home::Destination;
use crate::navigation::{NavigationService, NodeRef};
use crate::preferences::{FolderPlacement, SortDirection};
use crate::routes::Route;
use crate::ApiState;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Query, State};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Extension, Json};
use filesquid_core::CollectionId;
use std::collections::HashMap;
use std::sync::Arc;

/// The drive routes, relative to [`crate::API_PREFIX`].
pub(crate) fn routes() -> Vec<Route> {
    vec![
        ("/files/copies", post(copy), Action::WriteFiles),
        ("/files/view", get(view), Action::ReadFiles),
        ("/files/search", get(search), Action::ReadFiles),
        (
            "/files/collection-history-size",
            get(collection_history_size),
            Action::ReadFiles,
        ),
    ]
}

/// What the blocking part of a view read: the scope and the caller's own times.
struct ViewInputs {
    scope: ViewScope,
    mine: HashMap<String, MyTimes>,
}

/// `POST /api/v1/files/copies`: copy a file at its head into a new file, as a command
/// ([`super::command`]) whose expected revision is the source file object's head.
async fn copy(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Json<FileCommand<CopyRequest>>, JsonRejection>,
) -> Response {
    let command = body(request).and_then(|c| command::check(c.envelope, c.input));
    let (state, principal) = (&state, &principal);
    command::run(
        state,
        principal,
        COPY_OPERATION,
        command,
        |files, c, actor| async move {
            let operation_id = c.operation_id.as_str().to_string();
            let fingerprint = homing::fingerprint_of(&c.input)?;
            let mut input = c.input;
            let destination = Destination {
                collection_id: input.collection_id.clone(),
                folder: input.folder.clone(),
            };
            let name = files
                .copy_name(input.library_id.clone(), input.file_id.clone())
                .await?;
            let place = (input.library_id.as_str(), &destination);
            let ids = (operation_id.as_str(), Some(fingerprint.as_str()));
            let prepared = homing::prepare_home(state, principal, place, &name, ids).await?;
            input.reserved_file_id = prepared.reserved.as_ref().map(|id| id.as_str().to_string());
            let caller = Caller::new(actor).with_operation(c.operation_id);
            let attempt = files.copy(input, caller, c.expected).await;
            if attempt.is_err() {
                homing::release_reservation(&prepared, &operation_id).await;
            }
            let mut copied = attempt?;
            let (collection, path) =
                homing::settle_home(state, prepared, &operation_id, &copied.view.file_id).await?;
            copied.view.collection_id = Some(collection);
            copied.view.path = Some(path);
            Ok(recorded(resumed_entry(COPY_OPERATION, principal, copied)))
        },
    )
    .await
}

/// `GET /api/v1/files/view`: one page of a library, collection or virtual-collection node.
async fn view(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Query<ViewRequest>, QueryRejection>,
) -> Response {
    let outcome = view_page(&state, &principal, request).await;
    read_answer(&state, VIEW_OPERATION, &principal, outcome).await
}

/// The page a view request names.
async fn view_page(
    state: &ApiState,
    principal: &Principal,
    request: Result<Query<ViewRequest>, QueryRejection>,
) -> Result<ViewPage, FileError> {
    let request = query(request)?;
    check_view(request.view)?;
    check_limit(request.limit)?;
    let filter = check_filter(request.filter.as_deref())?;
    let files = files(state)?;
    if request.view == ViewKind::Recent {
        return recent_page(state, principal, &request, filter.as_deref()).await;
    }
    let inputs = read_view_inputs(state, principal, &request).await?;
    let mut complete = inputs.scope.complete;
    let mut candidates: Vec<Candidate> = inputs
        .scope
        .folders
        .into_iter()
        .map(|folder| folder_candidate(folder, request.sort))
        .collect();
    let across = inputs.scope.across_libraries;
    for mut library in inputs.scope.libraries {
        let facts = match files
            .file_facts(library.library_id.clone(), library.files)
            .await
        {
            Ok(facts) => facts,
            Err(err) if across && err.code() == crate::ErrorCode::NotFound => {
                complete = false;
                continue;
            }
            Err(err) => return Err(err),
        };
        complete &= facts.complete;
        candidates.extend(facts.facts.into_iter().map(|file| {
            let mine = inputs
                .mine
                .get(&file.object_id)
                .copied()
                .unwrap_or_default();
            let deleted = library.deleted.remove(&file.file_id);
            let place = FilePlace {
                library_id: library.library_id.clone(),
                home: library.homes.remove(&file.file_id),
            };
            file_candidate(file, place, mine, request.sort, deleted)
        }));
    }
    candidates.retain(|candidate| matches_filter(candidate, filter.as_deref()));
    let mut page = page(&request, candidates, complete)?;
    count_folders(state, principal, files.as_ref(), &mut page.rows).await?;
    Ok(page)
}

/// One page of the Recent view of `request`: the files the caller changed, opened or
/// uploaded, newest first, whatever sort key, direction or folder placement was asked.
async fn recent_page(
    state: &ApiState,
    principal: &Principal,
    request: &ViewRequest,
    filter: Option<&str>,
) -> Result<ViewPage, FileError> {
    let files = files(state)?;
    let scope = read_recent_scope(state, principal, request).await?;
    let mine = read_my_times(state, principal, &scope).await?;
    let mut complete = scope.complete;
    let mut candidates: Vec<Candidate> = Vec::new();
    for mut library in scope.libraries {
        let read = files
            .file_facts(library.library_id.clone(), library.files)
            .await;
        let facts = match read {
            Ok(facts) => facts,
            Err(err) if err.code() == crate::ErrorCode::NotFound => {
                complete = false;
                continue;
            }
            Err(err) => return Err(err),
        };
        complete &= facts.complete;
        for file in facts.facts {
            let Some(hit) = library.hits.get(&file.file_id).copied() else {
                continue;
            };
            let place = FilePlace {
                library_id: library.library_id.clone(),
                home: library.homes.remove(&file.file_id),
            };
            let times = mine.get(&file.object_id).copied().unwrap_or_default();
            let row = file_candidate(file, place, times, request.sort, None);
            candidates.push(recent_candidate(row, hit.kind, hit.time_ms));
        }
    }
    candidates.retain(|candidate| matches_filter(candidate, filter));
    let params = PageParams {
        fingerprint: fingerprint(request),
        direction: SortDirection::Descending,
        folders: FolderPlacement::Mixed,
        cursor: request.cursor.as_deref(),
        limit: check_limit(request.limit)?,
    };
    let page = page_rows(params, candidates)?;
    Ok(ViewPage {
        node: request.node.clone(),
        view: request.view,
        sort: request.sort,
        direction: SortDirection::Descending,
        folders: FolderPlacement::Mixed,
        rows: page.rows,
        next_cursor: page.next_cursor,
        complete,
    })
}

/// What the Recent view of `request` covers, read from the journal and the catalog on a
/// blocking thread.
async fn read_recent_scope(
    state: &ApiState,
    principal: &Principal,
    request: &ViewRequest,
) -> Result<RecentScope, FileError> {
    let navigation = navigation(state)?;
    let store = state
        .monitor()
        .map(|monitor| monitor.store())
        .ok_or_else(|| FileError::unavailable("no activity journal is installed"))?;
    let node = NodeRef::from_handle(&request.node).ok_or_else(FileError::not_found)?;
    let principal = principal.clone();
    let read = move || recent_scope(&navigation, &principal, &node, store.as_ref());
    tokio::task::spawn_blocking(read)
        .await
        .map_err(|_| FileError::internal("the recent view task failed"))?
}

/// The caller's own latest change and opening of each file of the libraries of `scope`, read
/// from the journal on a blocking thread.
async fn read_my_times(
    state: &ApiState,
    principal: &Principal,
    scope: &RecentScope,
) -> Result<HashMap<String, MyTimes>, FileError> {
    let Some(store) = state.monitor().map(|monitor| monitor.store()) else {
        return Ok(HashMap::new());
    };
    let libraries: Vec<String> = scope
        .libraries
        .iter()
        .map(|library| library.library_id.clone())
        .collect();
    let principal = principal.clone();
    let read = move || {
        let mut mine = HashMap::new();
        for library_id in &libraries {
            mine.extend(my_times(store.as_ref(), principal.actor(), library_id)?);
        }
        Ok(mine)
    };
    tokio::task::spawn_blocking(read)
        .await
        .map_err(|_| FileError::internal("the recent view task failed"))?
}

/// Count the files and bytes beneath every folder row of `rows`, the rows of one answered
/// page, within the bounds of one view. The catalog is read on a blocking thread, then the
/// file service reads each file's object.
async fn count_folders(
    state: &ApiState,
    principal: &Principal,
    files: &dyn FileOperations,
    rows: &mut [ViewRow],
) -> Result<(), FileError> {
    let nodes = folder_nodes(rows);
    if nodes.is_empty() {
        return Ok(());
    }
    let navigation = navigation(state)?;
    let principal = principal.clone();
    let read = move || folder_files(&navigation, &principal, &nodes);
    let folders = tokio::task::spawn_blocking(read)
        .await
        .map_err(|_| FileError::internal("the folder count task failed"))??;
    for folder in folders {
        let row = folder.row;
        let count = count_folder(files, folder).await?;
        apply_count(rows, row, count);
    }
    Ok(())
}

/// The files and bytes of `folder` as the file service reads them. A library the file
/// service does not serve, and a file whose object or record cannot be found, make the count
/// incomplete instead of failing the answer.
async fn count_folder(
    files: &dyn FileOperations,
    folder: FolderFiles,
) -> Result<FolderCount, FileError> {
    if folder.files.is_empty() {
        return Ok(FolderCount {
            complete: folder.complete,
            ..FolderCount::default()
        });
    }
    let facts = match files.file_facts(folder.library_id, folder.files).await {
        Ok(facts) => facts,
        Err(err) if err.code() == crate::ErrorCode::NotFound => {
            return Ok(FolderCount::default());
        }
        Err(err) => return Err(err),
    };
    Ok(FolderCount {
        files: facts.facts.len() as u64,
        bytes: facts.facts.iter().map(|file| file.size).sum(),
        complete: folder.complete && facts.complete,
    })
}

/// The scope and the caller's own times of `request`, read on a blocking thread.
async fn read_view_inputs(
    state: &ApiState,
    principal: &Principal,
    request: &ViewRequest,
) -> Result<ViewInputs, FileError> {
    let navigation = navigation(state)?;
    let store = state.monitor().map(|monitor| monitor.store());
    let home = state.home_collections();
    let node = NodeRef::from_handle(&request.node).ok_or_else(FileError::not_found)?;
    let (principal, view) = (principal.clone(), request.view);
    let read = move || {
        let scope = view_scope(&navigation, &principal, &node, view, home.as_deref())?;
        let mut mine = HashMap::new();
        let libraries = store.as_deref().map(|journal| (journal, &scope.libraries));
        for (journal, library) in libraries
            .into_iter()
            .flat_map(|(j, l)| l.iter().map(move |x| (j, x)))
        {
            mine.extend(my_times(journal, principal.actor(), &library.library_id)?);
        }
        Ok(ViewInputs { scope, mine })
    };
    tokio::task::spawn_blocking(read)
        .await
        .map_err(|_| FileError::internal("the view task failed"))?
}

/// `GET /api/v1/files/search`: the files and folder rows of the caller's readable scope that
/// match a query.
async fn search(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Query<SearchRequest>, QueryRejection>,
) -> Response {
    let outcome = search_results(&state, &principal, request).await;
    read_answer(&state, SEARCH_OPERATION, &principal, outcome).await
}

/// The page a search request names. The query is parsed before anything is read, so an
/// unknown or unavailable operator never reads a file.
async fn search_results(
    state: &ApiState,
    principal: &Principal,
    request: Result<Query<SearchRequest>, QueryRejection>,
) -> Result<SearchPage, FileError> {
    let request = query(request)?;
    if request.q.trim().is_empty() {
        return Err(FileError::invalid("A search needs a query."));
    }
    let parsed = parse(&request.q)?;
    let zone = parse_zone(request.zone.as_deref())?;
    check_limit(request.limit)?;
    let files = files(state)?;
    let (scope, mine) =
        read_search_inputs(state, principal, &request, parsed.wants_deleted()).await?;
    let mut examined = Examined {
        mine,
        complete: scope.complete,
        ..Examined::default()
    };
    for library in scope.libraries {
        let ids = library.files.iter().map(|f| f.file_id.clone()).collect();
        let placed: HashMap<String, ScopedFile> = library
            .files
            .into_iter()
            .map(|f| (f.file_id.as_str().to_string(), f))
            .collect();
        let facts = files.file_facts(library.library_id.clone(), ids).await?;
        examined.complete &= facts.complete;
        let found = search_files(&library.library_id, facts.facts, &placed);
        examined.files.extend(found);
        examined.folders.extend(library.folders);
    }
    let searcher = Searcher {
        zone,
        owns_files: *principal == Principal::Owner,
    };
    let mut page = search_page(&request, &parsed, searcher, examined)?;
    count_folders(state, principal, files.as_ref(), &mut page.rows).await?;
    let repository = request
        .node
        .as_deref()
        .and_then(NodeRef::from_handle)
        .is_some_and(|node| matches!(node, NodeRef::Repository(_)));
    if repository {
        // The repository is answered as `null` however it was named.
        page.node = None;
    }
    Ok(page)
}

/// The scope and the caller's own times of a search, read on a blocking thread.
async fn read_search_inputs(
    state: &ApiState,
    principal: &Principal,
    request: &SearchRequest,
    deleted: bool,
) -> Result<(SearchScope, HashMap<String, MyTimes>), FileError> {
    let navigation = navigation(state)?;
    let store = state.monitor().map(|monitor| monitor.store());
    let node = request
        .node
        .as_deref()
        .map(|handle| NodeRef::from_handle(handle).ok_or_else(FileError::not_found))
        .transpose()?;
    let principal = principal.clone();
    let read = move || {
        let scope = search_scope(&navigation, &principal, node.as_ref(), deleted)?;
        let mut mine = HashMap::new();
        if let Some(store) = store {
            for library in &scope.libraries {
                mine.extend(my_times(
                    store.as_ref(),
                    principal.actor(),
                    &library.library_id,
                )?);
            }
        }
        Ok((scope, mine))
    };
    tokio::task::spawn_blocking(read)
        .await
        .map_err(|_| FileError::internal("the search task failed"))?
}

/// `GET /api/v1/files/collection-history-size`: the bytes the history of a collection's
/// files keeps.
async fn collection_history_size(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    request: Result<Query<CollectionSizeQuery>, QueryRejection>,
) -> Response {
    let outcome = collection_size(&state, &principal, request).await;
    read_answer(
        &state,
        COLLECTION_HISTORY_SIZE_OPERATION,
        &principal,
        outcome,
    )
    .await
}

/// The storage use of the collection a request names.
async fn collection_size(
    state: &ApiState,
    principal: &Principal,
    request: Result<Query<CollectionSizeQuery>, QueryRejection>,
) -> Result<CollectionHistorySizeView, FileError> {
    let request = query(request)?;
    let collection =
        CollectionId::new(&request.collection_id).map_err(|_| FileError::not_found())?;
    let node = NodeRef::Collection(collection);
    let navigation = navigation(state)?;
    let principal = principal.clone();
    let read = move || collection_files(&navigation, &principal, &node);
    let (library_id, ids, complete) = tokio::task::spawn_blocking(read)
        .await
        .map_err(|_| FileError::internal("the storage-use task failed"))??;
    let mut size = files(state)?
        .files_history_size(library_id, request.collection_id, ids)
        .await?;
    size.truncated |= !complete;
    Ok(size)
}

/// The installed navigation service.
fn navigation(state: &ApiState) -> Result<Arc<NavigationService>, FileError> {
    state
        .navigation()
        .ok_or_else(|| FileError::unavailable("no navigation service is installed"))
}
