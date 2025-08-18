use std::env;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use crate::features::Feature;

/// AutoCategory feature
pub struct AutoCategory {
    enabled: Arc<Mutex<bool>>,
    task: Option<JoinHandle<()>>,
}

impl AutoCategory {
    /// Create a new AutoCategory feature with initial enabled state
    pub fn new(initial_state: bool) -> Self {
        Self {
            enabled: Arc::new(Mutex::new(initial_state)),
            task: None,
        }
    }

    /// Internal logic running in background thread
    fn run_logic(enabled: Arc<Mutex<bool>>) {

        let own_user_id = &CONFIG.own_user_id;
        let rt = Runtime::new().unwrap();
        let client = Client::new();
        let mut current_category = String::new();
        let oauth_token = env::var("TWITCH_API_OAUTH_TOKEN")
            .expect("TWITCH_API_OAUTH_TOKEN missing in .env");
        let twitch_client_id = env::var("TWITCH_API_CLIENT_ID")
            .expect("TWITCH_API_CLIENT_ID missing in .env");

        loop {
            if !*enabled.lock().unwrap() {
                thread::sleep(Duration::from_secs(1));
                continue;
            }

            // Check if the channel is live
            let live = rt.block_on(is_channel_live(&client, &oauth_token, &twitch_client_id, &own_user_id));
            if !live {
                //println!("Channel is offline, skipping auto-category.");
                thread::sleep(Duration::from_secs(60));
                continue;
            }

            let category = get_active_executable_name().unwrap_or("Just Chatting".to_string());

            if category != current_category {
                //println!("AutoCategory: changing category to '{}'", category);
                current_category = category.clone();

                // Only update your own stream
                rt.block_on(update_stream_category(&client, &oauth_token, &twitch_client_id, &own_user_id, &category));
            }

            thread::sleep(Duration::from_secs(1));
        }
    }
}

impl Feature for AutoCategory {
    fn name(&self) -> &str {
        "auto_category"
    }

    fn is_enabled(&self) -> bool {
        *self.enabled.lock().unwrap()
    }

    fn start(&mut self){
        if self.task.is_some() {
            return; // already running
        }
        {
            let mut enabled = self.enabled.lock().unwrap();
            *enabled = true;
        }
        let enabled_clone = Arc::clone(&self.enabled);
        let handle = std::thread::spawn(move || Self::run_logic(enabled_clone));
        self.task = Some(handle);
    }

    fn stop(&mut self) {
        let mut enabled = self.enabled.lock().unwrap();
        *enabled = false;

        // Optionally, join the thread (blocking) or just leave it to exit naturally
        if let Some(handle) = self.task.take() {
            let _ = handle.join();
        }
    }
}

use windows::Win32::Foundation::{HWND, MAX_PATH};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
use windows::Win32::System::ProcessStatus::K32GetModuleFileNameExW;
use std::path::Path;
use std::time::Duration;
use reqwest::Client;
use tokio::runtime::Runtime;

fn get_active_executable_name() -> Option<String> {
    unsafe {
        let hwnd: HWND = GetForegroundWindow();
        if hwnd.0 == std::ptr::null_mut() {
            return None;
        }

        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));

        let process_handle = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(handle) => Some(handle),
            Err(_) => None,
        };

        let mut buffer = [0u16; MAX_PATH as usize];
        let len = K32GetModuleFileNameExW(process_handle, None, &mut buffer);
        if len == 0 {
            return None;
        }

        let full_path = String::from_utf16_lossy(&buffer[..len as usize]);
        let file_name = Path::new(&full_path)
            .file_stem() // gets the name without extension
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();

        Some(file_name)
    }
}

use serde_json::{json, Value};
use crate::CONFIG;

pub async fn update_stream_category(
    client: &Client,
    oauth_token: &str,
    client_id: &str,
    broadcaster_id: &str,
    category_name: &str,
) {
    // Get the game ID from the game name
    let game_resp = client
        .get("https://api.twitch.tv/helix/games")
        .query(&[("name", category_name)])
        .header("Client-ID", client_id)
        .header("Authorization", format!("Bearer {}", oauth_token))
        .send()
        .await;

    let game_id = match game_resp {
        Ok(resp) => {
            let json: serde_json::Value = resp.json().await.unwrap_or_default();
            if let Some(id) = json["data"][0]["id"].as_str() {
                id.to_string()
            } else {
                "".to_string() // fallback if game not found
            }
        }
        Err(_) => "".to_string(),
    };

    // Update the channel's category
    let body = json!({
        "game_id": game_id,
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
            //println!("Category updated to '{}'", category_name);
        }
        Ok(resp) => {
            //println!("Failed to update category: {}", resp.status());
        }
        Err(err) => {
            //println!("Error updating category: {}", err);
        }
    }
}

async fn is_channel_live(client: &Client, oauth_token: &str, client_id: &str, user_id: &str) -> bool {
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