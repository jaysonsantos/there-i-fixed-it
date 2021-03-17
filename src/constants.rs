use camino::Utf8PathBuf;
use directories::ProjectDirs;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref PROJECT_DIR: ProjectDirs =
        ProjectDirs::from("com", "Jayson Reis", "There I Fixed It")
            .expect("failed to find a directory for configs");
    pub static ref CACHE_DIR: Utf8PathBuf =
        Utf8PathBuf::from_path_buf(PROJECT_DIR.cache_dir().to_owned())
            .expect("failed to get a cache directory");
}
