use serde::{Deserialize, Serialize};
use serde_json::json;
use crate::Data;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterResponse {
    pub choices: Vec<OpenRouterChoice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterChoice {
    pub message: ChatMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiResponse {
    pub candidates: Option<Vec<GeminiCandidate>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiCandidate {
    pub content: GeminiContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiContent {
    pub parts: Vec<GeminiPart>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiPart {
    pub text: Option<String>,
}

/// Send a chat completion request to OpenRouter (Ayumi's brain)
pub async fn completion_openrouter(
    data: &Data,
    system_prompt: &str,
    messages: Vec<ChatMessage>,
) -> anyhow::Result<String> {
    let api_key = std::env::var("OPENROUTER_API_KEY")?;
    // Legacy implementation used xiaomi/mimo-v2-flash:free
    let model = "xiaomi/mimo-v2-flash:free"; 
    
    let mut all_messages = vec![ChatMessage {
        role: "system".to_string(),
        content: system_prompt.to_string(),
    }];
    all_messages.extend(messages);

    let body = json!({
        "model": model,
        "messages": all_messages,
        "max_tokens": 2048, 
        "temperature": 0.5, // Adjusted to match typical chatbot settings
    });

    // Note: OpenRouter API URL
    let res = data.http_client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("HTTP-Referer", "https://discord.com") // Required by OpenRouter
        .header("X-Title", "Yuyuko Bot")
        .json(&body)
        .send()
        .await?;
        
    if !res.status().is_success() {
        let error_text = res.text().await?;
        anyhow::bail!("OpenRouter API error: {}", error_text);
    }

    let response: OpenRouterResponse = res.json().await?;
    
    response.choices.first()
        .map(|c| c.message.content.clone())
        .ok_or_else(|| anyhow::anyhow!("No choices in OpenRouter response"))
}

/// Send a request to Gemini for multimodal tasks (Translate, etc.) (Placeholder for now)
pub async fn completion_gemini(
    data: &Data,
    prompt: &str,
) -> anyhow::Result<String> {
    let api_key = std::env::var("GEMINI_API_KEY")?;
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={}",
        api_key
    );

    let body = json!({
        "contents": [{
            "parts": [{
                "text": prompt
            }]
        }]
    });

    let res = data.http_client
        .post(&url)
        .json(&body)
        .send()
        .await?;

    if !res.status().is_success() {
        let error_text = res.text().await?;
        anyhow::bail!("Gemini API error: {}", error_text);
    }
    
    let response: GeminiResponse = res.json().await?;
    
    response.candidates.as_ref()
        .and_then(|c| c.first())
        .and_then(|c| c.content.parts.first())
        .and_then(|p| p.text.clone())
        .ok_or_else(|| anyhow::anyhow!("No text in Gemini response"))
}

/// Send a multimodal request (Image + Text) to Gemini
pub async fn completion_gemini_vision(
    data: &Data,
    prompt: &str,
    image_data: &[u8],
    mime_type: &str,
) -> anyhow::Result<String> {
    let api_key = std::env::var("GEMINI_API_KEY")?;
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={}",
        api_key
    );

    use base64::{Engine as _, engine::general_purpose};
    let base64_image = general_purpose::STANDARD.encode(image_data);

    let body = json!({
        "contents": [{
            "parts": [
                { "text": prompt },
                {
                    "inline_data": {
                        "mime_type": mime_type,
                        "data": base64_image
                    }
                }
            ]
        }]
    });

    let res = data.http_client
        .post(&url)
        .json(&body)
        .send()
        .await?;

    if !res.status().is_success() {
        let error_text = res.text().await?;
        anyhow::bail!("Gemini Vision API error: {}", error_text);
    }
    
    let response: GeminiResponse = res.json().await?;
    
    response.candidates.as_ref()
        .and_then(|c| c.first())
        .and_then(|c| c.content.parts.first())
        .and_then(|p| p.text.clone())
        .ok_or_else(|| anyhow::anyhow!("No text in Gemini Vision response"))
}
