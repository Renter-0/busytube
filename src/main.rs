// TODO: Replace `Box<dyn std::error::Error>` with enums derived from `thiserror` crate
use busytube::{MetadataBuilder, MAX_BYTES, OFFSET_CHUNKS_COUNT};
use futures::{stream, StreamExt};
use reqwest::{Client, Url};
use std::env;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::prelude::*;
use std::path::PathBuf;

const HELP: &str = "YouTube scrapper to get video's title, duration and thumbnail\n\nUsage: busytube <URL_FILE> <OUTPUT_DIR>\n\nArguments:\n  <URL_FILE>\n  <OUTPUT_DIR>\n\nOptions:\n  -h, --help     Print help\n  -V, --version  Print version";

#[derive(Debug, PartialEq, Eq)]
struct Cli {
    url_file: PathBuf,
    output_dir: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
enum CliAction {
    Run(Cli),
    Help,
    Version,
}

fn parse_cli_args(args: impl IntoIterator<Item = OsString>) -> Result<CliAction, ()> {
    let args: Vec<OsString> = args.into_iter().collect();

    match args.as_slice() {
        [flag] if flag == "-h" || flag == "--help" => Ok(CliAction::Help),
        [flag] if flag == "-V" || flag == "--version" => Ok(CliAction::Version),
        [url_file, output_dir]
            if !url_file.to_string_lossy().starts_with('-')
                && !output_dir.to_string_lossy().starts_with('-') =>
        {
            Ok(CliAction::Run(Cli {
                url_file: PathBuf::from(url_file),
                output_dir: PathBuf::from(output_dir),
            }))
        }
        _ => Err(()),
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = match parse_cli_args(env::args_os().skip(1)) {
        Ok(CliAction::Run(args)) => args,
        Ok(CliAction::Help) => {
            println!("{HELP}");
            return Ok(());
        }
        Ok(CliAction::Version) => {
            // Cargo resolves this macro while compiling; it is not a runtime environment variable.
            println!("busytube {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Err(()) => {
            eprintln!("Usage: busytube <URL_FILE> <OUTPUT_DIR>\nTry 'busytube --help' for more information.");
            std::process::exit(2);
        }
    };

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

#[cfg(test)]
mod tests {
    use super::{parse_cli_args, Cli, CliAction};
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn parses_two_positional_paths() {
        let action = parse_cli_args([OsString::from("urls.md"), OsString::from("output")]);

        assert_eq!(
            action,
            Ok(CliAction::Run(Cli {
                url_file: PathBuf::from("urls.md"),
                output_dir: PathBuf::from("output"),
            }))
        );
    }

    #[test]
    fn recognizes_help_and_version_flags() {
        assert_eq!(
            parse_cli_args([OsString::from("--help")]),
            Ok(CliAction::Help)
        );
        assert_eq!(
            parse_cli_args([OsString::from("-V")]),
            Ok(CliAction::Version)
        );
    }

    #[test]
    fn rejects_missing_extra_and_flag_arguments() {
        assert!(parse_cli_args([]).is_err());
        assert!(parse_cli_args([
            OsString::from("urls.md"),
            OsString::from("output"),
            OsString::from("extra"),
        ])
        .is_err());
        assert!(parse_cli_args([OsString::from("--unknown"), OsString::from("output")]).is_err());
    }
}
