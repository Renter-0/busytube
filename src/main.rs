// TODO: Replace `Box<dyn std::error::Error>` with enums derived from `thiserror` crate
use busytube::{MetadataBuilder, MAX_BYTES, OFFSET_CHUNKS_COUNT};
use clap::Parser;
use futures::{stream, StreamExt};
use reqwest::{Client, Url};
use std::fs::{self, OpenOptions};
use std::io::prelude::*;
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about="YouTube scrapper to get video's title, duration and thumbnail", long_about=None)]
struct Cli {
    url_file: PathBuf,
    output_dir: PathBuf,
}
#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = Cli::parse();

    let url_file = args.url_file;
    let output_dir = args.output_dir;

    // Construct OS independend paths
    let videos = output_dir.join("videos.md");
    let thumbnails = output_dir.join("thumbnails");

    // This checks allows to unwrap read_to_string also it can panic on other errors
    // TODO: Account for less likely errors returned by OpenOptions
    if !url_file.exists() {
        println!(
            "Filepath \"{}\" doesn't exist\nCan't read its content",
            url_file.display()
        );
        return Ok(());
    }
    let contents = fs::read_to_string(url_file).unwrap();

    // Collect valid Youtube Video sharing/viewing URLs
    let urls: Vec<Url> = contents
        .lines()
        .filter_map(|line| Url::parse(line).ok())
        .collect();

    let client = Client::new();
    let metadata_builder = MetadataBuilder::new(&client, MAX_BYTES, OFFSET_CHUNKS_COUNT);
    let concurrent_downloads = urls.len().max(1);
    let mut metadata = stream::iter(urls)
        .map(|url| metadata_builder.build(url))
        .buffer_unordered(concurrent_downloads);

    while let Some(metadata) = metadata.next().await {
        let meta = metadata.unwrap();
        meta.save_thumbnail(&thumbnails, &client).await.unwrap();

        // Append extracted metadata to videos' file in specified format
        let mut vid = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&videos)
            .unwrap();
        if let Err(e) = writeln!(
            vid,
            "[link::[{}](https://youtu.be/{})], [duration::{}min], ![](thumbnails/{})\n\n",
            meta.title,
            meta.id.as_str(),
            std::time::Duration::from_millis(meta.duration.as_u64()).as_secs() / 60, // Convert to minutes
            meta.img_name
        ) {
            eprintln!("Couldn't write to a file {e}");
        }
    }

    Ok(())
}
