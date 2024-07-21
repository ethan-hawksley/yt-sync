use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

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
}

#[derive(Deserialize, Debug)]
struct VideoInfo {
    id: String,
    title: String,
}

fn get_config_path() -> PathBuf {
    dirs::home_dir().expect("Unable to locate home directory").join(".config/yt-sync/config.toml")
}

fn create_default_config() -> Config {
    Config {
        items: vec![
            Item {
                id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                location: "/home/user/Downloads/file_output".to_string(),
            },
            Item {
                id: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                location: "/home/user/Downloads/file_output2".to_string(),
            },
        ],
    }
}

fn write_default_config(path: &Path, config: &Config) -> io::Result<()> {
    let toml_string = toml::to_string(config).expect("Failed to serialize default config");
    fs::create_dir_all(path.parent().expect("Failed to find parent directory"))?;
    let mut writer = BufWriter::new(File::create(path)?);
    writer.write_all(toml_string.as_bytes())?;
    println!("Created default config at {:?}", path);
    Ok(())
}

fn read_config(path: &Path) -> io::Result<Config> {
    let mut content = String::new();
    BufReader::new(File::open(path)?).read_to_string(&mut content)?;
    println!("Loaded config at {:?}", path);
    Ok(toml::from_str(&content).expect("Failed to parse config"))
}

fn get_video_ids(playlist_id: &str) -> Result<(Vec<String>, Vec<String>), Box<dyn std::error::Error>> {
    let output = Command::new("yt-dlp")
        .args(&["-j", "--flat-playlist", &format!("https://www.youtube.com/playlist?list={}", playlist_id)])
        .output()?;

    if !output.status.success() {
        return Err(format!("yt-dlp failed with status: {}", output.status).into());
    }

    let stdout = String::from_utf8(output.stdout)?;
    let (mut video_ids, mut video_titles) = (Vec::new(), Vec::new());

    for line in stdout.lines() {
        let video_info: VideoInfo = serde_json::from_str(line)?;
        video_titles.push(sanitize_filename(&video_info.title));
        video_ids.push(video_info.id);
    }

    Ok((video_ids, video_titles))
}

fn download_video(video_id: &str, path: &str) -> bool {
    match Command::new("yt-dlp")
        .args(&["-P", path, "-x", "--format", "bestaudio", "--embed-thumbnail", "-q", video_id])
        .output()
    {
        Ok(output) if output.status.success() => true,
        Ok(output) => {
            println!("yt-dlp failed with output: {:?}", output);
            false
        }
        Err(e) => {
            println!("Failed to execute yt-dlp: {:?}", e);
            false
        }
    }
}

fn sanitize_filename(filename: &str) -> String {
    filename.chars().map(|c| match c {
        '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '？' => '_',
        _ => c,
    }).collect()
}

fn sync_playlist(id: &str, location: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Downloading playlist: {}", id);
    fs::create_dir_all(location)?;

    let (video_ids, video_titles) = get_video_ids(id)?;
    println!("Playlist contains: {:?}", video_titles);

    let downloaded_videos: HashSet<_> = fs::read_dir(location)?
        .filter_map(|entry| entry.ok().and_then(|e| e.path().file_name()?.to_str().map(sanitize_filename)))
        .collect();

    let download_count = video_ids.iter().progress().enumerate().filter(|(i, video_id)| {
        let file_name = format!("{} [{}].opus", sanitize_filename(&video_titles[*i]), video_id);
        !downloaded_videos.contains(&file_name) && download_video(video_id, location)
    }).count();

    println!("{} new songs successfully synced to {}", download_count, location);
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