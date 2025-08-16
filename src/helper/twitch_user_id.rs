use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize)]
struct TwitchUserData {
    id: String,
    login: String,
}

#[derive(Deserialize)]
struct TwitchUserResponse {
    data: Vec<TwitchUserData>,
}

pub async fn get_twitch_user_id(username: &str, client_id: &str, oauth: &str) -> anyhow::Result<String> {

    let url = format!("https://api.twitch.tv/helix/users?login={}", username);
    let resp = Client::new()
        .get(&url)
        .header("Client-ID", client_id)
        .bearer_auth(oauth)
        .send()
        .await?
        .json::<TwitchUserResponse>()
        .await?;

    Ok(resp.data.get(0).ok_or_else(|| anyhow::anyhow!("User not found"))?.id.clone())
}
