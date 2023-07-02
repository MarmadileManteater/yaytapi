use actix_web::web::{Path, Data, Query};
use actix_web::{HttpResponse, Responder, get};
use serde::{Serialize, Deserialize};
use actix_web::http::StatusCode;
use serde_json::{from_str,to_string, Value, Map};
use yayti::helpers::generate_playlist_continuation;
use yayti::parsers::ClientContext;
use yayti::extractors::innertube::{fetch_playlist, fetch_continuation};
use yayti::parsers::web::playlist::parse;
use std::str::FromStr;
use crate::helpers::get_previous_data;
use crate::AppSettings;

#[derive(Serialize, Deserialize)]
pub struct PlaylistEndpointQueryParams {
  page: Option<String>,
  hl: Option<String>
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
          match generate_playlist_continuation(&playlist_id, page_num) {
            Ok(continuation) => {
              let continuation = match fetch_continuation("browse", &continuation, &ClientContext::default_web(), hl).await {
                Ok(continuation) => continuation,
                Err(error) => return HttpResponse::build(StatusCode::from_u16(500).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\", \"message\": \"Error generating playlist continuation\", \"inner_error\": \"{}\" }}", error))
              };
              let continuation_data = match from_str::<Value>(&continuation) {
                Ok(result) => result,
                Err(error) => {
                  return HttpResponse::build(StatusCode::from_u16(500).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\", \"message\": \"Error parsing continuation response to JSON\", \"inner_error\": \"{}\" }}", error))
                }
              };
              continuation_data
            },
            Err(_) => {
              return HttpResponse::build(StatusCode::from_u16(500).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\", \"message\": \"Error generating continuation\" }}"))
            }
          }
        },
        Err(_) => {
          return HttpResponse::build(StatusCode::from_u16(400).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\", \"message\": \"Given page is not a number: {}\" }}", page));
        }
      }
    },
    None => {
      let Ok(playlist) = fetch_playlist(&playlist_id, &ClientContext::default_web(), hl, None).await else { todo!() };
      let Ok(playlist_value) = from_str::<Value>(&playlist) else { todo!() };
      playlist_value
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
