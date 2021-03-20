use std::{
    fmt::Display,
    process::{Output, Stdio},
    sync::Arc,
};

use camino::{Utf8Path, Utf8PathBuf};
use color_eyre::{
    eyre::{eyre, Context},
    Help, Result, SectionExt,
};
use tokio::{fs, process::Command};
use tracing::{debug, info, instrument, trace};

use crate::Repository;

use super::{glob_pattern::GlobPattern, FileOperation, Plan};

pub struct PlanExecutor {
    plan: Arc<Plan>,
    repository: Repository,
    directory: Utf8PathBuf,
}

impl PlanExecutor {
    pub fn new(plan: Arc<Plan>, repository: Repository, repositories_folder: &Utf8Path) -> Self {
        let directory = repositories_folder.join("repos").join(&repository.name);

        Self {
            plan,
            repository,
            directory,
        }
    }
    #[instrument(skip(self), fields(repository_name = self.repository.name.as_str()))]
    pub async fn process(&self) -> Result<()> {
        debug!("started");

        self.clone_repository().await?;
        self.ensure_branch().await?;

        if !self.process_operations().await? {
            return Ok(());
        }

        self.commit().await?;
        self.push().await?;
        self.open_pr().await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn clone_repository(&self) -> Result<()> {
        if self.directory.exists() {
            debug!("Skipping");
            return Ok(());
        }

        let output = Command::new("git")
            .args(&["clone", &self.repository.ssh_url.as_str()])
            .arg(&self.directory)
            .stdin(Stdio::null())
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?
            .wait_with_output()
            .await?;
        check_process(&output).wrap_err("failed to clone repository")?;
        info!("done");
        Ok(())
    }

    #[instrument(skip(self), fields(directory = self.directory.as_str()))]
    async fn ensure_branch(&self) -> Result<()> {
        // git branch --show-current is only on git 2.22+
        let current_branch = self
            .git_output(&["rev-parse", "--abbrev-ref", "HEAD"])
            .await
            .wrap_err("failed to list branch")?;
        let current_branch = current_branch.trim();
        if current_branch == self.plan.branch_name {
            debug!("branch already checked out");
            return Ok(());
        }

        self.git_output(&["reset", "--hard"])
            .await
            .wrap_err("failed to reset branch")?;
        self.git_output(&["checkout", &self.repository.default_branch])
            .await
            .wrap_err("failed to checkout default branch")?;

        self.git_output(&["pull", "-r"])
            .await
            .wrap_err("failed to pull changes")?;

        let _ = self
            .git_output(&["checkout", "-b", self.plan.branch_name.as_str()])
            .await
            .wrap_err("failed to checkout new branch")?;
        debug!("changed to branch {}", self.plan.branch_name);
        Ok(())
    }

    #[instrument(skip(self))]
    async fn git_output(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("git")
            .args(args)
            .stdin(Stdio::null())
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .current_dir(&self.directory)
            .spawn()?
            .wait_with_output()
            .await?;
        Ok(check_process(&output)?)
    }

    async fn process_operations(&self) -> Result<bool> {
        let mut files_changed = false;
        for operation in &self.plan.file_operations {
            files_changed |= self.process_operation(operation).await?;
        }
        Ok(files_changed)
    }

    async fn process_operation(&self, operation: &FileOperation) -> Result<bool> {
        let files = self.list_files(&self.directory, &operation.pattern).await?;
        let files = files.iter().map(|f| f.as_path()).collect::<Vec<_>>();

        self.process_files(&files, operation).await
    }

    #[instrument(skip(self))]
    async fn list_files(
        &self,
        directory: &Utf8Path,
        pattern: &GlobPattern,
    ) -> Result<Vec<Utf8PathBuf>> {
        let mut output = vec![];
        let glob_pattern = directory.join(pattern.as_str());

        for entry in glob::glob(&glob_pattern.as_str())? {
            let entry = entry?;
            if !entry.is_file() {
                continue;
            }
            output.push(Utf8PathBuf::from_path_buf(entry).unwrap());
        }

        Ok(output)
    }

    #[instrument(skip(self, files))]
    async fn process_files(&self, files: &[&Utf8Path], operation: &FileOperation) -> Result<bool> {
        let mut files_changed = false;
        for file in files {
            files_changed |= self.process_file(file, operation).await?;
        }
        Ok(files_changed)
    }

    #[instrument(skip(self, operation))]
    async fn process_file(&self, file: &Utf8Path, operation: &FileOperation) -> Result<bool> {
        trace!("fixing file");
        let mut text = fs::read_to_string(file).await?;
        let mut changed = false;

        for processor in &operation.processors {
            changed |= processor.process(&mut text);
        }

        if !changed {
            return Ok(changed);
        }

        fs::write(file, &text).await?;

        trace!("done");
        Ok(true)
    }

    #[instrument(skip(self))]
    async fn commit(&self) -> Result<()> {
        debug!("committing");
        let last_commit = self.git_output(&["log", "--format=%B", "-n", "1"]).await?;
        if last_commit.starts_with(&format!("{}\n", &self.plan.git_message)) {
            debug!("commit already done");
            return Ok(());
        }
        self.git_output(&["commit", "-a", "-m", &self.plan.git_message])
            .await
            .wrap_err("failed to commit changes")?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn push(&self) -> Result<()> {
        debug!("pushing");
        let output = self
            .git_output(&["push", "-u", "-f", "origin", &self.plan.branch_name])
            .await
            .wrap_err("failed to push changes")?;
        trace!("git: {:?}", output);
        Ok(())
    }

    #[instrument(skip(self))]
    async fn open_pr(&self) -> Result<()> {
        if self
            .plan
            .get_provider()
            .is_pr_open(&self.repository.name, &self.plan.branch_name)
            .await?
        {
            info!("pr already opened");
            return Ok(());
        }

        let body = self.plan.pull_request_body.as_ref().map(|b| b.as_str());
        let title = self
            .plan
            .pull_request_title
            .as_ref()
            .unwrap_or(&&self.plan.git_message);

        self.plan
            .get_provider()
            .open_pr(
                &self.repository.name,
                &self.repository.default_branch,
                &self.plan.branch_name,
                title.as_str(),
                body,
            )
            .await?;
        info!("done");
        Ok(())
    }
}

impl Display for PlanExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.repository.name)
    }
}

fn check_process(output: &Output) -> Result<String> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        return Ok(stdout.to_string());
    }

    let err = eyre!("failed to run command")
        .with_section(move || format!("Exit code: {:?}", output.status.code()))
        .with_section(move || stdout.trim().to_string().header("Stdout:"))
        .with_section(move || stderr.trim().to_string().header("Stderr:"));

    Err(err)
}

#[cfg(test)]
mod tests {
    use std::{process::Stdio, sync::Arc};

    use camino::{Utf8Path, Utf8PathBuf};
    use tempdir::TempDir;
    use tokio::process::Command;

    use crate::{plan::plan_from_file, Repository};

    use super::PlanExecutor;
    use crate::plan::executor::check_process;

    #[tokio::test]
    async fn test_executor_flow() {
        crate::setup_error_handlers().ok();
        let plan_file = Utf8PathBuf::from("tests/fixtures/simple-plan.toml");
        let plan = Arc::new(plan_from_file(&plan_file).await.unwrap());

        let repositories = plan.get_provider().list_repositories(false).await.unwrap();
        assert_eq!(repositories.len(), 1);

        for repository in repositories {
            let (repository, temp) = create_fake_repository(repository).await;
            let path = Utf8Path::from_path(temp.path()).unwrap();
            let executor = PlanExecutor::new(plan.clone(), repository, path);
            executor.process().await.unwrap();
        }
    }

    async fn create_fake_repository(repository: Repository) -> (Repository, TempDir) {
        let temp = TempDir::new("fake-repository").unwrap();
        let setup = Utf8PathBuf::from("tests/create-test-repository.sh");

        let command = Command::new("bash")
            .arg("-x")
            .arg(&setup)
            .arg(&temp.path())
            .stdin(Stdio::null())
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        let output = command.wait_with_output().await.unwrap();
        check_process(&output).unwrap();

        let new_repository = Repository {
            ssh_url: temp
                .path()
                .join("destination.git")
                .to_string_lossy()
                .to_string(),
            ..repository
        };

        (new_repository, temp)
    }
}
