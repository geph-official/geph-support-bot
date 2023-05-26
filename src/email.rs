use std::collections::HashMap;

use async_compat::CompatExt;
use warp::Filter;

pub async fn handle_email() -> anyhow::Result<()> {
    let echo = warp::path("support-bot-email")
        .and(warp::post())
        .and(warp::body::form())
        .map(|form: HashMap<String, String>| {
            let form = format!("{:?}", form);
            log::debug!("{}", form);
            form
        });

    warp::serve(echo).run(([0, 0, 0, 0], 3030)).compat().await;
    // receives emails

    // gets response & acts

    // sends reply
    Ok(())
}
