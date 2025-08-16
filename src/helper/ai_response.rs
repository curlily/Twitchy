pub async fn call_gemini_api(prompt: &str) -> Option<String> {
    let client = reqwest::Client::new();
    let api_key = match std::env::var("GEMINI_API_KEY") {
        Ok(k) => k,
        Err(e) => {
            eprintln!("GEMINI_API_KEY missing: {}", e);
            return None;
        }
    };

    let body = serde_json::json!({
        "contents": [
            {
                "parts": [
                    { "text": prompt }
                ]
            }
        ]
    });

    let resp = match client
        .post("https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent")
        .header("Content-Type", "application/json")
        .header("X-goog-api-key", &api_key)
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to send Gemini request: {}", e);
            return None;
        }
    };

    let json: serde_json::Value = match resp.json().await {
        Ok(j) => j,
        Err(e) => {
            eprintln!("Failed to parse Gemini response: {}", e);
            return None;
        }
    };

    // navigate the response JSON
    let text = json.get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|parts| parts.get(0))
        .and_then(|part| part.get("text"))
        .and_then(|t| t.as_str())
        .map(|s| s.to_string());

    if text.is_none() {
        eprintln!("Gemini response missing expected fields: {}", json);
    }

    text
}
