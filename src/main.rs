use rfd::FileDialog;
use std::io::{self, Write};
use std::process::{Command, Stdio};
// Helper function to read a number from the terminal
fn prompt_for_number(prompt: &str, default: f64) -> f64 {
    let mut input = String::new();
    print!("{} [default {}]: ", prompt, default);
    io::stdout().flush().unwrap();

    io::stdin().read_line(&mut input).unwrap();
    let trimmed = input.trim();

    if trimmed.is_empty() {
        default
    } else {
        trimmed.parse().unwrap_or(default)
    }
}

// Uses ffprobe to extract the exact length of the video in seconds
fn get_video_duration(file_path: &str) -> Result<f64, String> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            file_path,
        ])
        .output()
        .map_err(|e| format!("Failed to execute ffprobe: {}", e))?;

    let duration_str = String::from_utf8_lossy(&output.stdout);
    duration_str
        .trim()
        .parse::<f64>()
        .map_err(|_| "Could not parse video duration.".to_string())
}

fn main() {
    loop {
        println!("Step 1: Select a video to compress...");

        // 1. Pick the input video
        let input_path = match FileDialog::new()
            .add_filter("Videos", &["mp4", "avi", "mov"])
            .pick_file()
        {
            Some(path) => path,
            None => {
                println!("No file selected. Exiting.");
                return;
            }
        };

        let input_str = input_path.to_string_lossy().to_string();
        println!("Selected: {}", input_str);

        // 2. Get the duration
        let duration = match get_video_duration(&input_str) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Error reading video metadata: {}. Is FFmpeg installed?", e);
                return;
            }
        };
        println!("Video duration detected: {:.2} seconds", duration);

        // 3. Prompt for target size
        let target_mb = prompt_for_number("Enter target file size in Megabytes (MB)", 25.0);

        println!("Step 2: Choose where to save the new video...");

        // 4. Choose output location (Force MP4 extension)
        let output_path = match FileDialog::new()
            .set_file_name("compressed_video.mp4")
            .add_filter("MP4 Video", &["mp4"])
            .save_file()
        {
            Some(path) => path,
            None => {
                println!("No save location selected. Exiting.");
                return;
            }
        };

        let output_str = output_path.to_string_lossy().to_string();

        // 5. Calculate necessary bitrates
        // Target size in bits: MB * 8,388,608
        let target_bits = target_mb * 8388608.0;

        // Total bits per second
        let total_bitrate = target_bits / duration;

        // Reserve 128 kbps (128,000 bps) for the audio track
        let audio_bitrate = 128_000.0;
        let video_bitrate = total_bitrate - audio_bitrate;

        if video_bitrate <= 10_000.0 {
            eprintln!(
                "Target size is too small for a video of this length. Audio alone exceeds budget."
            );
            return;
        }

        println!("Processing...");
        println!("Target Video Bitrate: {:.0} bps", video_bitrate);
        println!("Target Audio Bitrate: {:.0} bps", audio_bitrate);

        // 6. Execute FFmpeg
        let mut child = Command::new("ffmpeg")
            .args([
                "-y", // Overwrite output automatically
                "-i",
                &input_str,
                "-c:v",
                "libx264", // Standard, highly compatible video codec
                "-b:v",
                &format!("{:.0}", video_bitrate),
                "-maxrate",
                &format!("{:.0}", video_bitrate * 1.5), // Prevent spikes
                "-bufsize",
                &format!("{:.0}", video_bitrate * 2.0),
                "-c:a",
                "aac", // Standard audio codec
                "-b:a",
                "128k",
                &output_str,
            ])
            .stdin(Stdio::null())
            .spawn()
            .expect("Failed to start FFmpeg process");

        let status = child.wait().expect("Failed to wait on FFmpeg process");

        if status.success() {
            println!(
                "✅ Success! Video compressed to approximately {:.2} MB at {}",
                target_mb, output_str
            );
        } else {
            eprintln!("❌ FFmpeg encountered an error during compression.");
        }
        println!("type Y to continue");
        let mut choice = String::new();
        io::stdin()
            .read_line(&mut choice)
            .expect("Failed to read...");
        //further interact with the choice, it must be a char
        // This is cool because it grabs the vary first char of the string, so the string Yes and yes also work
        // so the total yes answers are y, Y, yes, and Yes
        // Let's be honest, I copied this portion straight from my last little tool.
        let choice: char = match choice.trim().parse() {
            Ok(char) => char,
            Err(_) => continue,
        };
        match choice {
            'Y' => continue,
            'y' => continue,
            _ => {
                println!("Will not continue || Exiting...");
                return;
            }
        }
    }
}
