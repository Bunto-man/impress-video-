use rfd::FileDialog;
use std::io::{self, Write};
use std::process::{Command, Stdio};
// Helper function to securely read a number from the terminal
fn prompt_for_number(prompt: &str, default: f64) -> f64 {
    loop {
        let mut input = String::new();
        print!("{} [default {}]: ", prompt, default);
        io::stdout().flush().unwrap();

        io::stdin().read_line(&mut input).unwrap();
        let trimmed = input.trim();

        // If the user just presses Enter, accept the default
        if trimmed.is_empty() {
            return default;
        }

        // Attempt to parse the string into an f64 (decimal number)
        match trimmed.parse::<f64>() {
            Ok(valid_number) => {
                // If it is a valid number, break the loop and return it
                return valid_number;
            }
            Err(_) => {
                // If they typed text or symbols, print an error and restart the loop
                println!("ERROR: Invalid input. Please type a valid number.");
                continue;
            }
        }
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
    //fist, select a GPU choice for the rest of the program.
    let gpu_choice = prompt_for_number(
        "Select your GPU: \n[1] CPU (Universal) \n[2] NVIDIA \n[3] AMD \n[4] Intel \n[5] Apple Mac\nPress ENTER to use CPU",
        1.0,
    ) as u32;

    // 2. Map the choice to the correct FFmpeg encoder
    // let encoder = match gpu_choice {
    //     2 => "h264_nvenc",
    //     3 => "h264_amf",
    //     4 => "h264_qsv",
    //     5 => "h264_videotoolbox",
    //     _ => "libx264", // Safe CPU fallback
    // };
    loop {
        println!("Step 1: Select a video to compress...");

        // 1. Pick the input video
        let input_path = match FileDialog::new()
            .add_filter("Videos", &["mp4", "avi", "mov"])
            .pick_file()
        {
            Some(path) => path,
            None => {
                println!("ERROR : No file selected. Exiting...");
                return;
            }
        };

        let input_str = input_path.to_string_lossy().to_string();
        println!("Selected: {}", input_str);

        // 2. Get the duration
        let duration = match get_video_duration(&input_str) {
            Ok(d) => d,
            Err(e) => {
                eprintln!(
                    "ERROR: Error reading video metadata: {}. Is FFmpeg installed?",
                    e
                );
                continue;
            }
        };
        println!("Video duration detected: {:.2} seconds", duration);

        // 3. Prompt for target size
        let target_mb = prompt_for_number("Enter target file size in Megabytes (MB)", 25.0);

        // Target size in bits: MB * 8,388,608
        let target_bits = target_mb * 8388608.0;

        // Total bits per second
        let total_bitrate = target_bits / duration;

        // Reserve 128 kbps (128,000 bps) for the audio track
        let audio_bitrate = 128_000.0;
        let video_bitrate = total_bitrate - audio_bitrate;
        //to help you see whatever might have gone wrong.
        println!(
            "Target Bit Rate: {} must be lower than 10_000",
            video_bitrate
        );
        if video_bitrate <= 10_000.0 {
            eprintln!(
                "ERROR: Target size is too small for a video of this length. Audio alone exceeds budget."
            );
            continue;
        }
        // now choose where to put the video, once it WILL work.
        println!("Choose where to save the new video...");

        let output_path = match FileDialog::new()
            .set_file_name("compressed_video.mp4")
            .add_filter("MP4 Video", &["mp4"])
            .save_file()
        {
            Some(path) => path,
            None => {
                println!("No save location selected. Try Again?");
                continue;
            }
        };

        let output_str = output_path.to_string_lossy().to_string();
        //Now process the video crazy style.
        //
        println!("Processing...");
        println!("Target Video Bitrate: {:.0} bps", video_bitrate);
        println!("Target Audio Bitrate: {:.0} bps", audio_bitrate);

        // 1. Format the numbers into Strings first so they live long enough for the vector
        let video_bitrate_str = format!("{:.0}", video_bitrate);
        let maxrate_str = format!("{:.0}", video_bitrate * 1.5);
        let bufsize_str = format!("{:.0}", video_bitrate * 2.0);

        // 2. Start building the universal arguments
        let mut ffmpeg_args = vec![
            "-y", // Overwrite output automatically
            "-i", &input_str,
        ];

        // 3. Apply the specific encoder and its unique proprietary flags
        match gpu_choice {
            2 => {
                // NVIDIA
                ffmpeg_args.extend_from_slice(&[
                    "-c:v",
                    "h264_nvenc",
                    "-preset",
                    "p6",
                    "-tune",
                    "hq",
                ]);
            }
            3 => {
                // AMD (Handles the Linux vs Windows mismatch)
                if cfg!(target_os = "windows") {
                    ffmpeg_args.extend_from_slice(&["-c:v", "h264_amf", "-quality", "quality"]);
                } else {
                    println!("Notice: AMD AMF requires Windows. Defaulting to CPU encoding...");
                    ffmpeg_args.extend_from_slice(&["-c:v", "libx264", "-preset", "fast"]);
                }
            }
            4 => {
                // Intel QuickSync
                ffmpeg_args.extend_from_slice(&["-c:v", "h264_qsv", "-preset", "slower"]);
            }
            5 => {
                // Apple Mac
                ffmpeg_args.extend_from_slice(&["-c:v", "h264_videotoolbox"]);
            }
            _ => {
                // Universal CPU fallback
                ffmpeg_args.extend_from_slice(&["-c:v", "libx264", "-preset", "fast"]);
            }
        }

        // 4. Attach the universal bitrate and audio formatting to the end
        ffmpeg_args.extend_from_slice(&[
            "-b:v",
            &video_bitrate_str,
            "-maxrate",
            &maxrate_str,
            "-bufsize",
            &bufsize_str,
            "-c:a",
            "aac",
            "-b:a",
            "128k",
            &output_str,
        ]);

        // 5. Execute FFmpeg by passing the complete vector slice
        let mut child = Command::new("ffmpeg")
            .args(&ffmpeg_args)
            .stdin(Stdio::null())
            .spawn()
            .expect("Failed to start FFmpeg process");

        let status = child.wait().expect("Failed to wait on FFmpeg process");

        if status.success() {
            println!(
                "Success! Video compressed to approximately {:.2} MB at {}",
                target_mb, output_str
            );
        } else {
            eprintln!("ERROR: FFmpeg encountered an (unknown?) error during compression.");
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
