use std::collections::HashSet;
use std::env;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Parser;
use indicatif::ProgressIterator;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
struct Config {
    items: Vec<Item>,
}

#[derive(Deserialize, Serialize, Debug)]
struct Item {
    id: String,
    location: String,
    format: String,
    save_playlist: String,
}

#[derive(Deserialize, Debug)]
struct VideoInfo {
    id: String,
    title: String,
}

// Command line arguments for the program.
#[derive(Parser, Debug)]
#[command(
    name = "yt-sync",
    about = "Sync YouTube playlists to your local storage"
)]

struct Args {
    #[arg(short, long, default_value_t = get_default_config_path())]
    config: String,
    #[arg(short, long)]
    playlist_id: Option<String>,
    #[arg(short, long, default_value_t =   env::current_dir().unwrap().into_os_string().into_string().unwrap()
    )]
    location: String,
    #[arg(short, long, default_value = "audio")]
    format: String,
    #[arg(short, long, default_value = "false")]
    save_playlist: String,
    #[arg(short, long, action)]
    verbose: bool,
}

// Get the default configuration path for the program.
fn get_default_config_path() -> String {
    dirs::home_dir()
        .unwrap()
        .join(".config/yt-sync/config.toml")
        .to_str()
        .unwrap()
        .to_string()
}

// Create a default configuration for the program.
fn create_default_config() -> Config {
    Config {
        items: vec![
            Item {
                id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                location: "/home/user/Downloads/file_output".to_string(),
                format: "audio".to_string(),
                save_playlist: "true".to_string(),
            },
            Item {
                id: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                location: "/home/user/Downloads/file_output2".to_string(),
                format: "video".to_string(),
                save_playlist: "false".to_string(),
            },
        ],
    }
}

// Write the default configuration to a file.
fn write_default_config(path: &Path, config: &Config) -> io::Result<()> {
    let toml_string = toml::to_string(config).expect("Failed to serialize default config");
    fs::create_dir_all(path.parent().unwrap())?;
    BufWriter::new(File::create(path)?).write_all(toml_string.as_bytes())?;
    println!("Created default config at {:?}", path);
    Ok(())
}

// Read a configuration from a file.
fn read_config(path: &Path) -> io::Result<Config> {
    let mut content = String::new();
    BufReader::new(File::open(path)?).read_to_string(&mut content)?;
    println!("Loaded config at {:?}", path);
    Ok(toml::from_str(&content).expect("Failed to parse config"))
}

// Get the video IDs and titles from a YouTube playlist.
fn get_video_ids(
    playlist_id: &str,
) -> Result<(Vec<String>, Vec<String>), Box<dyn std::error::Error>> {
    let output = Command::new("yt-dlp")
        .args([
            "-j",
            "--flat-playlist",
            &format!("https://www.youtube.com/playlist?list={}", playlist_id),
        ])
        .output()?;

    if !output.status.success() {
        return Err(format!("yt-dlp failed with output: {:?}", output).into());
    }

    let stdout = String::from_utf8(output.stdout)?;
    let stdout_length = stdout.lines().count();
    let (mut video_ids, mut video_titles) = (
        Vec::with_capacity(stdout_length),
        Vec::with_capacity(stdout_length),
    );

    for line in stdout.lines() {
        let video_info: VideoInfo = serde_json::from_str(line)?;
        video_titles.push(sanitize_filename(&video_info.title));
        video_ids.push(video_info.id);
    }

    Ok((video_ids, video_titles))
}

// Download a video from YouTube using yt-dlp.
fn download_video(video_id: &str, path: &str, format: &str, verbose: bool) -> bool {
    // Create a list of arguments to pass to yt-dlp.
    let video_url = format!("https://www.youtube.com/watch?v={}", video_id);
    let mut args = vec![
        "-P",
        path,
        "-q",
        "--embed-thumbnail",
        "--embed-metadata",
        &*video_url,
    ];
    if format == "audio" {
        args.extend(&["-x", "--audio-format", "opus"]);
    } else {
        args.extend(&["-f", "bestvideo+bestaudio", "--merge-output-format", "mkv"]);
    }
    if verbose == true {
        args.extend(&["-vU"]);
    }

    // Run yt-dlp with the arguments and show an error message if it fails.
    match Command::new("yt-dlp").args(&args).output() {
        Ok(output) if output.status.success() || output.status.code() == Some(100) => true,
        Ok(output) => {
            println!(
                "yt-dlp failed to download {} with args: {:?}, with output: {:?}, and error code {:?}",
                video_id, args, output, output.status.code()
            );
            false
        }
        Err(e) => {
            println!("Failed to execute yt-dlp: {:?}", e);
            false
        }
    }
}

// Sanitize a filename to remove invalid characters.
fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '\\' | '|' | '*' | '“' | '”' => '＂',
            '？' | '?' => '？',
            '/' => '⧸',
            _ => c,
        })
        .collect()
}

// Sync a YouTube playlist to a local directory, ensuring no duplicates are downloaded.
fn sync_playlist(
    id: &str,
    location: &str,
    format: &str,
    save_playlist: &str,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Downloading playlist: {}", id);
    fs::create_dir_all(location)?;

    // Get the video IDs and titles from the playlist.
    let (video_ids, video_titles) = get_video_ids(id)?;
    if verbose {
        println!("Playlist contains: {:?}", video_titles);
    }

    // Get the list of already downloaded videos.
    let folder_contents: HashSet<_> = fs::read_dir(location)?
        .filter_map(|entry| {
            entry
                .ok()
                .and_then(|e| e.path().file_name()?.to_str().map(sanitize_filename))
        })
        .collect();

    if verbose {
        println!("Directory contains {:?}", folder_contents);
    }
    let mut m3u_file = None;
    if save_playlist == "true" {
        // Extract the parent directory and the child directory name.
        let location_path = Path::new(location);
        let parent_dir = location_path.parent().unwrap();
        let child_dir_name = location_path.file_name().unwrap().to_str().unwrap();

        let m3u_file_path = parent_dir.join(format!("{}.m3u", child_dir_name));
        // Try to delete old file
        let _ = fs::remove_file(&m3u_file_path).is_err();

        // Create the m3u file in the parent directory.
        m3u_file = Some(BufWriter::new(File::create(m3u_file_path)?));
    }

    // Download the videos that haven't been downloaded yet.
    let download_count = video_ids
        .iter()
        .progress()
        .enumerate()
        .filter(|(i, video_id)| {
            let file_name = if format == "audio" {
                format!(
                    "{} [{}].opus",
                    sanitize_filename(&video_titles[*i]),
                    video_id
                )
            } else {
                format!(
                    "{} [{}].mkv",
                    sanitize_filename(&video_titles[*i]),
                    video_id
                )
            };

            if folder_contents.contains(&file_name) {
                if let Some(ref mut m3u_file) = m3u_file {
                    writeln!(m3u_file, "{}/{}", location, file_name).unwrap();
                }
                false
            } else if download_video(video_id, location, format, verbose) {
                if verbose {
                    println!("Downloading \"{file_name}\"");
                }
                if let Some(ref mut m3u_file) = m3u_file {
                    writeln!(m3u_file, "{}{}", location, file_name).unwrap();
                }
                true
            } else {
                false
            }
        })
        .count();

    match download_count {
        1 => println!(
            "{} new song successfully synced to {}",
            download_count, location
        ),
        _ => println!(
            "{} new songs successfully synced to {}",
            download_count, location
        ),
    }
    Ok(())
}

// Main function to parse arguments and run the program.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let config_path = PathBuf::from(args.config);
    let config = if config_path.exists() {
        read_config(&config_path)?
    } else {
        let default_config = create_default_config();
        write_default_config(&config_path, &default_config)?;
        default_config
    };

    let verbose = args.verbose;
    if let Some(playlist_id) = args.playlist_id {
        let (location, format, save_playlist) = (args.location, args.format, args.save_playlist);
        sync_playlist(&playlist_id, &location, &format, &save_playlist, verbose)?;
    } else {
        for playlist in &config.items {
            sync_playlist(
                &playlist.id,
                &playlist.location,
                &playlist.format,
                &playlist.save_playlist,
                verbose,
            )?;
        }
    }

    Ok(())
}
