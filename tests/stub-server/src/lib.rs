use std::{
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
    thread::sleep,
    time::Duration,
};

use lazy_static::lazy_static;

lazy_static! {
    static ref WIREMOCK: WiremockRunner = WiremockRunner::new();
    static ref SERVER_UP: AtomicBool = AtomicBool::new(false);
}

struct WiremockRunner {
    url: String,
}

impl WiremockRunner {
    pub fn new() -> Self {
        assert_eq!(
            Command::new("docker-compose")
                .args(&["up", "-d"])
                .status()
                .expect("failed to run docker-compose")
                .code(),
            Some(0)
        );
        let host = Command::new("docker-compose")
            .args(&["port", "wiremock", "8080"])
            .output()
            .expect("failed to get wiremock port");
        let url = format!("http://{}", String::from_utf8_lossy(&host.stdout).trim());

        Self { url }
    }

    async fn wait_for_server(&self) {
        if SERVER_UP.load(Ordering::Relaxed) {
            return;
        }
        let mut last_error = None;
        for _ in 0..30 {
            match reqwest::get(&format!("{}/__admin", &self.url)).await {
                Ok(_) => {
                    SERVER_UP.swap(true, Ordering::Relaxed);
                    return;
                }
                Err(err) => last_error = Some(err),
            }

            sleep(Duration::from_millis(300));
        }
        panic!("Docker never went up last error {:?}", last_error);
    }
}

pub async fn start_wiremock() -> &'static str {
    WIREMOCK.wait_for_server().await;
    WIREMOCK.url.as_str()
}
