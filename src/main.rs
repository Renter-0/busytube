// TODO: Replace `Box<dyn std::error::Error>` with enums derived from `thiserror` crate
use busytube::{download_htmls, Metada, YoutubeVideoUrl, MAX_BYTES};
use reqwest::Client;
use scraper::Html;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::prelude::*;
use std::path::Path;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("No arguments supplied\nUsage:\nbusytube URLFILE VAULTPATH");
        return Ok(());
    }
    let file_with_urls = Path::new(&args[1]);
    let vault = Path::new(&args[2]);

    // Construct OS independend paths
    let videos = vault.join("storage").join("videos.md");
    let thumbnails = vault.join("storage").join("thumbnails");

    // This checks allows to unwrap read_to_string also it can panic on other errors
    // TODO: Account for less likely errors returned by OpenOptions
    if !file_with_urls.exists() {
        println!(
            "Filepath \"{}\" doesn't exist\nCan't read its content",
            file_with_urls.display()
        );
        return Ok(());
    }
    let contents = fs::read_to_string(file_with_urls).unwrap();

    // Collect valid Youtube Video sharing/viewing URLs
    let urls: Vec<YoutubeVideoUrl> = contents
        .lines()
        .filter_map(|line| YoutubeVideoUrl::parse(line).ok())
        .collect();

    let client = Client::new();
    let htmls = download_htmls(&client, urls, MAX_BYTES).await;
    for text in htmls {
        let html = String::from_utf8(text).unwrap();
        let meta = Metada::new(Html::parse_document(html.as_str()));
        meta.save_thumbnail(&thumbnails, client.clone())
            .await
            .unwrap();

        // Append extracted metadata to videos' file in specified format
        let mut vid = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&videos)
            .unwrap();
        if let Err(e) = writeln!(
            vid,
            "[link::[{}]({})], [duration::{}min], ![](thumbnails/{})\n\n",
            meta.title,
            meta.url.as_str(),
            meta.duration.as_secs() / 60, // Convert to minutes
            meta.img_name
        ) {
            eprintln!("Couldn't write to a file {}", e);
        }
    }

    Ok(())
}
