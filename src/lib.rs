use futures::StreamExt;
use regex::Regex;
use reqwest::{Client, Url};
use scraper::{Html, Selector};
use std::io::prelude::*;
use std::path::Path;

pub const MAX_BYTES: usize = 70000;
pub const OFFSET_CHUNKS_COUNT: usize = 360;

// `--`, the 11-character YouTube ID, and `.jpg` use 17 of the 255 bytes allowed
// for a portable filename component, leaving 238 bytes for the title.
const MAX_THUMBNAIL_TITLE_BYTES: usize = 238;
const INVALID_FILENAME_CHARS: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

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
    title_selector: Selector,
    script_selector: Selector,
    duration_regex: Regex,
    url_selector: Selector,
}

impl<'a> MetadataBuilder<'a> {
    pub fn new(client: &'a Client, max_bytes: usize, offset_chunk_number: usize) -> Self {
        Self {
            client,
            max_bytes,
            offset_chunk_number,
            title_selector: Selector::parse("meta[name='title']").unwrap(),
            script_selector: Selector::parse("body script").unwrap(),
            duration_regex: Regex::new(r#""approxDurationMs":"(\d+)""#).unwrap(),
            url_selector: Selector::parse("meta[property='og:url']").unwrap(),
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
        self.parse_metadata(&Html::parse_document(&html))
            .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidData, message).into())
    }

    pub fn parse_metadata(&self, html: &Html) -> Result<Metadata, &'static str> {
        let title = html
            .select(&self.title_selector)
            .next()
            .and_then(|elem| elem.value().attr("content"))
            .ok_or("Title wasn't found")?
            .to_string()
            .into_boxed_str();

        let script_block = html.select(&self.script_selector);

        let dur: u64 = script_block
            .flat_map(|elemref| elemref.text())
            .find_map(|text| self.duration_regex.captures(text))
            .ok_or("Regex didn't find approxDurationMs")?[1]
            .parse()
            .expect("Regex found non numeric value while searching for approxDurationMs");

        let url = html
            .select(&self.url_selector)
            .next()
            .and_then(|elem| elem.value().attr("content"))
            .ok_or("Video URL wasn't found in provided HTML")?;

        let id = YoutubeVideoId::parse(url)?;

        let img_name = thumbnail_filename(&title, &id);

        Ok(Metadata {
            title,
            id,
            duration: Millis::new(dur),
            img_name,
        })
    }
}

fn thumbnail_filename(title: &str, id: &YoutubeVideoId) -> Box<str> {
    // Reserve one byte for a possible Windows-device-name prefix.
    let mut sanitized: String = truncate_to_byte_length(title, MAX_THUMBNAIL_TITLE_BYTES - 1)
        .chars()
        .map(|character| {
            if character.is_control() || INVALID_FILENAME_CHARS.contains(&character) {
                '_'
            } else {
                character
            }
        })
        .collect();

    while matches!(sanitized.chars().last(), Some(' ' | '.')) {
        sanitized.pop();
    }
    if sanitized.is_empty() {
        sanitized.push_str("untitled");
    } else if is_windows_reserved_name(&sanitized) {
        sanitized.insert(0, '_');
    }

    format!("{sanitized}--{}.jpg", id.as_str()).into_boxed_str()
}

fn truncate_to_byte_length(value: &str, maximum_length: usize) -> &str {
    if value.len() <= maximum_length {
        return value;
    }

    let mut end = maximum_length;
    while !value.is_char_boundary(end) {
        end -= 1;
    }

    &value[..end]
}

fn is_windows_reserved_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name);
    matches!(
        stem.to_ascii_uppercase().as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

impl Metadata {
    /// Downloads image URL extracted from youtuble link and saves it
    /// using a sanitized title and `YouTube` video ID as the `jpg` filename
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
    use super::{thumbnail_filename, Client, Html, MetadataBuilder, YoutubeVideoId};

    #[test]
    fn test_metadata_from_html() {
        let html = Html::parse_document(
            r#"
                <meta name="title" content="How do Graphics Cards Work?  Exploring GPU Architecture">
                <meta property="og:url" content="https://www.youtube.com/watch?v=h9Z4oGN89MU">
                <body><script>{"approxDurationMs":"1709569"}</script></body>
            "#,
        );
        let client = Client::new();
        let metadata_builder = MetadataBuilder::new(&client, 0, 0);
        let meta = metadata_builder.parse_metadata(&html).unwrap();

        assert_eq!(meta.id.as_str(), "h9Z4oGN89MU");
        assert_eq!(meta.duration.as_u64(), 1_709_569);
        assert_eq!(
            &meta.title[..],
            "How do Graphics Cards Work?  Exploring GPU Architecture"
        );
        assert_eq!(
            &meta.img_name[..],
            "How do Graphics Cards Work_  Exploring GPU Architecture--h9Z4oGN89MU.jpg"
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

    #[test]
    fn thumbnail_filename_sanitizes_forbidden_characters_and_trailing_characters() {
        let id = YoutubeVideoId::parse("https://youtu.be/h9Z4oGN89MU").unwrap();

        assert_eq!(
            thumbnail_filename("A<>:\"/\\|?*\u{0000}B.  ", &id).as_ref(),
            "A__________B--h9Z4oGN89MU.jpg"
        );
    }

    #[test]
    fn thumbnail_filename_handles_empty_and_reserved_names() {
        let id = YoutubeVideoId::parse("https://youtu.be/h9Z4oGN89MU").unwrap();

        assert_eq!(
            thumbnail_filename(" . ", &id).as_ref(),
            "untitled--h9Z4oGN89MU.jpg"
        );
        assert_eq!(
            thumbnail_filename("CON", &id).as_ref(),
            "_CON--h9Z4oGN89MU.jpg"
        );
    }

    #[test]
    fn thumbnail_filename_limits_length_without_breaking_unicode_or_uniqueness() {
        let first_id = YoutubeVideoId::parse("https://youtu.be/h9Z4oGN89MU").unwrap();
        let second_id = YoutubeVideoId::parse("https://youtu.be/dQw4w9WgXcQ").unwrap();
        let filename = thumbnail_filename(&"😀".repeat(100), &first_id);

        assert!(filename.len() <= 255);
        assert!(filename.ends_with("--h9Z4oGN89MU.jpg"));
        assert_ne!(
            thumbnail_filename("Same title", &first_id),
            thumbnail_filename("Same title", &second_id)
        );
    }
}
