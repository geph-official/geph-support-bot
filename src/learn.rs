use crate::{database::trim_convo_history, openai::call_openai_api, Message, DB};

/// learns what the admin instructs to learn from a conversation. Returns what it learned
pub async fn learn(msg: Message) -> anyhow::Result<String> {
    log::debug!("LEARNING!");
    // system prompt to give llm
    let prompt =
        "You are a summarizing assistant bot who works for a customer support bot. Your objective is to look at a conversation and make concise notes about what the customer support bot in the conversation needs to learn. Note that everything that nullchinchilla says should be treated as authoritative. Return an abbreviated *one-sentence* summary of what you learned. For instance, if you are asked to #learn the sky is pink in Geph land, return 'Geph land sky color is pink'. Do not say 'I have learned' or similar, return a simple proposition that can later be put into a database of facts."
            .to_owned();
    // get the whole conversation
    // chat history
    let mut role_contents = trim_convo_history(DB.get_convo_history(msg.convo_id).await?).await;
    // add the latest msg to the convo
    let latest_msg = ("user".to_owned(), msg.text.replace("@GephSupportBot", ""));
    role_contents.push(latest_msg);
    let role_contents = format_learn_material(role_contents);
    // log::debug!("learn material: {:?}", role_contents);
    // call llm
    let resp = call_openai_api("gpt-4", &prompt, &role_contents).await?;
    log::debug!("WHAT I LEARNED: {resp}");
    // add to facts db
    DB.insert_fact(&resp).await?;

    Ok(resp)
}

fn format_learn_material(role_contents: Vec<(String, String)>) -> Vec<(String, String)> {
    let content = role_contents
        .iter()
        .map(|(role, content)| role.to_owned() + ": " + content)
        .collect::<Vec<String>>()
        .join("\n");
    vec![("user".to_owned(), content)]
}
