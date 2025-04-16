use futures::{stream, StreamExt};
use regex::Regex;
use reqwest::{Client, Url};
use scraper::{Html, Selector};
use std::io::prelude::*;
use std::path::Path;

pub const MAX_BYTES: usize = 70000;
pub const OFFSET_CHUNKS_COUNT: usize = 360;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct YoutubeVideoId(Box<str>);

impl YoutubeVideoId {
    pub fn parse(url: &str) -> Result<Self, &'static str> {
        // Seperating regex via macros as format! produces String and not &str
        macro_rules! DOMAIN_PATTERN {
            () => {
                r"(?:(?:www\.|m\.)?youtu(?:be\.com|\.be))"
            };
        }
        macro_rules! VIDEO_PATTERN {
            () => {
                r"(?:watch\?v=|v/|embed/)?(?<id>[a-zA-Z0-9_-]{11})"
            };
        }
        const YOUTUBE_PATTERN: &str =
            std::concat!("https://", DOMAIN_PATTERN!(), "/", VIDEO_PATTERN!());
        const ERR_MSG: &str = "Regex didn't find youtube id";

        let url_validator = Regex::new(YOUTUBE_PATTERN).unwrap();
        let id = url_validator
            .captures(url)
            .ok_or(ERR_MSG)?
            .name("id")
            .ok_or(ERR_MSG)?
            .as_str()
            .to_string()
            .into_boxed_str();

        Ok(Self { 0: id })
    }
    pub fn as_str(&self) -> &str {
        &self.0[..]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Millis(u64);

impl Millis {
    pub fn new(milliseconds: impl Into<u64>) -> Self {
        Self {
            0: milliseconds.into(),
        }
    }
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct Metada {
    pub title: Box<str>,
    pub id: YoutubeVideoId,
    pub duration: Millis,
    pub img_name: Box<str>,
}

pub async fn download_htmls(
    client: &Client,
    links: Vec<Url>,
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
                    .get(url)
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
        let title_selector = Selector::parse("meta[name='title']").unwrap();
        let title = html
            .select(&title_selector)
            .next()
            .and_then(|elem| elem.value().attr("content"))
            .ok_or("Title wasn't found")?
            .to_string()
            .into_boxed_str();

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

        let url_selector =
            Selector::parse("meta[property='og:url']").expect("Selector for URL didn't compile");

        let url = html
            .select(&url_selector)
            .next()
            .and_then(|elem| elem.value().attr("content"))
            .ok_or("Video URL wasn't found in provided HTML")?;

        let id = YoutubeVideoId::parse(url)?;

        Ok(Self {
            title,
            id,
            duration: Millis::new(dur),
            img_name: format!("{}.jpg", uuid::Uuid::new_v4()).into_boxed_str(),
        })
    }

    /// Downloads image URL extracted from youtuble link and saves it
    /// using `UUID v4` as the name and `jpg` as file extension
    pub async fn save_thumbnail(
        &self,
        thumbnail_dir: &Path,
        client: Client,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!(
            "https://i.ytimg.com/vi/{}/maxresdefault.jpg",
            self.id.as_str()
        );
        if !thumbnail_dir.exists() {
            let mut dir_builder = std::fs::DirBuilder::new();
            dir_builder.recursive(true).create(thumbnail_dir).unwrap();
        }
        let mut file = std::fs::File::create(thumbnail_dir.join(&self.img_name[..]))?;

        let contents = client.get(url).send().await?.bytes().await?;
        file.write_all(&contents)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        download_htmls, Client, Html, Metada, Url, YoutubeVideoId, MAX_BYTES, OFFSET_CHUNKS_COUNT,
    };
    #[tokio::test]
    async fn test_download_htmls_length() {
        let client = Client::new();
        let url: Vec<Url> =
            vec![Url::parse("https://www.youtube.com/watch?v=h9Z4oGN89MU").unwrap()];
        let chunk = download_htmls(&client, url, MAX_BYTES, OFFSET_CHUNKS_COUNT).await;
        assert_eq!(MAX_BYTES, chunk[0].as_ref().unwrap().len());
    }

    #[tokio::test]
    async fn test_is_downloaded_fragment_sufficient_for_parsing() {
        let client = Client::new();
        let urls: Vec<Url> =
            vec![Url::parse("https://www.youtube.com/watch?v=h9Z4oGN89MU").unwrap()];
        let fragments = download_htmls(&client, urls, MAX_BYTES, OFFSET_CHUNKS_COUNT).await;
        let fragment = String::from_utf8(fragments[0].as_ref().unwrap().clone()).unwrap();
        let html = Html::parse_document(fragment.as_str());

        let meta = Metada::new(html).unwrap();

        assert_eq!(meta.id.as_str(), "h9Z4oGN89MU");
        assert_eq!(meta.duration.as_u64(), 1709569);
        assert_eq!(
            &meta.title[..],
            "How do Graphics Cards Work?  Exploring GPU Architecture"
        );
    }
    #[test]
    fn test_youtube_video_id_parse() {
        const ID: &str = "h9Z4oGN89MU";
        const URLS: [&str; 5] = [
            "https://youtu.be/h9Z4oGN89MU",
            "https://youtu.be/h9Z4oGN89MU?si=3lAgdXzkExZahlOO",
            "https://www.youtube.com/watch?v=h9Z4oGN89MU",
            "https://m.youtube.com/watch?v=h9Z4oGN89MU&pp=b3Jrcw%3D%39aGB33IGdwdSDygUN",
            "https://m.youtube.com/watch?v=h9Z4oGN89MU&list=WL&index=1&t=12s&pp=QgAiAQBB",
        ];
        for url in URLS {
            let parsed = YoutubeVideoId::parse(url);
            assert!(parsed.is_ok());
            assert_eq!(parsed.unwrap().as_str(), ID);
        }
    }
}
