use tokio::time::{sleep, Duration};
use std::env;
use serde::Deserialize;
use reqwest::Client;
use chrono::Local;
use std::fs;
use std::io::Write;
use std::path::Path;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound::WavWriter;
use std::sync::{Arc, Mutex};
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

async fn capture_screenshot(output_folder: &str) -> Result<(Vec<u8>, String), Box<dyn std::error::Error>> {

    let screenshot = autopilot::bitmap::capture_screen().expect("Unable to capture screen");

    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let filename = format!("screenshot_{}.png", timestamp);
    let filepath = Path::new(output_folder).join(&filename);
    fs::create_dir_all(output_folder)?;

    screenshot.image
        .save(&filepath)
        .expect("Unable to save screenshot");

    let png_data = fs::read(&filepath)?;
    
    Ok((png_data, filename))
}

async fn send_image_to_openai(image_data: Vec<u8>) -> Result<String, Box<dyn std::error::Error>> {
    let api_endpoint = env::var("OPENAI_API_ENDPOINT")
        .unwrap_or_else(|_| "https://api.openai.com/v1/chat/completions".to_string());
    let api_key = env::var("OPENAI_API_KEY")?;
    let client = Client::new();

    let base64_image = base64::encode(&image_data);
    let payload = json!({
        "model": env::var("MODEL").unwrap_or_else(|_| "gpt-4-vision-preview".to_string()),
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "Describe this image."
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
        "max_tokens": 300
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

async fn save_description(output_folder: &str, filename: &str, description: &str) -> Result<(), Box<dyn std::error::Error>> {
    let desc_filename = format!("{}.txt", filename.trim_end_matches(".png"));
    let desc_filepath = Path::new(output_folder).join(desc_filename);
    let mut file = fs::File::create(desc_filepath)?;
    file.write_all(description.as_bytes())?;
    Ok(())
}

async fn record_audio(output_folder: String) -> Result<(), Box<dyn std::error::Error>> {
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or("No input device available")?;
    let config = device.default_input_config()?;

    let config: cpal::StreamConfig = config.into();

    let output_folder = Arc::new(output_folder);
    let writer = Arc::new(Mutex::new(None));

    let output_folder_clone = Arc::clone(&output_folder);
    let writer_clone = Arc::clone(&writer);

    let err_fn = |err| eprintln!("An error occurred on the audio stream: {}", err);

    let stream = device.build_input_stream(
        &config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
            let filename = format!("audio_{}.wav", timestamp);
            let filepath = Path::new(&*output_folder_clone).join(&filename);
            let mut writer_lock = writer_clone.lock().unwrap();
            if writer_lock.is_none() {
                let spec = hound::WavSpec {
                    channels: config.channels,
                    sample_rate: config.sample_rate.0,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                };
                *writer_lock = Some(WavWriter::create(&filepath, spec).expect("Failed to create WAV writer"));
            }
            if let Some(ref mut wav_writer) = *writer_lock {
                for &sample in data.iter().take(44100) { // assuming 1 second of audio
                    let sample_i16 = (sample * i16::MAX as f32) as i16;
                    wav_writer.write_sample(sample_i16).unwrap();
                }
            }
        },
        err_fn,
        None,
    )?;

    stream.play()?;

    loop {
        sleep(Duration::from_secs(60)).await;
        let mut writer_lock = writer.lock().unwrap();
        if let Some(wav_writer) = writer_lock.take() {
            wav_writer.finalize().unwrap();
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    let interval_seconds = env::var("SCREENSHOT_INTERVAL")
        .unwrap_or_else(|_| "60".to_string())
        .parse::<u64>()?;
    let output_folder = env::var("OUTPUT_FOLDER")
        .unwrap_or_else(|_| "output".to_string());

    // Start audio recording in a separate task
    let audio_output = output_folder.clone();
    tokio::task::spawn_blocking(move || {
        if let Err(e) = tokio::runtime::Runtime::new().unwrap().block_on(record_audio(audio_output)) {
            eprintln!("Error recording audio: {}", e);
        }
    });

    loop {
        println!("Capturing screenshot...");
        match capture_screenshot(&output_folder).await {
            Ok((image_data, filename)) => {
                println!("Sending screenshot to OpenAI...");
                match send_image_to_openai(image_data).await {
                    Ok(description) => {
                        println!("Description: {}", description);
                        if let Err(e) = save_description(&output_folder, &filename, &description).await {
                            eprintln!("Error saving description: {}", e);
                        }
                    },
                    Err(e) => eprintln!("Error describing image: {}", e),
                }
            },
            Err(e) => eprintln!("Error capturing screenshot: {}", e),
        }
        sleep(Duration::from_secs(interval_seconds)).await;
    }
}
