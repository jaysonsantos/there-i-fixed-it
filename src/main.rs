use std::sync::Arc;

use color_eyre::{eyre::Context, Result};
use serde::{Deserialize, Serialize};
use structopt::StructOpt;
use tokio::{sync::Semaphore, task};
use tracing::info;
use tracing_error::ErrorLayer;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use crate::arguments::Arguments;
use crate::constants::CACHE_DIR;
use crate::plan::{plan_from_file, PlanExecutor};

mod arguments;
mod constants;
mod plan;
mod providers;

#[derive(Debug, Serialize, Deserialize)]
pub struct Repository {
    name: String,
    private: bool,
    fork: bool,
    ssh_url: String,
    default_branch: String,
}

pub(crate) fn setup_error_handlers() -> Result<()> {
    if tracing::dispatcher::has_been_set() {
        return Ok(());
    }
    let error_layer = ErrorLayer::default();
    let filter_layer = EnvFilter::try_from_default_env().or_else(|_| EnvFilter::try_new("info"))?;
    let fmt_layer = fmt::layer().with_target(false);

    tracing_subscriber::Registry::default()
        .with(error_layer)
        .with(filter_layer)
        .with(fmt_layer)
        .try_init()?;

    color_eyre::install()?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    setup_error_handlers()?;

    let arguments = Arguments::from_args();
    info!("parsing plan");
    let plan = Arc::new(plan_from_file(&arguments.plan_file).await?);
    let provider = plan.get_provider();
    let all_repositories = provider
        .list_repositories(!arguments.skip_repository_cache)
        .await?;

    let executors = all_repositories
        .into_iter()
        .filter(|repository| plan.repository_allowed(&repository.name))
        .map(|repository| PlanExecutor::new(plan.clone(), repository, &CACHE_DIR))
        .collect::<Vec<_>>();

    let mut futures = vec![];

    let s = Arc::new(Semaphore::new(5));
    for executor in executors {
        let permit = s.clone().acquire_owned().await?;
        futures.push(task::spawn(async move {
            let _ = permit;
            executor
                .process()
                .await
                .wrap_err(format!("failed to process repository {}", executor))
        }))
    }

    for future in futures {
        let _response = future.await??;
    }

    info!("process done");

    Ok(())
}
