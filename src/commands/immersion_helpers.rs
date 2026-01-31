// Helper function to fetch page title from URL
async fn fetch_page_title(
    client: &reqwest::Client,
    url: &str,
) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    // Fetch the webpage
    let response = client
        .get(url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?;
    
    if !response.status().is_success() {
        return Ok(None);
    }
    
    let html = response.text().await?;
    
    // Simple regex to extract <title> tag content
    if let Some(start) = html.find("<title>") {
        if let Some(end) = html[start..].find("</title>") {
            let title_start = start + 7; // Length of "<title>"
            let title_end = start + end;
            let title = html[title_start..title_end].trim();
            
            // Decode HTML entities if needed (basic decoding)
            let decoded = html_escape::decode_html_entities(title).to_string();
            
            return Ok(Some(decoded));
        }
    }
    
    // Fallback: try og:title meta tag
    if let Some(og_title) = extract_meta_property(&html, "og:title") {
        return Ok(Some(og_title));
    }
    
    Ok(None)
}

// Helper to extract meta property content
fn extract_meta_property(html: &str, property: &str) -> Option<String> {
    let pattern = format!(r#"<meta property="{}" content=""#, property);
    if let Some(start) = html.find(&pattern) {
        let content_start = start + pattern.len();
        if let Some(end) = html[content_start..].find('"') {
            let content = &html[content_start..content_start + end];
            return Some(html_escape::decode_html_entities(content).to_string());
        }
    }
    None
}
