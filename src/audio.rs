use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound::WavWriter;
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};
use chrono::Local;
use std::path::Path;
use tokio::sync::mpsc;

pub async fn record_audio(output_folder: String) -> Result<(), Box<dyn std::error::Error>> {
    let host = cpal::default_host();
    let device = select_microphone(&host).await
        .unwrap_or_else(|| host.default_input_device()
        .expect("No input device available"));
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
            let filename = format!("{}_audio.wav", timestamp);
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

async fn select_microphone(host: &cpal::Host) -> Option<cpal::Device> {
    let devices = match host.input_devices() {
        Ok(devices) => devices.collect::<Vec<_>>(),
        Err(e) => {
            eprintln!("Error getting input devices: {}", e);
            return None;
        }
    };
    
    if devices.is_empty() {
        println!("No input devices found.");
        return None;
    }

    println!("Available input devices:");
    for (i, device) in devices.iter().enumerate() {
        println!("{}. {}", i + 1, device.name().unwrap_or_else(|_| "Unknown".to_string()));
    }

    println!("Enter the number of the device you want to use (or press Enter for default):");
    
    let (tx, mut rx) = mpsc::channel(1);
    tokio::task::spawn_blocking(move || {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).expect("Failed to read line");
        tx.blocking_send(input).expect("Failed to send input");
    });

    let input = tokio::time::timeout(std::time::Duration::from_secs(10), rx.recv()).await;
    
    match input {
        Ok(Some(choice)) if !choice.trim().is_empty() => {
            if let Ok(index) = choice.trim().parse::<usize>() {
                if index > 0 && index <= devices.len() {
                    return Some(devices[index - 1].clone());
                }
            }
            println!("Invalid choice. Using default device.");
            None
        },
        _ => {
            println!("No selection made. Using default device.");
            None
        }
    }
}
