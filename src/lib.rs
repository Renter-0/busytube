use futures::{stream, StreamExt};
use regex::Regex;
use reqwest::{Client, Url};
use scraper::{Html, Selector};
use std::io::prelude::*;
use std::path::Path;
use std::time::Duration;

pub const MAX_BYTES: usize = 70000;
pub const OFFSET_CHUNKS_COUNT: usize = 360;

#[derive(Debug, Clone)]
pub struct YoutubeVideoUrl {
    inner: Url,
}
impl YoutubeVideoUrl {
    pub fn parse(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
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
pub struct Metada {
    pub title: String,
    pub url: YoutubeVideoUrl,
    pub duration: Duration,
    pub img_url: Url,
    pub img_name: String,
}

pub async fn download_htmls(
    client: &Client,
    links: Vec<YoutubeVideoUrl>,
    max_bytes: usize,
    offset_chunk_number: usize,
) -> Vec<Result<Vec<u8>, Box<dyn std::error::Error>>> {
    // Creates multiple concurrent get requests and collects resulting HTML as the download finishes
    // afterwards
    let concurent_requests = links.len();
    stream::iter(links)
        .map(|url| {
            // Download content up to max_bytes
            async move {
                let mut byte_stream = client
                    .get(url.inner)
                    .send()
                    .await?
                    .bytes_stream()
                    .skip(offset_chunk_number);
                let mut collected_chunks = Vec::with_capacity(max_bytes);
                let mut remaining_bytes = max_bytes;

                while let Some(chunk) = byte_stream.next().await {
                    let chunk = chunk?;

                    // Prevent memory re-allocations if chunk exceeds max_bytes
                    let to_take = std::cmp::min(remaining_bytes, chunk.len());
                    collected_chunks.extend_from_slice(&chunk[..to_take]);

                    // Terminate download if max_bytes were reached
                    remaining_bytes = remaining_bytes.saturating_sub(chunk.len());
                    if remaining_bytes == 0 {
                        break;
                    }
                }
                Ok(collected_chunks)
            }
        })
        .buffer_unordered(concurent_requests)
        .collect()
        .await
}

impl Metada {
    pub fn new(html: Html) -> Result<Metada, &'static str> {
        let title_selector = Selector::parse("title").unwrap();
        let title = html
            .select(&title_selector)
            .next()
            .ok_or("Title wasn't found in provided HTML")?
            .inner_html();

        let binding = Selector::parse("body script").unwrap();
        let script_block = html.select(&binding);

        // NOTE: Regex can be stored somewhere else so it is compiled only once
        let re = Regex::new(r#""approxDurationMs":"(\d+)""#)
            .expect("Regex for approximate duration couldn't be compiled");

        let dur: u64 = script_block
            .flat_map(|elemref| elemref.text())
            .find_map(|text| re.captures(text).take())
            .ok_or("Regex didn't find approxDurationMs")?[1]
            .parse()
            .expect("Regex found non numeric value while searching for approxDurationMs");

        let dur = Duration::from_millis(dur);

        let img_src = Selector::parse("link[rel='image_src']")
            .expect("Selector for image src didn't compile");

        let img_url: Url = html
            .select(&img_src)
            .next()
            .and_then(|elem| elem.value().attr("href"))
            .ok_or("URL for img wasn't found in provided HTML")?
            .try_into()
            // TODO: Handle reqwest::ParseError
            .map_err(|_| "Couldn't convert href to Url")?;

        let url_selector =
            Selector::parse("meta[property='og:url']").expect("Selector for URL didn't compile");

        let url = html
            .select(&url_selector)
            .next()
            .and_then(|elem| elem.value().attr("content"))
            .ok_or("Video URL wasn't found in provided HTML")?
            .try_into()
            // TODO: Handle reqwest::ParseError
            .map_err(|_| "Couldn't selected href's content into Url")?;

        Ok(Self {
            title,
            url,
            duration: dur,
            img_url,
            img_name: uuid::Uuid::new_v4().to_string() + ".jpg",
        })
    }

    /// Downloads image URL extracted from youtuble link and saves it
    /// using `UUID v4` as the name and `jpg` as file extension
    pub async fn save_thumbnail(
        &self,
        thumbnail_dir: &Path,
        client: Client,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !thumbnail_dir.exists() {
            let mut dir_builder = std::fs::DirBuilder::new();
            dir_builder.recursive(true).create(thumbnail_dir).unwrap();
        }
        let mut file = std::fs::File::create(thumbnail_dir.join(self.img_name.as_str()))?;

        let contents = client
            .get(self.img_url.as_str())
            .send()
            .await?
            .bytes()
            .await?;
        file.write_all(&contents)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        download_htmls, Client, Html, Metada, YoutubeVideoUrl, MAX_BYTES, OFFSET_CHUNKS_COUNT,
    };
    #[tokio::test]
    async fn test_download_htmls_length() {
        let client = Client::new();
        let url: Vec<YoutubeVideoUrl> =
            vec![YoutubeVideoUrl::parse("https://www.youtube.com/watch?v=h9Z4oGN89MU").unwrap()];
        let chunk = download_htmls(&client, url, MAX_BYTES, OFFSET_CHUNKS_COUNT).await;
        assert_eq!(MAX_BYTES, chunk[0].as_ref().unwrap().len());
    }

    #[tokio::test]
    async fn test_is_downloaded_fragment_sufficient_for_parsing() {
        let client = Client::new();
        let urls: Vec<YoutubeVideoUrl> =
            vec![YoutubeVideoUrl::parse("https://www.youtube.com/watch?v=h9Z4oGN89MU").unwrap()];
        let fragments = download_htmls(&client, urls, MAX_BYTES, OFFSET_CHUNKS_COUNT).await;
        let fragment = String::from_utf8(fragments[0].as_ref().unwrap().clone()).unwrap();
        let html = Html::parse_document(fragment.as_str());

        let meta = Metada::new(html).unwrap();

        assert_eq!(
            meta.url.inner.as_str(),
            "https://www.youtube.com/watch?v=h9Z4oGN89MU"
        );
        assert_eq!(meta.duration.as_millis(), 1709569);
        assert_eq!(
            meta.title,
            "How do Graphics Cards Work?  Exploring GPU Architecture - YouTube"
        );
        assert_eq!(
            meta.img_url.as_str(),
            "https://i.ytimg.com/vi/h9Z4oGN89MU/maxresdefault.jpg"
        );
    }
}
