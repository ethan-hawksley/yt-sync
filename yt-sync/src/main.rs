use indicatif::ProgressIterator;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

#[derive(Deserialize, Serialize, Debug)]
struct Config {
    items: Vec<Item>,
}

#[derive(Deserialize, Serialize, Debug)]
struct Item {
    id: String,
    location: String,
}

#[derive(Deserialize, Debug)]
struct VideoInfo {
    id: String,
}

fn get_config_path() -> PathBuf {
    let homedir = dirs::home_dir().expect("Unable to locate home directory");
    homedir.join(".config/yt-sync/config.toml")
}

fn create_default_config() -> Config {
    let items = vec![
        Item {
            id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            location: "/home/user/Downloads/file_output".to_string(),
        },
        Item {
            id: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            location: "/home/user/Downloads/file_output2".to_string(),
        },
    ];

    Config { items }
}

fn write_default_config(path: &PathBuf, config: &Config) -> io::Result<()> {
    let toml_string = toml::to_string(config).expect("Failed to serialize default config");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::File::create(path)?;
    file.write_all(toml_string.as_bytes())?;
    println!("Created default config at {:?}", path);
    Ok(())
}

fn read_config(path: &PathBuf) -> io::Result<Config> {
    let content = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content).expect("Failed to parse config");
    println!("Loaded config at {:?}", path);
    Ok(config)
}
fn get_video_ids(playlist_id: &String) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let url = String::from("https://www.youtube.com/playlist?list=") + playlist_id;
    let output = Command::new("yt-dlp")
        .arg("-j") // Output info in JSON format
        .arg("--flat-playlist")
        .arg(url)
        .output()?;

    if !output.status.success() {
        return Err(format!("yt-dlp failed with status: {}", output.status).into());
    }

    let stdout = String::from_utf8(output.stdout)?;
    let mut video_ids = Vec::new();

    for line in stdout.lines() {
        let video_info: VideoInfo = serde_json::from_str(line)?;
        video_ids.push(video_info.id);
    }

    Ok(video_ids)
}

fn download_video(video_id: &String, path: &String) -> bool {
    let destination = String::from("-P ") + path;
    let output = Command::new("yt-dlp")
        .arg(destination)
        .arg("-x")
        .arg("--format")
        .arg("bestaudio")
        .arg("--embed-thumbnail")
        .arg("-q")
        .arg(video_id)
        .output();

    if !format!("{:?}", output).starts_with("Ok") {
        println!("{:?}", output);
        return false;
    }
    true
}

fn sync_playlist(id: &String, location: &String) -> Result<(), Box<dyn std::error::Error>> {
    println!("Downloading playlist: {}", id);
    let path = PathBuf::from(location);

    if !path.exists() {
        fs::create_dir_all(&path)?;
    }

    let video_ids = get_video_ids(id)?;
    println!("Playlist contains: {:?}", video_ids);

    let downloaded_videos: HashSet<String> = fs::read_dir(&path)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();

    let mut download_count = 0;

    for video in video_ids.into_iter().progress() {
        let mut exists = false;
        for downloaded_video in &downloaded_videos {
            if downloaded_video.contains(&video) && downloaded_video.contains("opus") {
                exists = true;
                break;
            }
        }
        if !exists && download_video(&video, location) {
            download_count += 1;
        }
    }
    println!(
        "{} new songs successfully synced to {}",
        download_count, location
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_path = get_config_path();
    let config = if config_path.exists() {
        read_config(&config_path)?
    } else {
        let default_config = create_default_config();
        write_default_config(&config_path, &default_config)?;
        default_config
    };

    for playlist in &config.items {
        sync_playlist(&playlist.id, &playlist.location)?;
    }
    Ok(())
}
