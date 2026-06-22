use futures::StreamExt;
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

        Ok(Self(id))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0[..]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Millis(u64);

impl Millis {
    pub fn new(milliseconds: impl Into<u64>) -> Self {
        Self(milliseconds.into())
    }
    #[must_use]
    pub const fn as_u64(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct Metadata {
    pub title: Box<str>,
    pub id: YoutubeVideoId,
    pub duration: Millis,
    pub img_name: Box<str>,
}

pub struct MetadataBuilder<'a> {
    client: &'a Client,
    max_bytes: usize,
    offset_chunk_number: usize,
}

impl<'a> MetadataBuilder<'a> {
    #[must_use]
    pub const fn new(client: &'a Client, max_bytes: usize, offset_chunk_number: usize) -> Self {
        Self {
            client,
            max_bytes,
            offset_chunk_number,
        }
    }

    pub async fn build(&self, url: Url) -> Result<Metadata, Box<dyn std::error::Error>> {
        let mut byte_stream = self
            .client
            .get(url)
            .send()
            .await?
            .bytes_stream()
            .skip(self.offset_chunk_number);
        let mut contents = Vec::with_capacity(self.max_bytes);
        let mut remaining_bytes = self.max_bytes;

        while let Some(chunk) = byte_stream.next().await {
            let chunk = chunk?;
            let to_take = std::cmp::min(remaining_bytes, chunk.len());
            contents.extend_from_slice(&chunk[..to_take]);
            remaining_bytes = remaining_bytes.saturating_sub(chunk.len());
            if remaining_bytes == 0 {
                break;
            }
        }

        let html = String::from_utf8(contents)?;
        Metadata::new(Html::parse_document(&html))
            .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidData, message).into())
    }
}

impl Metadata {
    pub fn new(html: Html) -> Result<Self, &'static str> {
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
            .find_map(|text| re.captures(text))
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
        client: &Client,
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
    use super::{Html, Metadata, YoutubeVideoId};

    #[test]
    fn test_metadata_from_html() {
        let html = Html::parse_document(
            r#"
                <meta name="title" content="How do Graphics Cards Work?  Exploring GPU Architecture">
                <meta property="og:url" content="https://www.youtube.com/watch?v=h9Z4oGN89MU">
                <body><script>{"approxDurationMs":"1709569"}</script></body>
            "#,
        );
        let meta = Metadata::new(html).unwrap();

        assert_eq!(meta.id.as_str(), "h9Z4oGN89MU");
        assert_eq!(meta.duration.as_u64(), 1_709_569);
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
