pub mod admin;
pub mod webhook;

use axum::Router;
use tokio::net::TcpListener;

pub struct Webserver {
    port: u16,
    router: Router,
}

impl Webserver {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            router: Router::new(),
        }
    }

    pub fn register(mut self, routes: Router) -> Self {
        self.router = self.router.merge(routes);
        self
    }

    pub async fn run(self) {
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = TcpListener::bind(&addr)
            .await
            .expect("Failed to bind HTTP server");
        tracing::info!("HTTP server listening on {}", addr);

        axum::serve(listener, self.router).await.ok();
    }
}
