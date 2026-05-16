use std::{
    net::TcpListener,
    sync::{Arc, atomic::AtomicBool},
};

use assignment::{GitHubClient, run_server};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let address = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(&address)?;
    let service = Arc::new(GitHubClient::new());
    let shutdown = Arc::new(AtomicBool::new(false));

    println!("listening on http://{address}");
    run_server(listener, service, shutdown);

    Ok(())
}
