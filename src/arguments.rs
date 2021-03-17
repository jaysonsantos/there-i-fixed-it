use camino::Utf8PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Arguments {
    #[structopt(long, short("f"))]
    pub plan_file: Utf8PathBuf,
    #[structopt(long, short)]
    pub skip_repository_cache: bool,
}
