use async_compat::CompatExt;
use warp::Filter;

pub async fn handle_email() -> anyhow::Result<()> {
    let hello_world = warp::path("support-bot-email").map(|| "Hello, World!");
    warp::serve(hello_world)
        .run(([0, 0, 0, 0], 3030))
        .compat()
        .await;
    // receives emails

    // gets response & acts

    // sends reply
    Ok(())
}
