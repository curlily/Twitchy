use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct UserResponse {
    emote_set: EmoteSet,
}

#[derive(Debug, Deserialize)]
struct EmoteSet {
    emotes: Vec<Emote>,
}

#[derive(Debug, Deserialize)]
struct Emote {
    name: String,
}

pub async fn fetch_user_emotes(user_id: &str) -> Result<Vec<String>, reqwest::Error> {
    let client = Client::new();
    let url = format!("https://7tv.io/v3/users/twitch/{}", user_id);

    let resp: UserResponse = client.get(&url).send().await?.json().await?;

    // collect just the names
    Ok(resp.emote_set.emotes.into_iter().map(|e| e.name).collect())
}
