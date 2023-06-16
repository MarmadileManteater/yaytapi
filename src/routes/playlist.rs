use actix_web::web::{Path, Data, Query};
use actix_web::{HttpResponse, Responder, get};
use serde::{Serialize, Deserialize};
use actix_web::http::StatusCode;
use serde_json::{from_str,to_string, Value};
use yayti::parsers::ClientContext;
use yayti::extractors::innertube::fetch_playlist;
use yayti::parsers::web::playlist::parse;
use crate::helpers::get_previous_data;
use crate::AppSettings;

#[derive(Serialize, Deserialize)]
pub struct CommentEndpointQueryParams {
  continuation: Option<String>,
  hl: Option<String>
}

#[get("/api/v1/playlists/{playlist_id}")]
pub async fn playlist_endpoint(path: Path<String>, query: Query<CommentEndpointQueryParams>, app_settings: Data<AppSettings>) -> impl Responder {
  let playlist_id = path.into_inner();
  let hl = query.hl.as_deref();
  let Ok(playlist) = fetch_playlist(&playlist_id, &ClientContext::default_web(), hl, None).await else { todo!() };
  let Ok(playlist_value) = from_str::<Value>(&playlist) else { todo!() };
  let playlist_result = parse(&playlist_value, &hl.unwrap_or("en"));
  let mut map = playlist_result.into_inv();
  if app_settings.return_innertube_response {
    map.insert(String::from("innertube"), playlist_value);
  }
  HttpResponse::build(StatusCode::from_u16(200).unwrap()).content_type("application/json").body(to_string(&map).unwrap())
}
