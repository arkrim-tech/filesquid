//! The drive routes of Task API-F2.2 (API-S-011, SRS-F-062): copy, the view operation and
//! storage use per collection. Reads take `files.read` and are not journaled when they
//! succeed; a copy takes `files.write` and is journaled with opaque IDs only. Every refusal
//! is journaled once.
//!
//! The view and the collection count read the host's catalog through the navigation service
//! and, for the view, the caller's own journal entries; both may block, so they run on a
//! blocking thread before the file service reads each file's object.

use super::command::{self, FileCommand};
use super::copy::CopyRequest;
use super::drive::{
    check_filter, check_limit, check_view, file_candidate, folder_candidate, matches_filter, page,
    Candidate, MyTimes, ViewPage, ViewRequest,
};
use super::drive_facts::{CollectionHistorySizeView, CollectionSizeQuery};
use super::drive_scope::{collection_files, my_times, view_scope, ViewScope};
use super::error::FileError;
use super::homing;
use super::provenance::Caller;
use super::routes::{body, files, query, read_answer, recorded, resumed_entry};
use super::search::{parse_zone, search_files, search_page, Examined, SearchPage, SearchRequest};
use super::search_match::Searcher;
use super::search_query::parse;
use super::search_scope::{search_scope, SearchScope};
use super::{COLLECTION_HISTORY_SIZE_OPERATION, COPY_OPERATION, SEARCH_OPERATION, VIEW_OPERATION};
use crate::auth::{Action, Principal};
use crate::home::Destination;
use crate::navigation::{NavigationService, NodeRef};
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
    let inputs = read_view_inputs(state, principal, &request).await?;
    let library_id = inputs.scope.library_id.clone();
    let facts = files.file_facts(library_id, inputs.scope.files).await?;
    let mut candidates: Vec<Candidate> = inputs
        .scope
        .folders
        .into_iter()
        .map(|folder| folder_candidate(folder, request.sort))
        .collect();
    candidates.extend(facts.facts.into_iter().map(|file| {
        let mine = inputs
            .mine
            .get(&file.object_id)
            .copied()
            .unwrap_or_default();
        file_candidate(file, mine, request.sort)
    }));
    candidates.retain(|candidate| matches_filter(candidate, filter.as_deref()));
    page(
        &request,
        candidates,
        inputs.scope.complete && facts.complete,
    )
}

/// The scope and the caller's own times of `request`, read on a blocking thread.
async fn read_view_inputs(
    state: &ApiState,
    principal: &Principal,
    request: &ViewRequest,
) -> Result<ViewInputs, FileError> {
    let navigation = navigation(state)?;
    let store = state.monitor().map(|monitor| monitor.store());
    let node = NodeRef::from_handle(&request.node).ok_or_else(FileError::not_found)?;
    let (principal, view) = (principal.clone(), request.view);
    let read = move || {
        let scope = view_scope(&navigation, &principal, &node, view)?;
        let mine = match store {
            Some(store) => my_times(store.as_ref(), principal.actor(), &scope.library_id)?,
            None => HashMap::new(),
        };
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
        let placed: HashMap<String, (Option<String>, bool)> = library
            .files
            .iter()
            .map(|f| {
                (
                    f.file_id.as_str().to_string(),
                    (f.collection.clone(), f.deleted),
                )
            })
            .collect();
        let ids = library.files.into_iter().map(|f| f.file_id).collect();
        let facts = files.file_facts(library.library_id, ids).await?;
        examined.complete &= facts.complete;
        examined.files.extend(search_files(facts.facts, &placed));
        examined.folders.extend(library.folders);
    }
    let searcher = Searcher {
        zone,
        owns_files: *principal == Principal::Owner,
    };
    let mut page = search_page(&request, &parsed, searcher, examined)?;
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
