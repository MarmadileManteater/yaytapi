
pub mod video;
pub mod channel;
pub mod playlist;
use std::fs;
use serde_json::from_str;
use serde::{Serialize, Deserialize};
use serde_json::{to_string_pretty, to_string};
use actix_web::web::{Query, Data, Path};
use actix_web::{HttpResponse, Responder, get, HttpRequest, routes};
use actix_web::http::StatusCode;
use crate::settings::AppSettings;

#[derive(Serialize, Deserialize)]
pub enum InnertubeEndpoint {
  Web,
  Android,
  TV
}

#[derive(Serialize, Deserialize)]
pub struct YaytAPIStats {
  cors_enabled: bool,
  local_streaming_enabled: bool,
  decipher_streams_enabled: bool,
  innertube_endpoints_used: Vec::<InnertubeEndpoint>,
  allow_null_keys_in_output: bool
}


#[derive(Serialize, Deserialize)]
pub struct Software {
  name: String,
  version: String,
  branch: String
}

#[derive(Serialize, Deserialize)]
pub struct Stats {
  version: String,
  software: Software,
  // TODO ✏ add the rest of the schema from https://docs.invidious.io/api/#get-apiv1stats
  yaytapi_settings: Option<YaytAPIStats>
}

#[derive(Serialize, Deserialize)]
pub struct StatsQueryParams {
  pretty: Option<i32>
}
#[derive(Deserialize)]
pub struct GitInfo {
  commit: String,
  branch: String
}

#[get("/api/v1/stats")]
pub async fn server_stats(params: Query<StatsQueryParams>, app_settings: Data<AppSettings>) -> impl Responder {
  let git_info = from_str::<GitInfo>(include_str!("../git-info.json")).unwrap(); // this is should never be a problem after compilation
  
  let pretty = params.pretty;
  let is_pretty = match pretty {
    Some(pretty) => pretty == 1,
    None => false
  };
  let stats = Stats {
    version: String::from("0.2.0"),
    software: Software {
      name: String::from("yaytapi"),
      version: git_info.commit,
      branch: git_info.branch
    },
    yaytapi_settings: if app_settings.publish_settings_inside_stats { 
      Some(YaytAPIStats {
        cors_enabled: app_settings.enable_cors,
        local_streaming_enabled: app_settings.enable_local_streaming,
        decipher_streams_enabled: app_settings.decipher_streams,
        innertube_endpoints_used: {
          let mut endpoints = vec!(InnertubeEndpoint::Web);
          if app_settings.use_android_endpoint_for_streams {
            endpoints.push(InnertubeEndpoint::Android);
          }
          endpoints
        },
        allow_null_keys_in_output: app_settings.retain_null_keys
      })
    } else {
      None
    }
  };
  let json_response = match if is_pretty {
    to_string_pretty(&stats)
  } else {
    to_string(&stats)
  } {
    Ok(json_response) => json_response,
    Err(_) => {
      return HttpResponse::build(StatusCode::from_u16(500).unwrap()).content_type("application/json").body("{ \"type\": \"error\", \"message\": \"failed to serialize response\" }");
    }
  };
  HttpResponse::Ok().content_type("application/json").body(json_response)
}

#[get("/static/{path:.*}")]
pub async fn static_files(params: Path<String>) -> impl Responder {
  let path = params.into_inner();
  let full_path = format!("./static/{}", path);
  match fs::read(&full_path).map(|bytes| {
    let content_type = match mime_guess::from_path(&full_path).first() {
      Some(value) => format!("{}", value),
      None => String::from("text/html")
    };
    HttpResponse::build(StatusCode::from_u16(200).unwrap()).content_type(content_type).body(bytes)
  }).map_err(|error| {
    HttpResponse::build(StatusCode::from_u16(500).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\", \"message\": \"{}\" }}", error))
  }) {
    Ok(response) => response,
    Err(response) => response
  }
}

#[get("/{path:.*}")]
pub async fn not_found(params: Path<String>) -> impl Responder {
  let path = params.into_inner();
  HttpResponse::build(StatusCode::from_u16(404).unwrap()).content_type("application/json").body(format!("{{ \"type\": \"error\", \"message\": \"The requested path '/{}' was not found on this server.\" }}", path))
}

#[routes]
#[get("/")]
#[get("/watch")]
#[get("/playlist")]
pub async fn homepage() -> impl Responder {
  HttpResponse::build(StatusCode::from_u16(200).unwrap()).content_type("text/html").body(actix_web::web::Bytes::from(include_bytes!("../static/home.html").into_iter().map(|u| u.to_owned()).collect::<Vec::<u8>>()))
}
