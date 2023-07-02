
use chrono::Utc;
use serde_json::{Value, Map};
use yayti::{parsers::web::playlist::{Playlist,PlaylistVideo}, helpers::AuthorThumbnail};
use crate::{routes::video::fetch_player_with_cache, settings::AppSettings};
// converts a local playlist to invidious format
pub async fn local_playlist_to_iv(title: &str, playlist_json: &Value, app_settings: &AppSettings) -> Option<Playlist> {
  match playlist_json.as_array() {
    Some(items) => {
      if items.len() > 0 {
        match items[0].as_str() {
          Some(_) => {
            let mut i = 0;
            // playlist of video ids / links containing video ids
            let mut videos = vec![];
            for link_like_video_id in items {
              match link_like_video_id.as_str() {
                Some(link_like_video_id) => {
                  let video_id = if link_like_video_id.contains("/watch?v=") {
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
                  };
                  let Ok(next_value) = fetch_player_with_cache(video_id, "en", app_settings, false, Some(&app_settings.pub_url.clone().unwrap_or(format!("{}:{}", app_settings.ip_address, app_settings.port)))).await else { return None };
                  i+=1;
                  videos.push(PlaylistVideo {
                    title: yayti::parsers::web::video::get_title(&next_value),
                    video_id: Some(String::from(video_id)),
                    author: yayti::parsers::web::video::get_author(&next_value),
                    author_id: yayti::parsers::web::video::get_author_id(&next_value),
                    video_thumbnails: Some(yayti::helpers::generate_yt_video_thumbnails_within_max_size(video_id, 360).into_iter().map(|thumbnail| AuthorThumbnail { url: thumbnail.url, width: thumbnail.width, height: thumbnail.height }).collect::<Vec::<AuthorThumbnail>>()),
                    index: Some(i),
                    length_seconds: yayti::parsers::web::video::get_length_seconds(&next_value)
                  })
                },
                None => {}
              }
            }
            let videos_count = videos.len() as i32;
            Some(Playlist {
              title: Some(String::from(title)),
              playlist_id: Some(String::from(title)),
              videos: Some(videos),
              playlist_thumbnails: None,
              author: Some(String::from("yaytapi")),
              author_id: None,
              author_url: None,
              author_thumbnails: Some(vec![AuthorThumbnail {
                url: String::from("/default_user.jpg"),
                width: 300,
                height: 300
              }, AuthorThumbnail {
                url: String::from("/default_user.jpg"),
                width: 300,
                height: 300
              }, AuthorThumbnail {
                url: String::from("/default_user.jpg"),
                width: 300,
                height: 300
              }]),
              description: Some(String::from("")),
              description_html: Some(String::from("")),
              video_count: Some(videos_count),
              view_count: Some(0),
              updated: Some(Utc::now().timestamp()),
              is_listed: Some(false)
            })
          },
          None => None
        }
      } else {
        None
      }
    },
    None => None
  }
}
