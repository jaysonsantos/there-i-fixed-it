mod constants;
mod github;
#[cfg(test)]
pub(crate) mod tests;

use async_trait::async_trait;
use color_eyre::{
    eyre::{eyre, Context},
    Help, Result, SectionExt,
};
use serde::{de::DeserializeOwned, Serialize};
use tokio::fs;

use crate::{constants::CACHE_DIR, Repository};

pub use self::github::GithubProvider;

#[async_trait]
pub trait Provider: Sync + Send {
    async fn is_pr_open(&self, repository_name: &str, branch_name: &str) -> Result<bool>;
    async fn open_pr(
        &self,
        repository_name: &str,
        base: &str,
        head: &str,
        title: &str,
        body: Option<&str>,
    ) -> Result<()>;
    async fn list_repositories(&self, use_cache: bool) -> Result<Vec<Repository>>;
}

pub(crate) async fn check_api_errors(response: reqwest::Response) -> Result<reqwest::Response> {
    match response.error_for_status_ref() {
        Err(source) => match response.text().await {
            Ok(body) => Err(eyre!(source)
                .with_section(move || body.trim().to_string().header("Body: ").to_string())),
            Err(err) => {
                return Err(eyre!(err));
            }
        },
        _ => Ok(response),
    }
}

pub(crate) async fn fetch_from_cache<T>(
    provider_name: &str,
    organization: &str,
) -> Result<Option<T>>
where
    T: DeserializeOwned,
{
    let file = CACHE_DIR.join(format!(
        "{}-{}-{}.json",
        "repositories", provider_name, organization
    ));
    if !file.exists() {
        return Ok(None);
    }

    Ok(serde_json::from_slice(&fs::read(file).await?)?)
}

pub(crate) async fn save_to_cache<T>(provider_name: &str, organization: &str, data: T) -> Result<()>
where
    T: Serialize,
{
    let contents = serde_json::to_vec_pretty(&data)?;
    let path = CACHE_DIR.join(format!(
        "{}-{}-{}.json",
        "repositories", provider_name, organization
    ));
    let project_folder = path
        .parent()
        .ok_or_else(|| eyre!("failed to determine project folder"))?;
    fs::create_dir_all(&project_folder).await?;
    fs::write(&path, &contents)
        .await
        .wrap_err_with(|| format!("failed to save cache {}", path))?;
    Ok(())
}
