use rfd::FileDialog;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Stdio};
/// Helper function to securely read a number from the terminal
/// - Prompt is the number entered by the user thru keyboard
/// - Default number for videos is set to half of the size of the selected video
/// - Default for the choice of processor is set to the CPU fallback option
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

/// Uses ffprobe to extract the exact length of the video in seconds
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
///cuts the name out of the file, ready for pasting.
fn get_file_name(file_path: &str) -> String {
    Path::new(file_path)
        .file_stem() // Extracts the name without the extension
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_string() // Converts the slice into an owned String
}
//make a new function to get the current size of the video...
fn get_file_size(file_path: &str) -> Result<f64, String> {
    let metadata =
        fs::metadata(file_path).map_err(|e| format!("Failed to read file metadata: {}", e))?;

    let size_in_bytes = metadata.len();
    let size_in_mb = size_in_bytes as f64 / 1_048_576.0;

    Ok(size_in_mb)
}
fn main() {
    //first, select a GPU choice for the rest of the program.
    //
    println!("This program will take any video you choose to compress.");
    let mut total_compressed: f64 = 0.0;
    let gpu_choice = prompt_for_number(
        "Select your GPU Architecture: \n[1] CPU (Universal) \n[2] NVIDIA \n[3] AMD \n[4] Intel \n[5] Apple Mac\nOR Press ENTER to use CPU",
        1.0,
    ) as u32;
    //let mut loops = 0;
    //--------------------------------------------------------------------------------------
    loop {
        println!("\nSelect a video to compress...");

        //Pick the input video
        let input_path = match FileDialog::new()
            .add_filter("Videos", &["mp4", "avi", "mov", "webm", "mkv", "wmv"])
            .pick_file()
        {
            Some(path) => path,
            None => {
                println!("ERROR : No file selected.\nTry again.");
                continue;
            }
        };
        //the path to the video
        let input_str = input_path.to_string_lossy().to_string();
        println!("Selected: {}", input_str);

        // Get the duration of the video
        let duration = match get_video_duration(&input_str) {
            Ok(d) => d,
            Err(e) => {
                eprintln!(
                    "ERROR: Error reading video metadata: {}. Is FFmpeg installed?",
                    e
                );
                return;
            }
        };
        //make it pretty.
        println!(
            "------------------------------------------------
        VIDEO DETAILS: "
        );
        println!("Video Duration Detected: {:.2} seconds", duration);

        let file_size_mb = match get_file_size(&input_str) {
            Ok(size) => size,
            Err(e) => {
                eprintln!("Error: {}", e);
                return;
            }
        };
        println!("Original Video Size: {:.2} MB", file_size_mb);

        let mut filename = String::from(get_file_name(&input_str));
        println!("Original Video Name: {filename}");

        //now separate the details to make it seem pretty.
        println!("--------------------------------------------------------------");
        println!("!!TIP!!: Do not go lower than 1 bit for audio, use 1k at lowest");
        let audio_bitraw = prompt_for_number("Enter Target audio quality in kb", 128.0);
        if audio_bitraw <= 0.001 {
            println!("FFMPEG WILL NOT WORK! Try again.");
            continue;
        }
        // Prompt for target size
        let default_compression = (file_size_mb / 2.0).round();

        //let audio_bitrate = 128_000.0;
        let audio_bitrate = audio_bitraw * 1000.0;
        let min_bitrate = 1_000.1 + audio_bitrate;
        let output_bitrateaudio = format!("{:.0}", audio_bitrate);
        //now judge the bits
        let min_total_bits = min_bitrate * duration;
        let min_mb = min_total_bits / 8_388_608.0;
        println!(
            "The MINIMUM compression for this video is {:.12} MB",
            min_mb
        );
        let target_mb = prompt_for_number(
            "Enter Target File Size in Megabytes (MB)",
            default_compression,
        );
        if target_mb > file_size_mb {
            println!("!! You can't compress a file to be larger !!\n Retrying...");
            continue;
        }
        // Target size in bits: MB * 8,388,608
        let target_bits = target_mb * 8388608.0;

        // Total bits per second
        let total_bitrate = target_bits / duration;

        // Reserve 128 kbps (128,000 bps) for the audio track
        //let audio_bitrate = 128_000.0;
        let video_bitrate = total_bitrate - audio_bitrate;

        //to help you see whatever might have gone wrong.
        println!(
            "Target Bit Rate: {} must be HIGHER than 10_000 to work",
            video_bitrate
        );
        if video_bitrate < 1_000.0 {
            eprintln!(
                "ERROR: Target size is too small for a video of this length. Audio alone exceeds budget.
                \nTip: slightly increase target size until at least 1,000bps"
            );
            continue;
        }

        let compression_factor = file_size_mb / target_mb;
        println!("\n\n!COMPRESSION FACTOR: {:.2}!\n", compression_factor);

        //give the output a file name too.
        // this reuses the string from earlier, just now it appends _compressed to the first name.
        filename.push_str("_compressed");
        // if loops == !0 {
        //     filename.push_str("{loops}");
        // }
        // now choose where to put the video, once it WILL work.
        println!("Choose where to save the new video...");

        let output_path = match FileDialog::new()
            .set_file_name(filename)
            .add_filter("MP4 Video", &["mp4"])
            .save_file()
        {
            Some(path) => path,
            None => {
                println!("No save location selected. Try Again?");
                continue;
            }
        };

        let mut output_str = output_path.to_string_lossy().to_string();

        if !output_str.to_lowercase().ends_with(".mp4") {
            output_str.push_str(".mp4");
        }
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
            2 => ffmpeg_args.extend_from_slice(&[
                "-c:v",
                "h264_nvenc",
                "-preset",
                "p7",
                "-tune",
                "hq",
            ]),
            3 => ffmpeg_args.extend_from_slice(&["-c:v", "h264_amf", "-quality", "quality"]),
            4 => ffmpeg_args.extend_from_slice(&["-c:v", "h264_qsv", "-preset", "veryslow"]),
            5 => ffmpeg_args.extend_from_slice(&["-c:v", "h264_videotoolbox"]),
            _ => {
                // "veryslow" squeezes every possible byte out of libx264 (or swap to "libx265" for even smaller files)
                ffmpeg_args.extend_from_slice(&["-c:v", "libx264", "-preset", "veryslow"]);
            }
        }

        if video_bitrate < 15_000.0 {
            println!("Maximum Crunch Mode: 72p @ 5 FPS");
            ffmpeg_args.extend_from_slice(&["-vf", "scale=-2:72", "-r", "5"]);
        } else if video_bitrate < 80_000.0 {
            println!("Heavy Compression Mode: 144p @ 10 FPS");
            ffmpeg_args.extend_from_slice(&["-vf", "scale=-2:144", "-r", "10"]);
        } else if video_bitrate < 250_000.0 {
            println!("Medium Compression Mode: 360p @ 24 FPS");
            ffmpeg_args.extend_from_slice(&["-vf", "scale=-2:360", "-r", "24"]);
        }

        // 4. Attach video bitrate and dynamically scaled audio
        ffmpeg_args.extend_from_slice(&[
            "-b:v",
            &video_bitrate_str,
            "-maxrate",
            &maxrate_str,
            "-bufsize",
            &bufsize_str,
        ]);

        if audio_bitrate < 32_000.0 {
            // Downmix to mono + 8 kHz so AAC can actually approach ultra-low bitrates
            ffmpeg_args.extend_from_slice(&[
                "-c:a",
                "aac",
                "-ac",
                "1",
                "-ar",
                "8000",
                "-b:a",
                &output_bitrateaudio,
            ]);
        } else {
            ffmpeg_args.extend_from_slice(&["-c:a", "aac", "-b:a", &output_bitrateaudio]);
        }

        ffmpeg_args.push(&output_str);

        // 5. Execute FFmpeg by passing the complete vector slice
        let mut child = Command::new("ffmpeg")
            .args(&ffmpeg_args)
            .stdin(Stdio::null())
            .spawn()
            .expect("Failed to start FFmpeg process");

        let status = child.wait().expect("Failed to wait on FFmpeg process");
        let output_size = match get_file_size(&output_str) {
            Ok(size) => size,
            Err(e) => {
                eprintln!("Error: {}", e);
                return;
            }
        }; //now the output size is actually accurate.
        if status.success() {
            println!(
                "\nSuccess! Video compressed to approximately {:.2} MB at {}",
                output_size, output_str
            );

            total_compressed += file_size_mb - output_size;
            println!(
                "Total Data Compressed This Session:: {:.2} MB\n",
                total_compressed
            );
            // loops += 1;
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
