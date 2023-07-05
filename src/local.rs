
use chrono::Utc;
use serde_json::{Value};
use yayti::{parsers::web::playlist::{Playlist,PlaylistVideo}, helpers::AuthorThumbnail};
use crate::{routes::video::fetch_player_with_cache, settings::AppSettings};

fn video_link_or_id_to_id(link_like_video_id: &str) -> &str {
  if link_like_video_id.contains("/watch?v=") {
    match link_like_video_id.split("/watch?v=").nth(1) {
      Some(video_id) => {
        video_id
      },
      None => {
        link_like_video_id
      }
    }
  } else if link_like_video_id.contains("youtu.be/") {
    match link_like_video_id.split("youtu.be/").nth(1) {
      Some(video_id) => {
        video_id
      },
      None => {
        link_like_video_id
      }
    }
  } else {
    link_like_video_id
  }
}

async fn array_item_into_video(item: &Value, index: u32, app_settings: &AppSettings) -> Option<PlaylistVideo> {
  match item.as_str() {
    Some(item) => {
      let video_id = video_link_or_id_to_id(&item);
      let mut app_settings = app_settings.clone();
      app_settings.cache_timeout = u64::MAX;// don't be picky with fetching these videos from cache
      // we don't need anything time sensitive like streaming data, just basic info
      let Ok(next_value) = fetch_player_with_cache(video_id, "en", &app_settings, false, Some(&app_settings.pub_url.clone().unwrap_or(format!("{}:{}", app_settings.ip_address, app_settings.port)))).await else { return None };
      Some(PlaylistVideo {
        title: yayti::parsers::web::video::get_title(&next_value),
        video_id: Some(String::from(video_id)),
        author: yayti::parsers::web::video::get_author(&next_value),
        author_id: yayti::parsers::web::video::get_author_id(&next_value),
        video_thumbnails: Some(yayti::helpers::generate_yt_video_thumbnails_within_max_size(video_id, 360).into_iter().map(|thumbnail| AuthorThumbnail { url: thumbnail.url, width: thumbnail.width, height: thumbnail.height }).collect::<Vec::<AuthorThumbnail>>()),
        index: Some(index as i32),
        length_seconds: yayti::parsers::web::video::get_length_seconds(&next_value)
      })
    },
    None => {
      None
    }
  }
}

// converts a local playlist to invidious format
pub async fn local_playlist_to_iv(title: &str, playlist_json: &Value, app_settings: &AppSettings) -> Option<Playlist> {
  let uri = app_settings.pub_url.clone().unwrap_or(format!("http://{}:{}", app_settings.ip_address, app_settings.port));
  let items = match playlist_json.as_array() {
    Some(items) => items.to_owned(),
    None => match playlist_json["videos"].as_array() {
      Some(videos) => videos.to_owned(),
      None => vec![]
    }
  };
  if items.len() > 0 {
    let mut i = 0;
    // playlist of video ids / links containing video ids
    let mut videos = vec![];
    for item in items {
      i+=1;
      match array_item_into_video(&item, i, app_settings).await {
        Some(video) => {
          videos.push(video);
        },
        None => {}
      };
    }
    let videos_count = videos.len() as i32;
    Some(Playlist {
      title: Some(playlist_json["title"].as_str().map(String::from).unwrap_or(String::from(title))),
      playlist_id: Some(String::from(title)),
      videos: Some(videos),
      playlist_thumbnails: None,
      author: Some(String::from("yaytapi")),
      author_id: Some(String::from("::yaytapi_local::")),// TODO ✏ implement local playlists channel
      author_url: None,
      // DUMMY DATA freetube will blow up if we don't send this info
      author_thumbnails: Some(vec![AuthorThumbnail {
        url: format!("{}/static/icon.png", uri),
        width: 400,
        height: 400
      }, AuthorThumbnail {
        url: format!("{}/static/icon.png", uri),
        width: 400,
        height: 400
      }, AuthorThumbnail {
        url: format!("{}/static/icon.png", uri),
        width: 400,
        height: 400
      }]),
      description: Some(playlist_json["description"].as_str().map(String::from).unwrap_or(String::from(""))),
      description_html: Some(String::from("")),
      video_count: Some(videos_count),
      view_count: Some(0),
      updated: Some(Utc::now().timestamp()),
      is_listed: Some(false)
    })
  } else {
    None
  }
}
