use indicatif::ProgressIterator;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
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
    title: String,
}

fn get_config_path() -> PathBuf {
    dirs::home_dir()
        .expect("Unable to locate home directory")
        .join(".config/yt-sync/config.toml")
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

fn write_default_config(path: &Path, config: &Config) -> io::Result<()> {
    let toml_string = toml::to_string(config).expect("Failed to serialize default config");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    writer.write_all(toml_string.as_bytes())?;
    println!("Created default config at {:?}", path);
    Ok(())
}

fn read_config(path: &Path) -> io::Result<Config> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut content = String::new();
    reader.read_to_string(&mut content)?;
    let config: Config = toml::from_str(&content).expect("Failed to parse config");
    println!("Loaded config at {:?}", path);
    Ok(config)
}

fn get_video_ids(
    playlist_id: &str,
) -> Result<(Vec<String>, Vec<String>), Box<dyn std::error::Error>> {
    let url = format!("https://www.youtube.com/playlist?list={}", playlist_id);
    let output = Command::new("yt-dlp")
        .arg("-j")
        .arg("--flat-playlist")
        .arg(&url)
        .output()?;

    if !output.status.success() {
        return Err(format!("yt-dlp failed with status: {}", output.status).into());
    }

    let stdout = String::from_utf8(output.stdout)?;
    let mut video_titles = Vec::new();
    let mut video_ids = Vec::new();

    for line in stdout.lines() {
        let video_info: VideoInfo = serde_json::from_str(line)?;
        video_titles.push(sanitize_filename(video_info.title.as_str()));
        video_ids.push(video_info.id);
    }

    Ok((video_ids, video_titles))
}

fn download_video(video_id: &str, path: &str) -> bool {
    let output = Command::new("yt-dlp")
        .arg("-P")
        .arg(path)
        .arg("-x")
        .arg("--format")
        .arg("bestaudio")
        .arg("--embed-thumbnail")
        .arg("-q")
        .arg(video_id)
        .output();

    match output {
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
    let filename = filename
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '？' => '_',
            _ => c,
        })
        .collect();
    filename
}

fn sync_playlist(id: &str, location: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Downloading playlist: {}", id);
    let path = Path::new(location);

    if !path.exists() {
        fs::create_dir_all(path)?;
    }

    let (video_ids, video_titles) = get_video_ids(id)?;
    println!("Playlist contains: {:?}", video_titles);

    let mut downloaded_videos = Vec::new();

    for entry in fs::read_dir(location)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(filename) = path.file_name() {
                if let Some(filename_str) = filename.to_str() {
                    downloaded_videos.push(sanitize_filename(filename_str));
                }
            }
        }
    }

    let mut download_count = 0;

    for (i, video) in video_ids.iter().progress().enumerate() {
        let video_title = sanitize_filename(video_titles[i].as_str());
        let file_name = format!("{} [{}].opus", video_title, video);
        if !downloaded_videos.contains(&file_name) && download_video(video, location) {
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
