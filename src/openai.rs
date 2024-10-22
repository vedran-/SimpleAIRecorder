use std::env;
use serde::Deserialize;
use reqwest::Client;
use base64;
use serde_json::json;

#[derive(Deserialize, Debug)]
struct OpenAIResponse {
    choices: Option<Vec<Choice>>,
    error: Option<OpenAIError>,
}

#[derive(Deserialize, Debug)]
struct OpenAIError {
    message: String,
    #[serde(rename = "type")]
    error_type: String,
}

#[derive(Deserialize, Debug)]
struct Choice {
    message: Message,
}

#[derive(Deserialize, Debug)]
struct Message {
    content: String,
}

pub async fn send_image_to_openai(image_data: Vec<u8>) -> Result<String, Box<dyn std::error::Error>> {
    let api_endpoint = env::var("OPENAI_API_ENDPOINT")
        .unwrap_or_else(|_| "https://api.openai.com/v1/chat/completions".to_string());
    let api_key = env::var("OPENAI_API_KEY")?;
    let client = Client::new();

    let base64_image = base64::encode(&image_data);
    let custom_prompt = get_ai_vision_prompt();
    let payload = json!({
        "model": env::var("MODEL").unwrap_or_else(|_| "gpt-4-vision-preview".to_string()),
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": custom_prompt
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:image/png;base64,{}", base64_image)
                        }
                    }
                ]
            }
        ],
        "max_tokens": 1024
    });

    let response = client
        .post(&api_endpoint)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        return Err(format!("OpenAI API error: {}", error_text).into());
    }

    let response_body = response.text().await?;
    println!("OpenAI response: {}", response_body); // Debug print

    let parsed_response: OpenAIResponse = serde_json::from_str(&response_body)?;

    if let Some(error) = parsed_response.error {
        return Err(format!("OpenAI API error: {} ({})", error.message, error.error_type).into());
    }

    parsed_response.choices
        .and_then(|choices| choices.first().map(|choice| choice.message.content.clone()))
        .ok_or_else(|| "No description found in OpenAI response".into())
}

fn get_ai_vision_prompt() -> String {
    env::var("AI_VISION_PROMPT")
        .unwrap_or_else(|_| "Describe this image of user screen, and try to describe what the user is doing.".to_string())
}
