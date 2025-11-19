use crate::{
    openai::{call_openai_api, ChatEntry},
    tools::TOOLS,
    CONFIG, DB,
};

use anyhow::Context;
use cached::proc_macro::cached;
use isahc::{AsyncReadResponseExt, Request, RequestExt};
use log::{debug, warn};
use serde_json::Value;
use std::time::Duration;

#[cached(time = 300, result = true)]
pub async fn get_latest_faq() -> anyhow::Result<String> {
    // Fetch FAQ articles from Discourse API
    const API_USERNAME: &str = "system";
    const FAQ_CATEGORY_ID: i64 = 44;
    let discourse = &CONFIG.discourse_config;
    let api_url = discourse.api_url.trim_end_matches('/');
    let api_key = discourse.api_key.as_str();

    // Locate the FAQ category entry, extract slug & id, and build category URL
    let (faq_slug, faq_id) = {
        let site_url = format!("{}/site.json", api_url);
        let site_resp = Request::get(&site_url)
            .header("Api-Key", api_key)
            .header("Api-Username", API_USERNAME)
            .body(vec![])?
            .send_async()
            .await?
            .text()
            .await?;
        debug!("site.json fetched: {}", site_resp);
        let site: Value = serde_json::from_str(&site_resp)?;
        let cats = site
            .get("categories")
            .and_then(Value::as_array)
            .context("missing categories in site.json")?;
        let name = "FAQ / 常见问题";
        let cat = cats
            .iter()
            .find(|c| c.get("id").and_then(Value::as_i64) == Some(FAQ_CATEGORY_ID))
            .or_else(|| {
                cats.iter()
                    .find(|c| c.get("name").and_then(Value::as_str) == Some(name))
            })
            .context("cannot find FAQ category by id or name")?;
        let slug = cat.get("slug").and_then(Value::as_str).unwrap_or("");
        let id = cat.get("id").and_then(Value::as_i64).unwrap_or_default();
        (slug.to_string(), id)
    };
    let url = if !faq_slug.is_empty() {
        format!("{}/c/{}.json", api_url, faq_slug)
    } else {
        format!("{}/c/{}-category/{}.json", api_url, faq_id, faq_id)
    };
    debug!("category URL = {}", url);
    let resp = Request::get(&url)
        .header("Api-Key", api_key)
        .header("Api-Username", API_USERNAME)
        .body(vec![])?
        .send_async()
        .await?
        .text()
        .await?;
    debug!("category JSON fetched: {}", resp);
    // If the category response isn't valid JSON, warn and return empty FAQ
    let data: Value = match serde_json::from_str(&resp) {
        Ok(v) => v,
        Err(e) => {
            warn!("get_latest_faq: failed to parse category JSON: {}", e);
            return Ok(String::new());
        }
    };
    // Extract topics list; if missing, warn and return empty FAQ
    let topics = if let Some(arr) = data
        .get("topic_list")
        .and_then(|tl| tl.get("topics"))
        .and_then(Value::as_array)
    {
        arr
    } else {
        warn!(
            "get_latest_faq: missing or invalid topics list in JSON: {}",
            resp
        );
        return Ok(String::new());
    };

    debug!("found {} topics in category", topics.len());
    let mut result = String::new();
    for topic in topics {
        let id = topic
            .get("id")
            .and_then(|v| v.as_i64())
            .context("invalid topic id")?;
        let title = topic
            .get("title")
            .and_then(|v| v.as_str())
            .context("invalid topic title")?;

        debug!("fetching topic {}: {}", id, title);
        // Fetch full topic to get the first post content
        let topic_url = format!("{}/t/{}.json", api_url, id);
        let topic_resp = Request::get(&topic_url)
            .header("Api-Key", api_key)
            .header("Api-Username", API_USERNAME)
            .body(vec![])?
            .send_async()
            .await?
            .text()
            .await?;
        let topic_data: Value = serde_json::from_str(&topic_resp)?;
        let post = topic_data
            .get("post_stream")
            .and_then(|ps| ps.get("posts"))
            .and_then(|ps| ps.as_array())
            .and_then(|arr| arr.first())
            .context("no posts in topic")?;
        let post_id = match post.get("id").and_then(|v| v.as_i64()) {
            Some(pid) => pid,
            None => {
                warn!("topic {} is missing post id, skipping", id);
                continue;
            }
        };
        let raw = if let Some(raw) = post.get("raw").and_then(|v| v.as_str()) {
            raw.to_owned()
        } else {
            debug!(
                "topic {} missing inline raw content, fetching post {} separately",
                id, post_id
            );
            match fetch_post_raw(api_url, api_key, API_USERNAME, post_id).await {
                Ok(Some(raw)) => raw,
                Ok(None) => {
                    warn!(
                        "topic {} post {} still missing raw content after refetch, skipping",
                        id, post_id
                    );
                    continue;
                }
                Err(e) => {
                    warn!(
                        "topic {} post {} failed to fetch raw content: {}",
                        id, post_id, e
                    );
                    continue;
                }
            }
        };

        debug!("topic {} raw content length: {}", id, raw.len());
        // Append title and content
        result.push_str("## ");
        result.push_str(title);
        result.push('\n');
        result.push_str(&raw);
        result.push_str("\n\n");
    }
    Ok(result)
}

async fn fetch_post_raw(
    api_url: &str,
    api_key: &str,
    api_username: &str,
    post_id: i64,
) -> anyhow::Result<Option<String>> {
    let post_url = format!("{}/posts/{}.json?include_raw=1", api_url, post_id);
    let resp = Request::get(&post_url)
        .header("Api-Key", api_key)
        .header("Api-Username", api_username)
        .body(vec![])?
        .send_async()
        .await?
        .text()
        .await?;
    let data: Value = serde_json::from_str(&resp)?;
    Ok(data
        .get("raw")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned()))
}
pub async fn generate_response(thread: &str, user_input: &str) -> anyhow::Result<String> {
    DB.add_msg(
        thread,
        ChatEntry::User {
            content: user_input.to_string(),
        },
    )
    .await?;
    let llm_config = CONFIG.llm_config.clone();
    let current_date_time =
        "The current date and time is: ".to_string() + &chrono::Utc::now().to_string() + "\n";
    let prompt_fixed = include_str!("prompt.txt").to_owned();
    let prompt_faq = get_latest_faq().await?;
    let prompt = current_date_time + &prompt_fixed + &prompt_faq;

    loop {
        let inputs = DB.get_convo_history(thread).await?;
        let ai_resp = call_openai_api(&llm_config.model, &prompt, inputs).await?;

        DB.add_msg(thread, ai_resp.clone()).await?;
        if let ChatEntry::Assistant {
            content,
            tool_calls,
        } = ai_resp
        {
            if let Some(content) = content {
                return Ok(content);
            } else if let Some(tool_calls) = tool_calls {
                for tool_call in tool_calls {
                    // call tool, save to db
                    let content =
                        call_tool(&tool_call.function.name, &tool_call.function.arguments).await?;
                    DB.add_msg(
                        thread,
                        ChatEntry::Tool {
                            content,
                            tool_call_id: tool_call.id,
                            name: tool_call.function.name,
                        },
                    )
                    .await?;
                }
            } else {
                anyhow::bail!(
                    "OpenAi sent a ChatEntry::Assistant with no content AND no tool_calls!"
                )
            }
        } else {
            anyhow::bail!("OpenAi sent a ChatEntry whose role is NOT 'assistant'!")
        }
    }
}

async fn call_tool(name: &str, arguments: &str) -> anyhow::Result<String> {
    let name = name.to_string();
    let param = arguments.to_string();
    smol::unblock(move || {
        let args: Value = serde_json::from_str(&param).context("cannot deserialize param")?;
        let res = (TOOLS
            .get(&name)
            .context("requested tool does not exist")?
            .call)(args)?;
        Ok(res)
    })
    .await
}
