use reqwest::Client;
use serde_json::{json, Value};

pub async fn get_stream_category(
    client: &Client,
    oauth_token: &str,
    client_id: &str,
    broadcaster_id: &str,
) -> Option<String> {
    let resp = client
        .get("https://api.twitch.tv/helix/channels")
        .query(&[("broadcaster_id", broadcaster_id)])
        .header("Client-ID", client_id)
        .header("Authorization", format!("Bearer {}", oauth_token))
        .send()
        .await;

    if let Ok(resp) = resp {
        if let Ok(json) = resp.json::<Value>().await {
            if let Some(data) = json["data"].as_array() {
                if let Some(channel) = data.get(0) {
                    return channel["game_name"].as_str().map(|s| s.to_string());
                }
            }
        }
    }

    None
}

pub async fn get_game_id(
    client: &Client,
    oauth_token: &str,
    client_id: &str,
    category_name: &str,
) -> Option<String> {
    // Get the game ID from the game name
    let game_resp = client
        .get("https://api.twitch.tv/helix/games")
        .query(&[("name", category_name)])
        .header("Client-ID", client_id)
        .header("Authorization", format!("Bearer {}", oauth_token))
        .send()
        .await;

    match game_resp {
        Ok(resp) => {
            let json: Value = resp.json().await.unwrap_or_default();
            if let Some(id) = json["data"][0]["id"].as_str() {
                Some(id.to_string())
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

pub async fn update_stream_category(
    client: &Client,
    oauth_token: &str,
    client_id: &str,
    broadcaster_id: &str,
    category_name: &str,
) -> String {

    let game_id = get_game_id(client, oauth_token, client_id, broadcaster_id).await;

    // Update the channel's category
    let body = json!({
        "game_id": game_id.clone().unwrap_or("509658".to_string()) // Fallback to "Just Chatting" if none
    });

    let update_resp = client
        .patch(&format!("https://api.twitch.tv/helix/channels?broadcaster_id={}", broadcaster_id))
        .header("Client-ID", client_id)
        .header("Authorization", format!("Bearer {}", oauth_token))
        .json(&body)
        .send()
        .await;

    match update_resp {
        Ok(resp) if resp.status().is_success() => {
            format!("[Auto Category] {}Category updated to '{}'", if game_id == None {
                format!("'{}' not found! ", category_name)
            } else {
                "".to_string()
            }, category_name)
        }
        Ok(resp) => {
            format!("[Auto Category] Failed to update category: {}", resp.status())
        }
        Err(err) => {
            format!("[Auto Category] Error updating category: {}", err)
        }
    }
}

pub async fn is_channel_live(client: &Client, oauth_token: &str, client_id: &str, user_id: &str) -> bool {
    let resp = client
        .get("https://api.twitch.tv/helix/streams")
        .query(&[("user_id", user_id)])
        .header("Client-ID", client_id)
        .header("Authorization", format!("Bearer {}", oauth_token))
        .send()
        .await;

    if let Ok(resp) = resp {
        if let Ok(json) = resp.json::<Value>().await {
            return json["data"].as_array().map(|arr| !arr.is_empty()).unwrap_or(false);
        }
    }

    false
}