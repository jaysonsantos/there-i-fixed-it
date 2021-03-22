use async_trait::async_trait;
use color_eyre::{eyre::Context, Result};
use lazy_static::lazy_static;
use regex::Regex;
use reqwest::{
    header::{HeaderMap, ACCEPT, CONTENT_TYPE, USER_AGENT},
    Client, ClientBuilder, Method, RequestBuilder,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, info, instrument, trace};

use crate::Repository;

use super::constants::OUR_USER_AGENT;
use super::{check_api_errors, fetch_from_cache, save_to_cache, Provider};

#[derive(Debug, Deserialize, Clone)]
pub struct GithubProvider {
    user: String,
    token: String,
    organization: String,
    #[serde(default = "default_url")]
    api_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PrCreateRequest<'a> {
    title: &'a str,
    body: Option<&'a str>,
    base: &'a str,
    head: &'a str,
}

#[derive(Debug, Deserialize)]
struct PrCreateResponse {
    url: String,
}

#[async_trait]
impl Provider for GithubProvider {
    #[instrument(skip(self))]
    async fn is_pr_open(&self, repository_name: &str, branch_name: &str) -> Result<bool> {
        let url = format!(
            "{}/repos/{}/{}/pulls",
            self.api_url, self.organization, repository_name
        );
        let head = format!("{}:{}", self.organization, branch_name);
        let response = self
            .request(Method::GET, &url)?
            .query(&[("head", head.as_str()), ("state", "open")])
            .send()
            .await?;

        let response = check_api_errors(response).await?;
        let body: Vec<Value> = response.json().await?;
        assert!(body.len() <= 1);
        Ok(body.len() >= 1)
    }

    #[instrument(skip(self),  fields(organization = self.organization.as_str()))]
    async fn open_pr(
        &self,
        repository_name: &str,
        base: &str,
        head: &str,
        title: &str,
        body: Option<&str>,
    ) -> Result<()> {
        debug!("openning pr");
        let url = format!(
            "{}/repos/{}/{}/pulls",
            self.api_url, self.organization, repository_name
        );
        let payload = PrCreateRequest {
            title,
            body,
            base,
            head,
        };
        let response = self
            .request(Method::POST, &url)?
            .json(&payload)
            .send()
            .await?;
        let response = check_api_errors(response)
            .await
            .wrap_err("failed to open pr")?;
        let rv: PrCreateResponse = response.json().await?;
        info!("pr created with url {}", rv.url);

        Ok(())
    }

    #[instrument(skip(self), fields(organization = self.organization.as_str()))]
    async fn list_repositories(&self, use_cache: bool) -> Result<Vec<Repository>> {
        if use_cache {
            if let Some(repositories) = fetch_from_cache("github", &self.organization).await? {
                trace!("using cached repositories");
                return Ok(repositories);
            }
        }
        trace!("fetching repositories");
        let mut output = vec![];
        let mut next_page_url = Some(format!(
            "{}/orgs/{}/repos?type=private&per_page=100&page=1",
            self.api_url, self.organization
        ));
        while let Some(url) = next_page_url.as_ref() {
            let (repositories, next_page) = self.list_repositories_per_page(&url).await?;
            output.extend(repositories);
            next_page_url = next_page
        }
        save_to_cache("github", &self.organization, &output).await?;
        Ok(output)
    }
}

impl GithubProvider {
    #[instrument(skip(self))]
    async fn list_repositories_per_page(
        &self,
        url: &str,
    ) -> Result<(Vec<Repository>, Option<String>)> {
        debug!("Fetching repositories on {}", &url);
        let response = self.request(Method::GET, &url)?.send().await?;

        let response = check_api_errors(response).await?;
        let link_header = response
            .headers()
            .get("link")
            .expect("github always send link")
            .to_str()
            .unwrap();
        let next_page = get_next_url(link_header).map(|p| p.to_string());

        let repositories: Vec<Repository> = response.json().await?;

        Ok((repositories, next_page))
    }

    fn request(&self, method: Method, url: &str) -> Result<RequestBuilder> {
        Ok(client()?
            .request(method, url)
            .basic_auth(&self.user, Some(&self.token)))
    }
}

fn client() -> Result<Client> {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, "application/vnd.github.v3+json".parse()?);
    headers.insert(CONTENT_TYPE, "application/json".parse()?);
    headers.insert(USER_AGENT, OUR_USER_AGENT.parse()?);

    let client = ClientBuilder::new().default_headers(headers).build()?;

    Ok(client)
}

fn default_url() -> String {
    "https://api.github.com".to_owned()
}

fn get_next_url(link_header: &str) -> Option<&str> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r#"<(.+?)>; rel="next""#).unwrap();
    }
    RE.captures(link_header)
        .map(|c| c.get(1))
        .flatten()
        .map(|m| m.as_str())
}

#[cfg(test)]
mod tests {
    use stub_server::start_wiremock;

    use crate::{providers::Provider, setup_error_handlers};

    use super::{get_next_url, GithubProvider};

    #[cfg(docker)]
    #[tokio::test]
    async fn test_github() {
        setup_error_handlers().ok();
        let base_url = start_wiremock().await.unwrap();
        let provider = GithubProvider {
            user: "test-user".to_string(),
            token: "bebacafe".to_string(),
            organization: "fix-it".to_string(),
            api_url: format!("{}/github", base_url),
        };

        let repositories = provider.list_repositories(false).await.unwrap();
        assert_eq!(repositories.len(), 2);
        let repository = &repositories[0];
        assert_eq!(repository.name, "fix-it-1");
        assert!(provider
            .is_pr_open("fix-it-1", "valid-branch")
            .await
            .expect("failed to check if a pr for valid branch is open"));
        assert!(!provider
            .is_pr_open("fix-it-1", "invalid-branch")
            .await
            .expect("failed to check if a pr for invalid branch is not open"));
        provider
            .open_pr("fix-it-2", "base", "head", "title", Some("body"))
            .await
            .expect("failed to open pr");
    }

    #[test]
    fn test_next_url() {
        let with_next = r#"</repos?type=private&per_page=100&page=2>; rel="next", </repos?type=private&per_page=100&page=1>; rel="first""#;
        let without_next = r#"</repos?type=private&per_page=100&page=1>; rel="first""#;
        assert_eq!(
            get_next_url(with_next),
            Some(r#"/repos?type=private&per_page=100&page=2"#)
        );
        assert_eq!(get_next_url(without_next), None);
        assert_eq!(get_next_url(""), None);
    }
}
