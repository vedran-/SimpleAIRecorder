mod screenshot;
mod audio;
mod openai;
mod window_info;

use tokio::time::{sleep, Duration};
use std::env;
use crate::screenshot::{capture_screenshot, save_description};
use crate::audio::record_audio;
use crate::openai::send_image_to_openai;
use crate::window_info::get_active_window_info;

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
                let system_info = get_active_window_info();

                println!("Sending screenshot to OpenAI...");
                match send_image_to_openai(image_data).await {
                    Ok(description) => {
                        println!("Description: {}", description);
                        let full_description = format!("{}\n\nDescription:\n{}", system_info, description);
                        if let Err(e) = save_description(&output_folder, &filename, &full_description).await {
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
