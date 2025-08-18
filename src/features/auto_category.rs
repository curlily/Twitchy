use std::env;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use windows::Win32::Foundation::{HWND, MAX_PATH};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
use windows::Win32::System::ProcessStatus::K32GetModuleFileNameExW;
use std::path::Path;
use std::sync::mpsc::Sender;
use std::time::Duration;
use reqwest::Client;
use tokio::runtime::Runtime;
use crate::CONFIG;
use crate::features::Feature;
use crate::twitch;

/// AutoCategory feature
pub struct AutoCategory {
    enabled: Arc<Mutex<bool>>,
    task: Option<JoinHandle<()>>,
    logger: Option<Sender<String>>,
}

impl AutoCategory {
    /// Create a new AutoCategory feature with initially enabled state
    pub fn new(initial_state: bool) -> Self {
        Self {
            enabled: Arc::new(Mutex::new(initial_state)),
            task: None,
            logger: None,
        }
    }

    /// Internal logic running in background thread
    fn run_logic(enabled: Arc<Mutex<bool>>, logger: Option<Sender<String>>) {

        let own_user_id = &CONFIG.own_user_id;
        let rt = Runtime::new().unwrap();
        let client = Client::new();
        let mut last_active_window = String::new();
        let oauth_token = env::var("TWITCH_API_OAUTH_TOKEN")
            .expect("TWITCH_API_OAUTH_TOKEN missing in .env");
        let twitch_client_id = env::var("TWITCH_API_CLIENT_ID")
            .expect("TWITCH_API_CLIENT_ID missing in .env");
        let mut live_before: bool = true;

        loop {

            if !*enabled.lock().unwrap() {
                thread::sleep(Duration::from_secs(1));
                continue;
            }

            // Check if the channel is live
            let live = rt.block_on(twitch::is_channel_live(&client, &oauth_token, &twitch_client_id, &own_user_id));

            if !live {

                if live != live_before {
                    if let Some(log_tx) = &logger {
                        log_tx.send("[Auto Category] You're offline - waiting for ya~".to_string()).unwrap();
                    }
                }

                live_before = live;

                continue;
            }

            let active_window = get_active_executable_name().unwrap_or("Just Chatting".to_string());

            if active_window != last_active_window {

                last_active_window = active_window.clone();

                // Only update your own stream
                let stream_update_result = rt.block_on(twitch::update_stream_category(&client, &oauth_token, &twitch_client_id, &own_user_id, &active_window));
                if let Some(log_tx) = &logger {
                    log_tx.send(stream_update_result).unwrap();
                }
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
        let logger_clone = self.logger.clone();
        let handle = thread::spawn(move || Self::run_logic(enabled_clone, logger_clone));
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

    fn set_logger(&mut self, logger: Sender<String>) {
        self.logger = Some(logger);
    }
}

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