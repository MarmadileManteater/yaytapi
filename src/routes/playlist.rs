use actix_web::web::{Path, Data, Query};
use actix_web::{HttpResponse, Responder, get, App};
use chrono::Utc;
use serde::{Serialize, Deserialize};
use actix_web::http::StatusCode;
use serde_json::{from_str,to_string, Value, Map, json};
use yayti::helpers::generate_playlist_continuation;
use yayti::parsers::ClientContext;
use yayti::extractors::innertube::{fetch_playlist, fetch_continuation};
use yayti::parsers::web::playlist::parse;
use std::str::FromStr;
use crate::helpers::{get_previous_data, DbWrapper};
use crate::AppSettings;

#[derive(Serialize, Deserialize)]
pub struct PlaylistEndpointQueryParams {
  page: Option<String>,
  hl: Option<String>
}

pub enum FetchPlaylistError {
  FailedToFetchPlaylist,
  FailedToParsePlaylist,
  FailedToGenerateContinuation,
  FailedToFetchContinuation(reqwest::Error),
  FailedToParseContinuationResponse(serde_json::Error)
}

async fn fetch_continuation_with_cache(db: &DbWrapper, app_settings: &AppSettings, playlist_id: &str, lang: &str, page_num: i32) -> Result<Value, FetchPlaylistError> {
  match generate_playlist_continuation(&playlist_id, page_num) {
    Ok(continuation) => {
      let token = continuation;
      match get_previous_data("playlist", &format!("{}-{}", token, lang), db, app_settings).await {
        Some(previous_data) => Ok(previous_data),
        None => {
          let continuation = match fetch_continuation("browse", &token, &ClientContext::default_web(), Some(lang)).await {
            Ok(continuation) => continuation,
            Err(error) => return Err(FetchPlaylistError::FailedToFetchContinuation(error))
          };
          let mut continuation_data = match from_str::<Value>(&continuation) {
            Ok(result) => result,
            Err(error) => {
              return Err(FetchPlaylistError::FailedToParseContinuationResponse(error));
            }
          };
          continuation_data["timestamp"] = Utc::now().timestamp().into();
          if app_settings.cache_requests {
            db.insert_json("playlist", &format!("{}-{}", token, lang), &json!(continuation_data)).await;
          }
          Ok(continuation_data)
        }
      }
    },
    Err(_) => {
      return Err(FetchPlaylistError::FailedToGenerateContinuation)
    }
  }
}

async fn fetch_playlist_with_cache(db: &DbWrapper, app_settings: &AppSettings, playlist_id: &str, lang: &str) -> Result<Value, FetchPlaylistError> {
  let previous_data = get_previous_data("playlist", &format!("{}-{}", playlist_id, lang), db, app_settings).await;
  match previous_data {
    Some(previous_data) => Ok(previous_data),
    None => {
      let Ok(playlist) = fetch_playlist(&playlist_id, &ClientContext::default_web(), Some(lang), None).await else { return Err(FetchPlaylistError::FailedToFetchPlaylist) };
      let Ok(mut playlist_value) = from_str::<Value>(&playlist) else { return Err(FetchPlaylistError::FailedToParsePlaylist) };
      playlist_value["timestamp"] = Utc::now().timestamp().into();
      if app_settings.cache_requests {
        db.insert_json("playlist", &format!("{}-{}", playlist_id, lang), &json!(playlist_value)).await;
      }
      Ok(playlist_value)
    }
  }
}

#[get("/api/v1/playlists/{playlist_id}")]
pub async fn playlist_endpoint(path: Path<String>, query: Query<PlaylistEndpointQueryParams>, app_settings: Data<AppSettings>) -> impl Responder {
  let playlist_id = path.into_inner();
  let page = query.page.as_deref();
  let hl = query.hl.as_deref();
  let mut map = Map::<String, Value>::new();
  let playlist_value = match page {
    Some(page) => {
      match i32::from_str(page) {
        Ok(page_num) => {
          if page_num < 1 {
            return HttpResponse::build(StatusCode::from_u16(400).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\", \"message\": \"Page must be greater than zero\" }}"));
          }
          match fetch_continuation_with_cache(&app_settings.get_json_db().await, &app_settings, &playlist_id, hl.unwrap_or("en"), page_num).await {
            Ok(result) => result,
            Err(error) => {
              match error {
                FetchPlaylistError::FailedToGenerateContinuation => {
                  return HttpResponse::build(StatusCode::from_u16(500).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\", \"message\": \"Error generating playlist continuation\" }}"));
                },
                FetchPlaylistError::FailedToFetchContinuation(error) => {
                  return HttpResponse::build(StatusCode::from_u16(500).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\", \"message\": \"Unknown error\" }}"));
                },
                FetchPlaylistError::FailedToParseContinuationResponse(error) => {
                  return HttpResponse::build(StatusCode::from_u16(500).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\", \"message\": \"Error parsing continuation response to JSON\", \"inner_error\": \"{}\" }}", error));
                },
                _ => {
                  return HttpResponse::build(StatusCode::from_u16(500).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\", \"message\": \"Unknown error\" }}"));
                }
              };
            }
          }
        },
        Err(_) => {
          return HttpResponse::build(StatusCode::from_u16(400).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\", \"message\": \"Given page is not a number: {}\" }}", page));
        }
      }
    },
    None => {
      match fetch_playlist_with_cache(&app_settings.get_json_db().await, &app_settings, &playlist_id, &hl.unwrap_or("en")).await { 
        Ok(playlist) => playlist,
        Err(_) => {
          return HttpResponse::build(StatusCode::from_u16(500).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\" }}"));
        }
      }
    }
  };
  let playlist_result = match parse(&playlist_value, &hl.unwrap_or("en")) {
    Ok(playlist_result) => playlist_result,
    Err(error) => {
      let alert = &error.alerts[0];
      return HttpResponse::build(StatusCode::from_u16(404).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\", \"message\": \"{}\", \"message_type\": \"{}\" }}", alert.alert_text.clone().unwrap_or(String::from("")), alert.alert_type.clone().unwrap_or(String::from(""))));
    }
  };
  map = playlist_result.into_inv();
  if app_settings.return_innertube_response {
    map.insert(String::from("innertube"), playlist_value);
  }
  HttpResponse::build(StatusCode::from_u16(200).unwrap()).content_type("application/json").body(to_string(&map).unwrap())
}
