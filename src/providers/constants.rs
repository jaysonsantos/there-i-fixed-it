use lazy_static::lazy_static;

lazy_static! {
    pub static ref OUR_USER_AGENT: String = format!(
        "{}/{} +({})",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_REPOSITORY")
    );
}
