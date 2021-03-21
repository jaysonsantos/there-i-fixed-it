use std::{env::var_os, process::Command};

fn main() {
    let has_docker = Command::new("docker-compose")
        .arg("--version")
        .spawn()
        .is_ok();
    let in_ci = var_os("CI").is_some();
    // Windows on github has docker but only runs windows images
    let allowed_in_ci = !in_ci || cfg!(linux);
    if has_docker && allowed_in_ci {
        println!("cargo:rustc-cfg=docker");
    }
}
