use serde::Deserialize;
use tide::{convert::json, Result};

use super::Request;

#[derive(Debug, Deserialize)]
struct ListRepositoriesQuery {
    page: u8,
}

#[derive(Debug, Deserialize)]
struct ListPrQuery {
    head: String,
    state: String,
}

#[derive(Debug, Deserialize)]
struct CreatePrRequest {
    head: String,
    base: String,
    title: String,
    body: Option<String>,
}

pub(crate) async fn list_repositories(request: Request) -> Result {
    let query: ListRepositoriesQuery = request.query()?;
    if query.page > 2 {
        return Ok(json!([]).into());
    }
    Ok(json!([{
        "name": format!("fix-it-{}", query.page),
        "private": true,
        "fork": false,
        "ssh_url": "",
        "default_branch": "main",
    }])
    .into())
}

pub(crate) async fn list_pull_requests(request: Request) -> Result {
    let query: ListPrQuery = request.query()?;
    assert_eq!(query.state, "open");
    if query.head == format!("{}:invalid-branch", request.param("organization")?) {
        return Ok(json!([]).into());
    }
    Ok(json!([{}]).into())
}

pub(crate) async fn create_pull_request(mut request: Request) -> Result {
    let payload: CreatePrRequest = request.body_json().await?;
    assert_eq!(payload.head, "head");
    assert_eq!(payload.base, "base");
    assert_eq!(payload.title, "title");
    assert_eq!(payload.body, Some("body".to_owned()));
    Ok(json!({
        "url": "http://localhost/your-pr"
    })
    .into())
}
