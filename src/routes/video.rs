
use unqlite::{UnQLite};
use serde_json::{Value, to_string_pretty, from_str};
use serde::{Deserialize};
use chrono::prelude::Utc;
use actix_web::web::{Path, Data, Query};
use actix_web::{HttpResponse, Responder, get};
use yayti::extractors::innertube::fetch_next;
use yayti::parsers::ClientContext;
use crate::helpers::JsonDb;
use crate::settings::AppSettings;

async fn fetch_next_with_cache(id: &str, lang: &str, app_settings: &AppSettings) -> Result<Value, reqwest::Error> {
  let db = UnQLite::create(&app_settings.db_name);
  let previous_data = if app_settings.cache_requests {
    match db.seek_for_json::<Value>(&format!("{}-{}", id, lang)) {
      Some(json) => {
        match json["timestamp"].as_i64() {
          Some(timestamp) => {
            let current_timestamp = Utc::now().timestamp();
            let offset = current_timestamp - timestamp;
            if offset as i32 > app_settings.cache_timeout {
              db.delete(&format!("{}-{}", id, lang));
              None
            } else {
              Some(json)
            }
          },
          None => None
        }
      },
      None => None
    }
  } else {
    None
  };
  match previous_data {
    Some(json) => {
      Ok(json)
    },
    None => {
      match fetch_next(id, &ClientContext::default_web(), Some(lang)).await {
        Ok(next) => {
          let Ok(mut json) = from_str::<Value>(&next) else { todo!() };
          json["timestamp"] = Utc::now().timestamp().into();
          if app_settings.cache_requests {
            db.insert_json(&format!("{}-{}", id, lang), &json);
          }
          Ok(json)
        },
        Err(error) => Err(error)
      }
    }
  }
}

#[derive(Deserialize)]
struct VideoEndpointQueryParams {
  hl: Option<String>,
  fields: Option<String>
}

#[get("/api/v1/videos/{video_id}")]
pub async fn video_endpoint(path: Path<String>, query: Query<VideoEndpointQueryParams>, app_settings: Data<AppSettings>) -> impl Responder {
  let video_id = path.into_inner();
  let lang = query.hl.clone().unwrap_or(String::from("en"));
  let fields = match &query.fields {
    Some(fields) => {
      if fields.contains(",") {
        fields.split(",").into_iter().map(|field| String::from(field)).collect::<Vec::<String>>()
      } else {
        vec!(String::from(fields))
      }
    },
    None => Vec::<String>::new()
  };
  
  let Ok(next_res) = fetch_next_with_cache(&video_id, &lang, &app_settings).await else { todo!() };
  let Ok(json_response) = to_string_pretty(&next_res) else { todo!() };
  HttpResponse::Ok().content_type("application/json").body(json_response)
}
