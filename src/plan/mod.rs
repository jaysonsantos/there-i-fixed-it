pub mod executor;
pub mod glob_pattern;

use camino::Utf8Path;
use color_eyre::{eyre::Context, Result};
use regex::Regex;
use serde::Deserialize;
use tokio::fs;
use tracing::instrument;

use crate::providers::{GithubProvider, Provider};

pub use self::executor::PlanExecutor;
use self::glob_pattern::GlobPattern;

#[cfg(test)]
use crate::providers::tests::TestProvider;

#[derive(Debug, Deserialize)]
pub struct Plan {
    branch_name: String,
    git_message: String,
    pull_request_title: Option<String>,
    pull_request_body: Option<String>,
    #[serde(rename = "files")]
    file_operations: Vec<FileOperation>,
    provider: PlanProvider,
    #[serde(rename = "repositories")]
    /// There is no default just to be explicit and avoid applying changes on all repositories
    repository_allow_filters: Vec<GlobPattern>,
    #[serde(rename = "deny_repositories", default)]
    repository_deny_filters: Vec<GlobPattern>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "name", rename_all = "snake_case")]
pub enum PlanProvider {
    Github(GithubProvider),
    #[cfg(test)]
    Test(TestProvider),
}

#[derive(Debug, Deserialize)]
pub struct FileOperation {
    #[serde(rename = "glob")]
    pattern: GlobPattern,
    processors: Vec<Processor>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Processor {
    Regex(RegexProcessor),
}

#[derive(Debug, Deserialize)]
pub struct RegexProcessor {
    operations: Vec<RegexOperation>,
}

#[derive(Debug, Deserialize)]
pub struct RegexOperation {
    #[serde(with = "serde_regex")]
    from: Regex,
    to: String,
}

#[instrument]
pub async fn plan_from_file(path: &Utf8Path) -> Result<Plan> {
    let contents = fs::read_to_string(path)
        .await
        .wrap_err_with(|| format!("failed to read plan file from {:?}", path))?;

    plan_from_str(&contents).wrap_err_with(|| format!("failed to parse {:?}", path))
}

#[instrument(skip(plan))]
pub fn plan_from_str(plan: &str) -> Result<Plan> {
    toml::from_str(plan).wrap_err("failed to parse plan")
}

impl Plan {
    pub fn get_provider(&self) -> Box<&dyn Provider> {
        match &self.provider {
            PlanProvider::Github(provider) => Box::new(provider),
            #[cfg(test)]
            PlanProvider::Test(provider) => Box::new(provider),
        }
    }

    pub fn repository_allowed(&self, repository_name: &str) -> bool {
        self.repository_allow_filters
            .iter()
            .any(|f| f.matches(repository_name))
            && !self.repository_denied(repository_name)
    }

    fn repository_denied(&self, repository_name: &str) -> bool {
        self.repository_deny_filters
            .iter()
            .any(|f| f.matches(repository_name))
    }
}

impl Processor {
    pub fn process(&self, text: &str) -> String {
        // TODO: Find a way to make this CoW
        let mut text = text.to_string();
        match self {
            Processor::Regex(processor) => {
                for operation in &processor.operations {
                    text = operation.from.replace_all(&text, &operation.to).to_string()
                }
                text
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;

    use super::plan_from_file;

    #[tokio::test]
    async fn test_filters() {
        let plan_file = Utf8PathBuf::from("tests/fixtures/simple-plan.toml");
        let plan = plan_from_file(&plan_file).await.unwrap();

        assert!(plan.repository_allowed("my-repo"));
        assert!(plan.repository_allowed("abc-rs-my-repo"));
        assert!(!plan.repository_allowed("my-repo-rs"));
    }
}
