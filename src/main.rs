mod adapters;
mod decision;

use std::error::Error;

use tokio::net::TcpListener;

const HOOK_ADDR: &str = "0.0.0.0:8080";
const EXTAUTHZ_ADDR: &str = "0.0.0.0:9000";

type BoxError = Box<dyn Error + Send + Sync>;

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    let hook_listener = TcpListener::bind(HOOK_ADDR).await?;
    let hook = axum::serve(hook_listener, adapters::hook::router());
    println!("WithHuman hook API listening on {HOOK_ADDR}");

    let extauthz = tonic::transport::Server::builder()
        .add_service(adapters::extauthz::service())
        .serve(EXTAUTHZ_ADDR.parse()?);
    println!("WithHuman extAuthz listening on {EXTAUTHZ_ADDR}");

    tokio::try_join!(
        async { hook.await.map_err(BoxError::from) },
        async { extauthz.await.map_err(BoxError::from) },
    )?;

    Ok(())
}
