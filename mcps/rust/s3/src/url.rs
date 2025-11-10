use weil_rs::http::{HttpClient, HttpMethod};

/// Simple URL shortener using TinyURL API
pub fn shorten_url(long_url: &str) -> Result<String, String> {
    // Basic URL validation
    if !long_url.starts_with("http://") && !long_url.starts_with("https://") {
        return Err("Invalid URL format".to_string());
    }

    // TinyURL API endpoint
    let api_url = "https://tinyurl.com/api-create.php";
    
    // Create query parameters
    let query_params = vec![("url".to_string(), long_url.to_string())];

    // Make HTTP request
    let response = HttpClient::request(api_url, HttpMethod::Get)
        .query(query_params)
        .send()
        .map_err(|e| format!("HTTP error: {}", e))?;

    // Check if request was successful
    if response.status() != 200 {
        return Err(format!("TinyURL API returned status: {}", response.status()));
    }

    let shortened_url = response.text();
    
    // Validate that we got a valid shortened URL back
    if shortened_url.is_empty() || !shortened_url.starts_with("http") {
        return Err("Invalid response from TinyURL API".to_string());
    }

    Ok(shortened_url)
}
