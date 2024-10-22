use chrono::Local;
use std::fs;
use std::path::Path;
use std::io::Write;

pub async fn capture_screenshot(output_folder: &str) -> Result<(Vec<u8>, String), Box<dyn std::error::Error>> {
    let screenshot = autopilot::bitmap::capture_screen().expect("Unable to capture screen");

    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let filename = format!("{}_screenshot.png", timestamp);
    let filepath = Path::new(output_folder).join(&filename);
    fs::create_dir_all(output_folder)?;

    screenshot.image
        .save(&filepath)
        .expect("Unable to save screenshot");

    let png_data = fs::read(&filepath)?;
    
    Ok((png_data, filename))
}

pub async fn save_description(output_folder: &str, filename: &str, description: &str) -> Result<(), Box<dyn std::error::Error>> {
    let desc_filename = format!("{}.txt", filename.trim_end_matches(".png"));
    let desc_filepath = Path::new(output_folder).join(desc_filename);
    let mut file = fs::File::create(desc_filepath)?;
    file.write_all(description.as_bytes())?;
    Ok(())
}
