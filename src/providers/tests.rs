use async_trait::async_trait;
use color_eyre::Result;
use serde::Deserialize;
use tracing::instrument;

use crate::Repository;

use super::Provider;

#[derive(Debug, Deserialize, Clone)]
pub struct TestProvider;

#[async_trait]
impl Provider for TestProvider {
    #[instrument(skip(self))]
    async fn is_pr_open(&self, _repository_name: &str, _branch_namee: &str) -> Result<bool> {
        Ok(false)
    }

    #[instrument(skip(self))]
    async fn open_pr(
        &self,
        _repository_name: &str,
        _base: &str,
        _head: &str,
        _title: &str,
        _body: Option<&str>,
    ) -> Result<()> {
        Ok(())
    }

    #[instrument(skip(self))]
    async fn list_repositories(&self, _use_cache: bool) -> Result<Vec<Repository>> {
        Ok(vec![Repository {
            name: "working-repo".to_string(),
            private: true,
            fork: false,
            ssh_url: "any-url".to_string(),
            default_branch: "main".to_string(),
        }])
    }
}
