use std::sync::{Arc, Mutex};

use rand::{thread_rng, Rng};
use tide::{listener::ToListener, prelude::*, utils::After, Request as TideRequest, Response};
use tokio::{
    sync::oneshot,
    task::{self, JoinHandle},
};

mod github;

type Request = TideRequest<Arc<Mutex<State>>>;

#[derive(Debug, Default)]
struct State;

pub struct StubServer {
    pub url: String,
    handle: JoinHandle<()>,
}

impl Drop for StubServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

pub async fn start_stub_server() -> StubServer {
    let (tx, rx) = oneshot::channel::<()>();
    let mut rng = thread_rng();
    let port: u16 = rng.gen_range(12000..20000);
    let url = format!("http://127.0.0.1:{}", port);
    let mut app = tide::with_state(Arc::new(Mutex::new(State::default())));

    app.at("/github/orgs/:organization/repos")
        .get(github::list_repositories);
    app.at("/github/repos/:organization/:repository/pulls")
        .get(github::list_pull_requests)
        .post(github::create_pull_request);

    app.with(After(|mut res: Response| async {
        if let Some(error) = res.error() {
            let msg = format!("{:?}", error);
            res.set_body(msg);
        }
        Ok(res)
    }));

    let mut listener = format!("127.0.0.1:{}", port).to_listener().unwrap();
    let handle = task::spawn(async move {
        listener.bind(app).await.unwrap();
        tx.send(()).unwrap();
        println!("bound");
        listener.accept().await.unwrap();
    });

    rx.await.unwrap();
    StubServer { url, handle }
}
