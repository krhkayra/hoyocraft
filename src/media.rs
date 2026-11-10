//! Experimental HoYoPlay media metadata. No launcher theme is taken from the API.
//!
//! Call `fetch_genshin_media` from a worker thread: the blocking client has bounded
//! connect/total timeouts and a bounded response body, but must not run on Qt's UI thread.
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{io::Read, time::Duration};
use url::Url;

pub const GENSHIN_GAME_ID: &str = "gopR6Cufr3";
pub const BASIC_INFO_URL: &str =
    "https://sg-hyp-api.hoyoverse.com/hyp/hyp-connect/api/getAllGameBasicInfo";
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MediaAssets {
    pub video_url: String,
    pub background_url: String,
}

#[derive(Deserialize)]
struct ApiResponse {
    retcode: i64,
    message: Option<String>,
    data: Option<ApiData>,
}

#[derive(Deserialize)]
struct ApiData {
    game_info_list: Vec<GameInfo>,
}

#[derive(Deserialize)]
struct GameInfo {
    game: GameIdentity,
    #[serde(default)]
    backgrounds: Vec<Background>,
}

#[derive(Deserialize)]
struct GameIdentity {
    id: String,
}

#[derive(Deserialize)]
struct Background {
    video: Option<Asset>,
    background: Option<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    #[serde(default)]
    url: String,
}

pub fn fetch_genshin_media() -> Result<MediaAssets> {
    let client = reqwest::blocking::Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::limited(3))
        .user_agent(concat!("HoYoCraft/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("Could not initialize the experimental media client")?;
    let response = client
        .get(BASIC_INFO_URL)
        .query(&[
            ("launcher_id", "VYTpXlbWo8"),
            ("language", "en-us"),
            ("game_id", GENSHIN_GAME_ID),
        ])
        .send()
        .context("Could not fetch experimental Genshin media metadata")?
        .error_for_status()
        .context("The experimental media endpoint returned an HTTP error")?;
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES)
    {
        bail!("The experimental media response exceeded the size limit");
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_RESPONSE_BYTES + 1)
        .read_to_end(&mut bytes)
        .context("Could not read experimental media metadata")?;
    parse_response(&bytes)
}

fn asset_url(asset: Option<Asset>) -> String {
    asset
        .and_then(|asset| {
            let url = Url::parse(&asset.url).ok()?;
            (url.scheme() == "https"
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none())
            .then(|| url.to_string())
        })
        .unwrap_or_default()
}

/// Parse metadata only for the requested game. Unknown fields (including theme)
/// are deliberately ignored; a changed or unusable schema is a recoverable error.
pub fn parse_response(bytes: &[u8]) -> Result<MediaAssets> {
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        bail!("The experimental media response exceeded the size limit");
    }
    let response: ApiResponse = serde_json::from_slice(bytes)
        .context("The experimental media response has an unexpected format")?;
    if response.retcode != 0 {
        let message: String = response
            .message
            .unwrap_or_default()
            .chars()
            .take(160)
            .collect();
        bail!(
            "The experimental media API returned code {}: {message}",
            response.retcode
        );
    }
    let game = response
        .data
        .context("The experimental media response has no data")?
        .game_info_list
        .into_iter()
        .find(|game| game.game.id == GENSHIN_GAME_ID)
        .context("The experimental media response does not contain Genshin Impact")?;
    let mut static_fallback = None;
    for background in game.backgrounds {
        let assets = MediaAssets {
            video_url: asset_url(background.video),
            background_url: asset_url(background.background),
        };
        // Keep the video and its static counterpart from the same slot together.
        if !assets.video_url.is_empty() {
            return Ok(assets);
        }
        if static_fallback.is_none() && !assets.background_url.is_empty() {
            static_fallback = Some(assets);
        }
    }
    static_fallback.context("The experimental media response contains no usable HTTPS media URLs")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    #[ignore = "requires access to the public HoYoPlay API"]
    fn live_genshin_metadata_request() {
        let assets = fetch_genshin_media().unwrap();
        assert!(!assets.background_url.is_empty());
        assert!(!assets.video_url.is_empty());
    }

    fn response(backgrounds: Value) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "retcode": 0, "message": "OK", "data": { "game_info_list": [{
                "game": { "id": GENSHIN_GAME_ID }, "backgrounds": backgrounds
            }] }
        }))
        .unwrap()
    }

    #[test]
    fn captured_live_response_selects_video_and_its_static_counterpart() {
        // Public response captured 2026-11-10, not a mocked API schema.
        let assets =
            parse_response(include_bytes!("../tests/fixtures/genshin-basic-info.json")).unwrap();
        assert!(
            assets
                .video_url
                .starts_with("https://launcher-webstatic.hoyoverse.com/")
        );
        assert!(assets.video_url.ends_with(".webm"));
        assert!(assets.background_url.ends_with(".webp"));
        assert!(
            !assets
                .background_url
                .contains("f1ae06ddd13133caa934569acdd2bbcc")
        ); // theme URL
    }

    #[test]
    fn other_games_are_never_selected() {
        let bytes = serde_json::to_vec(&json!({ "retcode": 0, "data": { "game_info_list": [{
            "game": { "id": "another-game" }, "backgrounds": [{ "video": { "url": "https://example.com/video.webm" } }]
        }] }})).unwrap();
        assert!(parse_response(&bytes).is_err());
    }

    #[test]
    fn static_only_and_video_only_slots_are_supported() {
        let assets = parse_response(&response(json!([
            { "background": { "url": "https://example.com/static.webp" } }
        ])))
        .unwrap();
        assert!(assets.video_url.is_empty());
        assert_eq!(assets.background_url, "https://example.com/static.webp");
        let assets = parse_response(&response(json!([
            { "video": { "url": "https://example.com/video.webm" } }
        ])))
        .unwrap();
        assert!(assets.background_url.is_empty());
        assert_eq!(assets.video_url, "https://example.com/video.webm");
    }

    #[test]
    fn invalid_urls_are_ignored_and_theme_is_not_used() {
        let bytes = response(json!([
            { "video": { "url": "file:///tmp/private.webm" }, "background": { "url": "http://example.com/plain.webp" }, "theme": { "url": "https://example.com/theme.webp" } },
            { "video": { "url": "https://user:secret@example.com/video.webm" }, "background": { "url": "https://example.com/valid.webp" } }
        ]));
        let assets = parse_response(&bytes).unwrap();
        assert!(assets.video_url.is_empty());
        assert_eq!(assets.background_url, "https://example.com/valid.webp");
    }

    #[test]
    fn empty_error_malformed_and_oversized_responses_fail() {
        assert!(parse_response(&response(json!([]))).is_err());
        assert!(parse_response(br#"{"retcode":-1,"message":"not available"}"#).is_err());
        assert!(parse_response(br#"{"retcode":0,"data":{}}"#).is_err());
        assert!(parse_response(br#"{"data":{"game_info_list":[]}}"#).is_err());
        assert!(parse_response(&vec![b' '; MAX_RESPONSE_BYTES as usize + 1]).is_err());
    }
}
