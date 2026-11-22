//! The self-scoped journal route (API-S-031 **Self-scoped journal**, ADR 033):
//!
//! - `GET /journal`: one page of the journal entries whose actor is the caller.
//!
//! The route performs [`Action::ReadOwnJournal`] (`journal.read_own`), which every accepted
//! credential is granted whatever the policy says, so a caller who is not an administrator can
//! read the record of their own activity. It takes the filters, cursor paging and redaction of
//! `GET /admin/journal` ([`crate::admin::journal`]) and answers the same
//! [`crate::admin::JournalViewPage`]; display names (`names=true`) are resolved for the
//! caller, so only for files the caller may read. The actor is never a filter here: it is
//! always the caller ([`Principal::actor`]), and an `actor` parameter is refused. The route
//! grants none of the admin routes, which keep their own action. On the desktop the caller is
//! the owner.
//!
//! A successful read is not journaled (API-S-017); a refused call is journaled by the
//! authorisation layer like any other.

use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Extension;

use super::JournalQuery;
use crate::admin::journal::{answer_page, bad_request, monitor_params, JournalParams};
use crate::auth::{Action, Principal};
use crate::routes::Route;
use crate::ApiState;

/// The route's path, relative to the API prefix.
pub const OWN_JOURNAL_PATH: &str = "/journal";

/// The self-scoped journal route, relative to the API prefix.
pub(crate) fn route() -> Route {
    (OWN_JOURNAL_PATH, get(own_page), Action::ReadOwnJournal)
}

/// `GET /journal`: one page of the caller's own entry views.
async fn own_page(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    params: Result<Query<JournalParams>, QueryRejection>,
) -> Response {
    let (monitor, params) = match monitor_params(&state, params) {
        Ok(found) => found,
        Err(error) => return error.into_response(),
    };
    let (query, asked) = match own_query(&params, &principal) {
        Ok(found) => found,
        Err(param) => return bad_request(param).into_response(),
    };
    answer_page(
        &state,
        &monitor,
        &principal,
        query,
        asked,
        "api.journal.own",
    )
    .await
}

/// The store query of `params` limited to the entries whose actor is `principal`, and
/// whether display names are asked for. An `actor` parameter is `Err("actor")`: the caller
/// cannot choose whose entries to read. Any other `Err` names the invalid parameter. Pure.
fn own_query(
    params: &JournalParams,
    principal: &Principal,
) -> Result<(JournalQuery, bool), &'static str> {
    if params.filters_actor() {
        return Err("actor");
    }
    let (query, asked) = params.to_named_query()?;
    let own = JournalQuery {
        actor: Some(principal.actor().to_string()),
        ..query
    };
    Ok((own, asked))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parameters parsed from `query`, as the route reads them.
    fn params(query: &str) -> JournalParams {
        let uri: axum::http::Uri = format!("/journal?{query}").parse().expect("uri");
        let Query(params) = Query::<JournalParams>::try_from_uri(&uri).expect("query");
        params
    }

    #[test]
    fn the_query_is_always_limited_to_the_caller() {
        for principal in [Principal::Owner, Principal::Administrator] {
            let (query, asked) =
                own_query(&params("operation=file.upload&names=true"), &principal).expect("query");
            assert_eq!(query.actor.as_deref(), Some(principal.actor()));
            assert_eq!(query.operation.as_deref(), Some("file.upload"));
            assert!(asked);
        }
    }

    #[test]
    fn an_actor_parameter_is_refused_even_when_it_names_the_caller() {
        for actor in ["owner", "administrator", "system", ""] {
            let asked = params(&format!("actor={actor}"));
            assert_eq!(own_query(&asked, &Principal::Owner).unwrap_err(), "actor");
        }
    }

    #[test]
    fn an_invalid_filter_names_its_parameter() {
        let refused = own_query(&params("limit=0"), &Principal::Owner);
        assert_eq!(refused.unwrap_err(), "limit");
    }
}
