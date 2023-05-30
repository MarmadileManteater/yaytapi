
use serde_json::{json, Value, to_string_pretty, to_string, from_str, Map};
use serde::{Serialize, Deserialize};
use chrono::prelude::Utc;
use actix_web::web::{Path, Data, Query, Payload};
use actix_web::{HttpResponse, Responder, get, route};
use yayti::extractors::{ciphers::get_player_js_id, ciphers::get_player_response, innertube::{fetch_next,fetch_player_with_sig_timestamp}};
use yayti::parsers::{ClientContext, ciphers::{extract_sig_timestamp, decipher_streams}, ciphers, web::video::{fmt_inv_with_existing_map, fmt_inv, get_legacy_formats, get_adaptive_formats}};
use yayti::helpers::{generate_yt_video_thumbnail_url,generate_yt_video_thumbnails_within_max_size};
use reqwest::Client;
use std::str::FromStr;
use std::num::ParseIntError;
use actix_web::http::StatusCode;
use actix_web::HttpRequest;
use std::fmt::{Formatter, Display};
use urlencoding::decode;
use urlencoding::encode;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use futures_util::StreamExt;
use crate::settings::AppSettings;
use crate::helpers::get_previous_data;
use crate::helpers::DbWrapper;
use crate::helpers::ActixHeadersIntoReqwest;
use crate::helpers::ReqwestHeadersIntoResponseBuilder;

async fn fetch_player_js_with_cache(db: &DbWrapper, app_settings: &AppSettings, player_js_id_option: Option<String>) -> Result<(String, i32, String), FetchPlayerError> {
  let player_js_id = match &player_js_id_option {
    Some (player_js_id) => String::from(player_js_id),
    None => String::from(match get_player_js_id().await {
      Ok(player_js_id) => player_js_id,
      Err(error) => {
        match error {
          Some(error) => return Err(FetchPlayerError::Reqwest(error)),
          None => return Err(FetchPlayerError::PlayerJsIdNotFound)
        }
      }
    })
  };
  let need_new_player_js = match &player_js_id_option {
    Some(_) => true,
    None => {
      let previous_js_id = get_previous_data("player", "player.js-id", &db, app_settings).await;
      match previous_js_id {
        Some(previous_js_id_str) => {
          previous_js_id_str.as_str().unwrap_or("") != player_js_id
        },
        None => {
          true
        }
      }
    }
  };

  
  if need_new_player_js {
    let player_js_response = match get_player_response(&player_js_id).await {
      Ok(player_js_response) => player_js_response,
      Err(error) => {
        return Err(FetchPlayerError::Reqwest(error));
      }
    };
    let signature_timestamp = match extract_sig_timestamp(&player_js_response) {
      Ok(signature_timestamp) => signature_timestamp,
      Err(error) => {
        return Err(FetchPlayerError::SignatureTimestampNotFound(error));
      }
    };

    match player_js_id_option {
      Some(_) => {},
      None => {
        db.delete("player", "player.js-id").await;
        db.insert_json("player", "player.js-id", &json!(player_js_id)).await;
        db.delete("player", &format!("player.js-{}", player_js_id)).await;
        db.insert_json("player", &format!("player.js-{}", player_js_id), &json!(player_js_response)).await;
        db.delete("player", "signature_timestamp").await;
        db.insert_json("player", "signature_timestamp", &json!(signature_timestamp)).await;
      }
    };

    Ok((player_js_response, signature_timestamp, player_js_id))
  } else {
    let Some(player_js_response) = get_previous_data("player", &format!("player.js-{}", player_js_id), &db, app_settings).await else { todo!() };
    let Some(signature_timestamp) = get_previous_data("player", "signature_timestamp", &db, app_settings).await else { todo!() };
    Ok((String::from(player_js_response.as_str().unwrap()), signature_timestamp.as_i64().unwrap() as i32, player_js_id))
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

pub enum FetchPlayerError {
  Reqwest(reqwest::Error),
  PlayerJsIdNotFound,
  SignatureTimestampNotFound(ParseIntError),
  FailedToSerializePlayer,
  ResponseUnplayable,
  LoginRequired,
  FailedToDecipher(String)
}

impl Display for FetchPlayerError {
  fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
    write!(f, "{}", match self {
      FetchPlayerError::Reqwest(error) => format!("Error making request to innertube {}", error),
      FetchPlayerError::PlayerJsIdNotFound => format!("No player.js id found in `/iframe_api` response"),
      FetchPlayerError::SignatureTimestampNotFound(_) => format!("Unable to parse sig timestamp from player.js response"),
      FetchPlayerError::FailedToSerializePlayer => format!("Failed to serialize the JSON response from innertube (this probably means the response was the wrong mime type)"),
      FetchPlayerError::ResponseUnplayable => format!("Response is unplayable"),
      FetchPlayerError::LoginRequired => format!("Login required"),
      FetchPlayerError::FailedToDecipher(error) => format!("Failed to decipher: {}", error)
    })
  }
}

async fn fetch_player_with_cache(id: &str, lang: &str, app_settings: &AppSettings, local: bool, hostname: Option<&str>) -> Result<Value,FetchPlayerError> {
  let hostname = app_settings.clone().pub_url.unwrap_or(String::from(hostname.unwrap_or("")));
  let db = app_settings.get_json_db().await;
  let previous_data = get_previous_data("player", &format!("{}-{}-{}", id, lang, local), &db, app_settings).await;
  match previous_data {
    Some(json) => {
      Ok(json)
    },
    None => {
      let (player_js_response, signature_timestamp, player_js_id) = match fetch_player_js_with_cache(&db, &app_settings, None).await {
        Ok(response) => response,
        Err(error) => {
          return Err(error);
        }
      };
      let context = if app_settings.use_android_endpoint_for_streams {
        ClientContext::default_android()
      } else {
        ClientContext::default_web()
      };
      match fetch_player_with_sig_timestamp(id, signature_timestamp, &context, Some(lang)).await {
        Ok(player) => {
          let mut json = match from_str::<Value>(&player) {
            Ok(json) => json,
            Err(_) => {
              return Err(FetchPlayerError::FailedToSerializePlayer);
            }
          };
          match json["playabilityStatus"]["status"].as_str() {
            Some(status) => {
              if status == "LOGIN_REQUIRED" {
                return Err(FetchPlayerError::LoginRequired);
              }
              if status == "ERROR" {
                return Err(FetchPlayerError::ResponseUnplayable);
              }
            },
            None => {}
          };
          let mut streams = Vec::<String>::new();
          let empty_vec = Vec::new();
          let formats = match json["streamingData"]["formats"].as_array() {
            Some(formats) => formats,
            None => &empty_vec
          };
          let adaptive_formats = match json["streamingData"]["adaptiveFormats"].as_array() {
            Some(formats) => formats,
            None => &empty_vec
          };
          let need_to_decipher = if formats.len() > 0 {
            match formats[0]["url"].as_str() {
              Some(_) => false,
              None => true
            }
          } else if adaptive_formats.len() > 0 {
            match adaptive_formats[0]["url"].as_str() {
              Some(_) => false,
              None => true
            }
          } else {
            false
          };
          if need_to_decipher {
            for k in 0..formats.len() {
              streams.push(String::from(formats[k]["signatureCipher"].as_str().unwrap()));
            }
            for k in 0..adaptive_formats.len() {
              streams.push(String::from(adaptive_formats[k]["signatureCipher"].as_str().unwrap()));
            }
            let deciphered_streams = if app_settings.decipher_on_video_endpoint {
              match decipher_streams(streams, &player_js_response) {
                Ok(streams) => streams,
                Err(error) => {
                  return Err(FetchPlayerError::FailedToDecipher(error));
                }
              }
            } else {
              streams.into_iter().map(|stream| {
                Some(format!("{}/decipher_stream?signature_cipher={}&player_js_id={}&video_id={}&local={}", hostname, encode(&stream), &player_js_id, &id, local && app_settings.enable_local_streaming))
              }).collect::<Vec::<Option<String>>>()
            };
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
          } else {
            let formats_len = formats.len();
            let adaptive_len = adaptive_formats.len();
            let mut i = 0;
            for k in 0..formats_len {
              let url = json["streamingData"]["formats"][k]["url"].as_str().unwrap_or("");
              let url_parts = url.split("googlevideo.com").collect::<Vec::<&str>>();
              let google_hostname = format!("{}googlevideo.com", url_parts[0]).replace("https://", "");
              let url_after = url_parts[1];
              let url = format!("{}{}&host={}&local={}", hostname, url_after, encode(&google_hostname), local && app_settings.enable_local_streaming);
              json["streamingData"]["formats"][k]["url"] = json!(url);
              i = i + 1;
            }
            for k in 0..adaptive_len {
              let url = json["streamingData"]["adaptiveFormats"][k]["url"].as_str().unwrap_or("");
              let url_parts = url.split("googlevideo.com").collect::<Vec::<&str>>();
              let google_hostname = format!("{}googlevideo.com", url_parts[0]).replace("https://", "");
              let url_after = url_parts[1];
              let url = format!("{}{}&host={}&local={}", hostname, url_after, encode(&google_hostname), local && app_settings.enable_local_streaming);
              json["streamingData"]["adaptiveFormats"][k]["url"] = json!(url);
              i = i + 1;
            }
          }
          json["timestamp"] = Utc::now().timestamp().into();
          if app_settings.cache_requests {
            db.insert_json("player", &format!("{}-{}-{}", id, lang, local), &json).await;
          }
          Ok(json.clone())
        },
        Err(error) => Err(FetchPlayerError::Reqwest(error))
      }
    }
  }
}

const DEFAULT_FIELDS: [&str; 37] = ["type", "title", "videoId", "videoThumbnails", "storyboards", "description", "descriptionHtml", "published", "publishedText", "keywords", "viewCount", "likeCount", "dislikeCount", "paid", "premium", "isFamilyFriendly", "allowedRegions", "genre", "genreUrl", "author", "authorId", "authorUrl", "authorThumbnails", "subCountText", "lengthSeconds", "allowRatings", "rating", "isListed", "liveNow", "isUpcoming", "hlsUrl", "dashUrl", "adaptiveFormats", "formatStreams", "captions", "recommendedVideos", "musicTracks"];

trait AreFieldsInValue {
  fn are_all_fields_in_value(&self, fields: &Vec::<String>) -> bool;
}
impl AreFieldsInValue for Map<String, Value> {
  fn are_all_fields_in_value(&self, fields: &Vec::<String>) -> bool {
    for i in 0..fields.len() {
      if !self.contains_key(&fields[i]) {
        return false;
      }
    }
    true
  }
}

fn filter_out_everything_but_fields(mut json: Map<String, Value>, fields: &Vec::<String>) -> Map<String, Value> {
  let keys = json.keys().into_iter().map(|s| String::from(s)).collect::<Vec::<String>>();
  for i in 0..keys.len() {
    let key = &keys[i];
    if !fields.contains(key) {
      json.remove(key);
    }
  }
  json
}

fn add_in_missing_fields(mut json: Map<String, Value>, fields: &Vec::<String>) -> Map<String, Value> {
  let keys = json.keys().into_iter().map(|s| String::from(s)).collect::<Vec::<String>>();
  for field in fields {
    if !keys.contains(field) {
      json.insert(String::from(field), json!(None::<String>));
    }
  }
  json
}

fn sort_to_inv_schema(json: Map<String, Value>, fields: &Vec::<String>) -> Map<String, Value> {
  let mut output = Map::<String, Value>::new();
  let keys = json.keys().into_iter().map(|s| String::from(s)).collect::<Vec::<String>>();
  for i in 0..fields.len() {
    let field = &fields[i];
    if keys.contains(field) {
      output.insert(String::from(field), json[field].clone());
    }
  }
  output
}

#[derive(Deserialize)]
pub struct VideoEndpointQueryParams {
  hl: Option<String>,
  local: Option<bool>,
  fields: Option<String>,
  pretty: Option<i32>
}

#[derive(Serialize, Deserialize)]
pub struct InnerTubeResponse {
  next: Option<Value>,
  player: Value
}

#[derive(Serialize, Deserialize)]
pub struct CommentUrl {
  title: String,
  url: String,
  token: String
}

#[get("/api/v1/videos/{video_id}")]
pub async fn video_endpoint(req: HttpRequest, path: Path<String>, query: Query<VideoEndpointQueryParams>, app_settings: Data<AppSettings>) -> impl Responder {
  let local = query.local.unwrap_or(false);
  let connection_info = req.connection_info();
  let uri = String::from(format!("{}://{}", connection_info.scheme(), connection_info.host()));
  let video_id = path.into_inner();
  let lang = query.hl.clone().unwrap_or(String::from("en"));
  let is_pretty = match query.pretty {
    Some(pretty) => pretty == 1,
    None => false
  };
  let fields = match &query.fields {
    Some(fields) => {
      if fields.contains(",") {
        fields.split(",").into_iter().map(|field| String::from(field)).collect::<Vec::<String>>()
      } else {
        vec!(String::from(fields))
      }
    },
    None => DEFAULT_FIELDS.into_iter().map(|string| String::from(string)).collect::<Vec::<String>>()
  };
  let player_res = match fetch_player_with_cache(&video_id, &lang, &app_settings, local, Some(&uri)).await {
    Ok(player_res) => player_res,
    Err(fetch_player_error) => {
      let status_code = match fetch_player_error {
        FetchPlayerError::LoginRequired => 403,//🤷‍♀️ this might not be the best response code
        FetchPlayerError::ResponseUnplayable => 404,
        _ => 500
      };
      return HttpResponse::build(StatusCode::from_u16(status_code).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\", \"message\": \"Failed to fetch `player` endpoint\", \"inner_message\": \"{}\" }}", fetch_player_error));
    }
  };
  let mut innertube = InnerTubeResponse {
    next: None,
    player: player_res.clone()
  };
  let mut json = fmt_inv(&player_res, &lang);
  if !json.are_all_fields_in_value(&fields) {
    let Ok(next_res) = fetch_next_with_cache(&video_id, &lang, &app_settings).await else { todo!() };
    json = fmt_inv_with_existing_map(&next_res, &lang, json);
    innertube.next = Some(next_res);
  }
  json = filter_out_everything_but_fields(json, &fields);
  // figure out what thumbnail sizes exist from the streams data we know about
  let mut max_size = 680;
  if fields.contains(&String::from("formatStreams")) && json.contains_key("formatStreams") {
    match json["formatStreams"].as_array() {
      Some(streams) => {
        for stream in streams {
          let width = i32::from_str(stream["size"].as_str().unwrap_or("0").split("x").collect::<Vec::<&str>>()[0]).unwrap_or(0);
          if max_size < width {
            max_size = width;
          }
        }
      },
      None => {}
    }
  }
  if fields.contains(&String::from("adaptiveFormats")) && json.contains_key("adaptiveFormats") {
    match json["adaptiveFormats"].as_array() {
      Some(streams) => {
        for stream in streams {
          let width = i32::from_str(stream["size"].as_str().unwrap_or("0").split("x").collect::<Vec::<&str>>()[0]).unwrap_or(0);
          if max_size < width {
            max_size = width;
          }
        }
      },
      None => {}
    }
  }
  json.insert(String::from("captions"), json!([]));
  // video thumbnails can be generated without making a request
  if !json.contains_key("videoThumbnails") {
    json.insert(String::from("videoThumbnails"), json!(generate_yt_video_thumbnails_within_max_size(&video_id, max_size)));
  }
  // keywords should always be an array an never null
  if !json.contains_key("keywords") {
    json.insert(String::from("keywords"), json!([]));
  }
  // fields that iv keeps, but don't get populated
  if !json.contains_key("rating") {
    json.insert(String::from("rating"), json!(0));
  }
  if !json.contains_key("dislikeCount") {
    json.insert(String::from("dislikeCount"), json!(0));
  }
  if app_settings.retain_null_keys {
    json = add_in_missing_fields(json, &fields);
  }
  // hls url is optionally included
  if json.contains_key("hlsUrl") {
    json.remove(&String::from("hlsUrl"));
  }
  if app_settings.sort_to_inv_schema {
    json = sort_to_inv_schema(json, &fields);
  }
  // TODO integrate comments into fields API
  match &innertube.next.as_ref() {
    Some(next) => {
      json.insert(String::from("comments"), json!(yayti::parsers::web::video::get_comment_continuations(next).unwrap_or(vec!()).into_iter().map(|continuation| {
        CommentUrl {
          title: format!("{}", continuation.title),
          url: format!("/api/v1/comments/{}?continuation={}", video_id, continuation.token),
          token: format!("{}", continuation.token)
        }
      }).collect::<Vec::<CommentUrl>>()));
    },
    None => {}
  }
  if app_settings.return_innertube_response {
    json.insert(String::from("innertube"), json!(innertube));
  }
  let json_response = match if is_pretty {
    to_string_pretty(&json)
  } else {
    to_string(&json)
  } {
    Ok(json_response) => json_response,
    Err(_) => {
      return HttpResponse::build(StatusCode::from_u16(500).unwrap()).content_type("application/json").body("{ \"type\": \"error\", \"message\": \"failed to serialize response\" }");
    }
  };
  HttpResponse::Ok().content_type("application/json").body(json_response)
}

#[get("/vi/{video_id}/{file_name}.jpg")]
pub async fn video_thumbnail_proxy(params: Path<(String, String)>) -> impl Responder {
  let video_id = String::from(&params.0);
  let file_name = String::from(&params.1);
  let client = Client::new();
  match client.get(generate_yt_video_thumbnail_url(&video_id, &file_name)).send().await {
    Ok(thumbnail_response) => {
      HttpResponse::Ok().content_type("image/jpeg").status(thumbnail_response.status()).streaming(thumbnail_response.bytes_stream())
    },
    Err(err) => HttpResponse::build(StatusCode::from_u16(404).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\", \"message\": \"{}\" }}", err))
  }
}

#[derive(Deserialize)]
pub struct LatestVersionQueryParams {
  id: String,
  itag: String,
  local: Option<bool>,
  hl: Option<String>
}

struct Format {
  url: Option<String>
}

#[get("/latest_version")]
pub async fn latest_version(req: HttpRequest, params: Query<LatestVersionQueryParams>, app_settings: Data<AppSettings>) -> impl Responder {
  let video_id = &params.id;
  let connection_info = req.connection_info();
  let uri = String::from(format!("{}://{}", connection_info.scheme(), connection_info.host()));
  let itag = i32::from_str(&params.itag).unwrap_or(0); 
  let lang = params.hl.clone().unwrap_or(String::from("en"));
  let local = &params.local.unwrap_or(false);
  let player_res = match fetch_player_with_cache(&video_id, &lang, &app_settings, *local, Some(&uri)).await {
    Ok(player_res) => player_res,
    Err(error) => {
      return HttpResponse::build(StatusCode::from_u16(500).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\", \"message\": \"Failed to fetch `/player` endpoint\", \"inner_message\": \"{}\" }}", error))
    }
  };
  let legacy_formats = match get_legacy_formats(&player_res) {
    Some(legacy_formats) => legacy_formats,
    None => Vec::new()
  };
  let mut format = None;
  let mut available_itags = Vec::<i32>::new();
  for i in 0..legacy_formats.len() {
    available_itags.push(legacy_formats[i].itag);
    if legacy_formats[i].itag == itag {
      format = Some(Format {
        url: legacy_formats[i].url.clone()
      });
      break;
    }
  }
  let adaptive_formats = match get_adaptive_formats(&player_res) {
    Some(adaptive_formats) => adaptive_formats,
    None => Vec::new()
  };
  for i in 0..adaptive_formats.len() {
    available_itags.push(adaptive_formats[i].itag);
    if adaptive_formats[i].itag == itag {
      format = Some(Format {
        url: adaptive_formats[i].url.clone()
      });
      break;
    }
  }
  match format {
    Some(format_match) => {
      let url = match format_match.url {
        Some(url) => url,
        None => {
          return HttpResponse::build(StatusCode::from_u16(500).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\", \"message\": \"A stream was found matching the given itag: {}, but there was an error returning the url.\" }}", itag));
        }
      };
      HttpResponse::build(StatusCode::from_u16(302).unwrap()).insert_header(("Location", url)).body("")
    },
    None => {
      HttpResponse::build(StatusCode::from_u16(404).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\", \"message\": \"No streams found matching the given itag: {}\", \"available_streams\": [{}] }}", itag, available_itags.into_iter().map(|e| format!("{}", e)).collect::<Vec<String>>().join(",")))
    }
  }
}

#[derive(Deserialize)]
pub struct VideoPlaybackQueryParams {
  host: String,
  local: Option<bool>
}

#[route("/videoplayback", method="GET", method="HEAD")]
pub async fn videoplayback(req: HttpRequest, mut payload: Payload, params: Query<VideoPlaybackQueryParams>, app_settings: Data<AppSettings>) -> impl Responder {
  let local = &params.local.unwrap_or(true);
  if !app_settings.enable_local_streaming && *local {
    return HttpResponse::build(StatusCode::from_u16(403).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\", \"message\": \"Local streaming is disabled.\" }}"));
  }
  let hostname = match decode(&params.host) {
    Ok(hostname) => format!("https://{}", hostname),
    Err(error) => {
      return HttpResponse::build(StatusCode::from_u16(400).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\", \"message\": \"Failed to decode host parameter.\", \"inner_message\": \"{}\" }}", error));
    }
  };
  let uri = format!("{}{}", hostname, req.uri());
  if *local {
    let mut headers = req.get_reqwest_headers();
    headers.remove("referrer");
    headers.remove("access-control-allow-origin");
    headers.remove("connection");
    headers.insert("access-control-allow-origin", "*".try_into().unwrap());
    // ✏ TODO Add proper local streaming
    // this doesn't work very well except for playing videos front to back with no seeking
    /* if headers.contains_key("range") {
      let range = headers.get("range").unwrap().to_str().unwrap();
      println!("{}", range);
      uri = format!("{}&range={}", uri, &range.split("=").collect::<Vec<&str>>()[1]);
      println!("{}", uri);
      headers.remove("range");
    } */
    let (tx, rx) = mpsc::unbounded_channel();
  
    actix_web::rt::spawn(async move {
      while let Some(chunk) = payload.next().await {
        tx.send(chunk).unwrap();
      }
    });

    let client = Client::new();
    let forwarded_req = client
        .request(req.method().into(), uri)
        .body(reqwest::Body::wrap_stream(UnboundedReceiverStream::new(rx)));
  
    let res = forwarded_req
        .send()
        .await
        .unwrap();
  
    let mut client_resp = HttpResponse::build(res.status());
    res.headers().add_headers_to_builder(client_resp).streaming(res.bytes_stream())
  } else {
    HttpResponse::build(StatusCode::from_u16(302).unwrap()).content_type("application/json").insert_header(("Location", uri)).body("{}")
  }
}

#[derive(Deserialize)]
pub struct DecipherStreamQueryParams {
  signature_cipher: String,
  player_js_id: String,
  video_id: String,
  local: Option<bool>
}

#[get("/decipher_stream")]
pub async fn decipher_stream(params: Query<DecipherStreamQueryParams>, app_settings: Data<AppSettings>) -> impl Responder {
  let local = params.local.unwrap_or(false);
  
  if app_settings.decipher_streams {
    let signature_cipher = match decode(&params.signature_cipher) {
      Ok(decoded) => decoded,
      Err(error) => {
        return HttpResponse::build(StatusCode::from_u16(500).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\", \"message\": \"{}\" }}", error))
      }
    };
    // refuse to execute anything that contains " ' ; function for while ( { } ) [ ]
    let input_blacklist = ["\"", "'", ";", "function", "for", "while", "(", "{", "[", "]", "}", ")"];
    for item in input_blacklist {
      if signature_cipher.contains(item) {
        return HttpResponse::build(StatusCode::from_u16(400).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\", \"message\": \"Refusing to execute potentially malicious payload\" }}"))
      }
    }
    let db = app_settings.get_json_db().await;
    let (player_js_res, _, _) = match fetch_player_js_with_cache(&db, &app_settings, Some(String::from(&params.player_js_id))).await {
      Ok(player_js) => player_js,
      Err(error) => {
        return HttpResponse::build(StatusCode::from_u16(500).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\", \"message\": \"Failed to fetch `/player` endpoint\", \"inner_message\": \"{}\" }}", error))
      }
    };
    match ciphers::decipher_stream(&signature_cipher, &player_js_res) {
      Ok(deciphered_url) => {
        let client = Client::new();
        let is_decipher_good = match client.head(&deciphered_url).send().await {
          Ok(response) => response.status() != 403,
          Err(_) => false
        };
        if is_decipher_good {
          let url_parts = deciphered_url.split("googlevideo.com").collect::<Vec::<&str>>();
          let google_hostname = format!("{}googlevideo.com", url_parts[0]).replace("https://", "");;
          let url_after = url_parts[1];
          let url = format!("{}&host={}&local={}", url_after, encode(&google_hostname), local);
          HttpResponse::build(StatusCode::from_u16(302).unwrap()).insert_header(("Location",url)).content_type("application/json").body("")
        } else {
          db.delete("player", &format!("{}-{}-{}", &params.video_id, "en", local)).await;
          HttpResponse::build(StatusCode::from_u16(500).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\", \"message\": \"Deciphering JS ran without fail, but the result was a broken link.\", \"url\": \"{}\" }}", &deciphered_url))
        }
      },
      Err(error) => {
        db.delete("player", &format!("{}-{}-{}", &params.video_id, "en", local)).await;
        HttpResponse::build(StatusCode::from_u16(500).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\", \"message\": \"{}\" }}", error.replace("\"", "\\\"")))
      }
    }
    
  } else {
    HttpResponse::build(StatusCode::from_u16(403).unwrap()).content_type("application/json").body("{ \"type\": \"error\", \"message\": \"Deciphering streams has been disabled.\" }")
  }
}
 