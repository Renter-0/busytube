// TODO: Replace `Box<dyn std::error::Error>` with enums derived from `thiserror` crate
use futures::{stream, StreamExt};
use regex::Regex;
use reqwest::{Client, Url};
use scraper::{Html, Selector};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::prelude::*;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone)]
struct YoutubeVideoUrl {
    inner: Url,
}
impl YoutubeVideoUrl {
    fn parse(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let inner = Url::parse(url)?;
        YoutubeVideoUrl::try_from(inner)
    }
}

impl std::ops::Deref for YoutubeVideoUrl {
    type Target = Url;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl TryFrom<Url> for YoutubeVideoUrl {
    type Error = Box<dyn std::error::Error>;
    fn try_from(value: Url) -> Result<Self, Self::Error> {
        // Currently all youtube videos start with these two
        const YOUTUBE_DOMAINS: [&str; 2] = ["www.youtube.com", "youtu.be"];
        if value
            .domain()
            .is_some_and(|domain| YOUTUBE_DOMAINS.contains(&domain))
        {
            Ok(Self { inner: value })
        } else {
            Err("Provided URL doesn't belong to Youtube's Video Sharing/Viewing".into())
        }
    }
}
impl<'a> TryFrom<&'a str> for YoutubeVideoUrl {
    type Error = Box<dyn std::error::Error>;
    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        YoutubeVideoUrl::parse(value)
    }
}
impl std::ops::DerefMut for YoutubeVideoUrl {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[derive(Debug, Clone)]
struct Metada {
    title: String,
    url: YoutubeVideoUrl,
    duration: Duration,
    img_url: Url,
    img_name: String,
}

async fn get_htmls(client: Client, links: Vec<YoutubeVideoUrl>) -> Vec<String> {
    // Creates multiple concurrent get requests and collects resulting HTML as the download finishes
    let concurent_requests = links.len();
    stream::iter(links)
        .map(|url| {
            let client = &client;
            async move {
                let response = client.get(url.inner).send().await.unwrap();
                // TODO: Rewrite to download only the first N bytes needed for parser
                response.text().await.unwrap()
            }
        })
        .buffer_unordered(concurent_requests)
        .collect::<Vec<String>>()
        .await
}

impl Metada {
    // NOTE: Most of the parts that can error here are due to not getting a valid youtube URL
    // TODO: Rewrite new as it shouldn't panic
    fn new(html: Html) -> Metada {
        let title_selector = Selector::parse("title").unwrap();
        let title = html.select(&title_selector).next().unwrap().inner_html();
        let binding = Selector::parse("body script").unwrap();
        let script_block = html.select(&binding);
        let re = Regex::new(r#""approxDurationMs":"(\d+)""#)
            .expect("Regex for approximate duration couldn't be compiled");

        let dur: Option<String> = script_block
            .flat_map(|elemref| elemref.text())
            .find_map(|text| re.captures(text).map(|cap| cap[1].into()));

        let dur = Duration::from_millis(
            dur.expect("Duration errored")
                .parse::<u64>()
                .expect("Duration couldn't be created from duration string"),
        );
        let img_src = Selector::parse("link[rel='image_src']")
            .expect("Selector for image src didn't compile");

        let img_url: Url = html
            .select(&img_src)
            .next()
            .and_then(|elem| elem.value().attr("href"))
            .expect("href wasn't found for image_src selector")
            .try_into()
            .unwrap();

        let url_selector =
            Selector::parse("meta[property='og:url']").expect("Selector for URL didn't compile");

        let url = html
            .select(&url_selector)
            .next()
            .and_then(|elem| elem.value().attr("content"))
            .expect("content attr wasn't found for url selector")
            .try_into()
            .unwrap();

        Self {
            title,
            url,
            duration: dur,
            img_url,
            img_name: uuid::Uuid::new_v4().to_string() + ".jpg",
        }
    }
    /// Downloads image URL extracted from youtuble link and saves it
    /// using `UUID v4` as the name and `jpg` as file extension
    async fn save_thumbnail(
        &self,
        thumbnail_dir: &Path,
        client: Client,
    ) -> Result<(), std::io::ErrorKind> {
        if !thumbnail_dir.exists() {
            let mut dir_builder = std::fs::DirBuilder::new();
            dir_builder.recursive(true).create(thumbnail_dir).unwrap();
        }
        if !thumbnail_dir.is_dir() {
            return Err(std::io::ErrorKind::NotADirectory);
        }
        let mut file = std::fs::File::create(thumbnail_dir.join(self.img_name.as_str())).unwrap();

        let contents = client
            .get(self.img_url.as_str())
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        file.write_all(&contents).unwrap();
        Ok(())
    }
}

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
    let htmls = get_htmls(client.clone(), urls).await;
    for text in htmls {
        let meta = Metada::new(Html::parse_document(text.as_str()));
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
