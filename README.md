# yt-sync
A CLI tool to sync your YouTube playlists to a local directory.

## Project Description

`yt-sync` allows you to download your YouTube or YouTube Music playlists into a local directory. It is simple to use and supports both audio and video formats. Additionally, it can save .m3u playlists of the videos to keep track.

Configuration is stored at `~/.config/yt-sync/config.json`, and an example configuration file is automatically generated.

To install, ensure you have Rust and Cargo installed, and then run `cargo install --git https://github.com/ethan-hawksley/yt-sync`.

To run, simply run `yt-sync` in the terminal.

The layout of the configuration file is as follows:
```toml
[[items]]
id = "id_of_the_playlist"
location = "path_to_save_the_playlist"
format = "audio" # or "video", to specify the format of the downloaded videos.
save_playlist = "false" # or true, to save the playlist as a .m3u file in the parent directory.
```

This can be repeated for as many playlists as you want to sync.

Alternatively, you can run `yt-sync --help` to see the available options, and use it without the configuration file.

License: MIT