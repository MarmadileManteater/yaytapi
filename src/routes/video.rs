
use serde_json::{Map, json, Value, to_string_pretty, from_str};
use serde::{Serialize, Deserialize};
use chrono::prelude::Utc;
use actix_web::web::{Path, Data, Query};
use actix_web::{HttpResponse, Responder, get};
use yayti::extractors::{ciphers::get_player_js_id, ciphers::get_player_response, innertube::{fetch_next,fetch_player_with_sig_timestamp}};
use yayti::parsers::{ClientContext, ciphers::{extract_sig_timestamp, decipher_streams}, web::video::{fmt_inv_with_existing_map, fmt_inv}};
use yayti::helpers::generate_yt_video_thumbnail_url;
use reqwest::Client;
use crate::settings::AppSettings;
use crate::helpers::DbWrapper;

async fn get_previous_data(collection: &str, key: &str, db: &DbWrapper, app_settings: &AppSettings) -> Option<Value> {
 if app_settings.cache_requests {
   match db.seek_for_json(collection, key).await {
     Some(json) => {
       match json["timestamp"].as_i64() {
         Some(timestamp) => {
           let current_timestamp = Utc::now().timestamp();
           let offset = current_timestamp - timestamp;
           if offset as i32 > app_settings.cache_timeout {
             db.delete(collection, key).await;
             None
           } else {
             Some(json)
           }
         },
         None => Some(json)
       }
     },
     None => None
   }
 } else {
   None
 }
}

async fn fetch_next_with_cache(id: &str, lang: &str, app_settings: &AppSettings) -> Result<Value, reqwest::Error> {
  // create a connection to the db
  let db = app_settings.get_json_db().await;
  let previous_data = get_previous_data("next", &format!("{}-{}", id, lang), &db, &app_settings).await;
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
            db.insert_json("next", &format!("{}-{}", id, lang), &json).await;
          }
          Ok(json)
        },
        Err(error) => Err(error)
      }
    }
  }
}

async fn fetch_player_with_cache(id: &str, lang: &str, app_settings: &AppSettings) -> Result<Value, reqwest::Error> {
  let db = app_settings.get_json_db().await;

  let previous_data = get_previous_data("player", &format!("{}-{}", id, lang), &db, app_settings).await;
  match previous_data {
    Some(json) => {
      Ok(json)
    },
    None => {
      let Ok(player_js_id) = get_player_js_id().await else { todo!() };

      let previous_js_id = get_previous_data("player", "player.js-id", &db, app_settings).await;
      let need_new_player_js = match previous_js_id {
        Some(previous_js_id_str) => {
          previous_js_id_str.as_str().unwrap_or("") != player_js_id
        },
        None => {
          true
        }
      };
      
      let (player_js_response, signature_timestamp) = if need_new_player_js {
        let Ok(player_js_response) = get_player_response(&player_js_id).await else { todo!() };
        let signature_timestamp = extract_sig_timestamp(&player_js_response);
        db.delete("player", "player.js-id").await;
        db.delete("player", "player.js").await;
        db.delete("player", "signature_timestamp").await;
        db.insert_json("player", "player.js-id", &json!(player_js_id)).await;
        db.insert_json("player", "player.js", &json!(player_js_response)).await;
        db.insert_json("player", "signature_timestamp", &json!(signature_timestamp)).await;
        (player_js_response, signature_timestamp)
      } else {
        let Some(player_js_response) = get_previous_data("player", "player.js", &db, app_settings).await else { todo!() };
        let Some(signature_timestamp) = get_previous_data("player", "signature_timestamp", &db, app_settings).await else { todo!() };
        (String::from(player_js_response.as_str().unwrap()), signature_timestamp.as_i64().unwrap() as i32)
      };
    
      match fetch_player_with_sig_timestamp(id, signature_timestamp, &ClientContext::default_web(), Some(lang)).await {
        Ok(player) => {
          let Ok(mut json) = from_str::<Value>(&player) else { todo!() };
          let mut streams = Vec::<String>::new();
          let Some(formats) = json["streamingData"]["formats"].as_array() else { todo!() };
          let Some(adaptive_formats) = json["streamingData"]["adaptiveFormats"].as_array() else { todo!() };
          match formats[0]["url"].as_str() {
            Some(_) => {},
            None => {
              for k in 0..formats.len() {
                streams.push(String::from(formats[k]["signatureCipher"].as_str().unwrap()));
              }
              for k in 0..adaptive_formats.len() {
                streams.push(String::from(adaptive_formats[k]["signatureCipher"].as_str().unwrap()));
              }
              let Ok(deciphered_streams) = decipher_streams(streams, &player_js_response) else { todo!() };
              let formats_len = formats.len();
              let adaptive_len = adaptive_formats.len();
              let mut i = 0;
              for k in 0..formats_len {
                json["streamingData"]["formats"][k]["url"] = json!(deciphered_streams[i]);
                i = i + 1;
              }
              for k in 0..adaptive_len {
                json["streamingData"]["adaptiveFormats"][k]["url"] = json!(deciphered_streams[i]);
                i = i + 1;
              }
            }
          }
          json["timestamp"] = Utc::now().timestamp().into();
          if app_settings.cache_requests {
            db.insert_json("player", &format!("{}-{}", id, lang), &json).await;
          }
          Ok(json.clone())
        },
        Err(error) => Err(error)
      }
    }
  }
}

#[derive(Deserialize)]
pub struct VideoEndpointQueryParams {
  hl: Option<String>,
  fields: Option<String>
}

#[derive(Serialize, Deserialize)]
pub struct InnerTubeResponse {
  next: Value,
  player: Value
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
  
  let Ok(player_res) = fetch_player_with_cache(&video_id, &lang, &app_settings).await else { todo!() };
  let mut json = fmt_inv(&player_res, &lang);
  let Ok(next_res) = fetch_next_with_cache(&video_id, &lang, &app_settings).await else { todo!() };
  let json = fmt_inv_with_existing_map(&next_res, &lang, json);
  /*let data = InnerTubeResponse {
    next: next_res,
    player: player_res
  };*/
  let Ok(json_response) = to_string_pretty(&json) else { todo!() };
  HttpResponse::Ok().content_type("application/json").body(json_response)
}

#[get("/vi/{video_id}/{file_name}.jpg")]
pub async fn video_thumbnail_proxy(params: Path<(String, String)>) -> impl Responder {
  let video_id = String::from(&params.0);
  let file_name = String::from(&params.1);
  let client = Client::new();
  match client.get(generate_yt_video_thumbnail_url(&video_id, &file_name)).send().await {
    Ok(thumbnail_response) => {
      HttpResponse::Ok().content_type("image/jpeg").streaming(thumbnail_response.bytes_stream())
    },
    Err(_err) => HttpResponse::Ok().body("error")
  }
}
