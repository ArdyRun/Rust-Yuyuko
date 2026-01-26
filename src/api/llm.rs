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

/// Response structures for image generation
#[derive(Debug, Clone, Deserialize)]
pub struct ImageGenResponse {
    pub candidates: Option<Vec<ImageGenCandidate>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageGenCandidate {
    pub content: ImageGenContent,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageGenContent {
    pub parts: Vec<ImageGenPart>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageGenPart {
    pub text: Option<String>,
    pub inline_data: Option<ImageInlineData>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInlineData {
    pub mime_type: String,
    pub data: String,
}

/// Result of image generation
pub struct ImageGenerationResult {
    pub image_data: Vec<u8>,
    pub mime_type: String,
    #[allow(dead_code)]
    pub text: Option<String>,
}

/// Generate an image using Gemini's image generation model
pub async fn generate_image(
    data: &Data,
    prompt: &str,
) -> anyhow::Result<ImageGenerationResult> {
    let api_key = std::env::var("GEMINI_API_KEY")?;
    
    // Using gemini-2.0-flash-preview-image-generation model
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash-preview-image-generation:generateContent?key={}",
        api_key
    );

    // Clean up the prompt
    let clean_prompt = prompt
        .to_lowercase()
        .replace("buatkan gambar", "")
        .replace("generate gambar", "")
        .replace("buat gambar", "")
        .replace("gambarkan", "")
        .replace("draw", "")
        .replace("create image", "")
        .replace("bikin gambar", "")
        .trim()
        .to_string();
    
    let full_prompt = format!(
        "Create a high-quality, detailed anime style image of: {}. Make it visually appealing, artistic, and well-composed.",
        clean_prompt
    );

    let body = json!({
        "contents": [{
            "parts": [{
                "text": full_prompt
            }]
        }],
        "generationConfig": {
            "temperature": 0.7,
            "responseModalities": ["TEXT", "IMAGE"]
        }
    });

    let res = data.http_client
        .post(&url)
        .json(&body)
        .send()
        .await?;

    if !res.status().is_success() {
        let error_text = res.text().await?;
        anyhow::bail!("Gemini Image Generation API error: {}", error_text);
    }
    
    let response: ImageGenResponse = res.json().await?;
    
    // Find image part in response
    let candidates = response.candidates
        .ok_or_else(|| anyhow::anyhow!("No candidates in image generation response"))?;
    
    let candidate = candidates.first()
        .ok_or_else(|| anyhow::anyhow!("Empty candidates array"))?;
    
    // Look for image data in parts
    let mut image_data: Option<ImageInlineData> = None;
    let mut text_response: Option<String> = None;
    
    for part in &candidate.content.parts {
        if let Some(ref inline) = part.inline_data {
            if inline.mime_type.starts_with("image/") {
                image_data = Some(inline.clone());
            }
        }
        if let Some(ref text) = part.text {
            text_response = Some(text.clone());
        }
    }
    
    let inline = image_data
        .ok_or_else(|| anyhow::anyhow!("No image data in response"))?;
    
    use base64::{Engine as _, engine::general_purpose};
    let decoded = general_purpose::STANDARD.decode(&inline.data)?;
    
    Ok(ImageGenerationResult {
        image_data: decoded,
        mime_type: inline.mime_type,
        text: text_response,
    })
}
