
pub mod video;
pub mod comment;
pub mod channel;
use serde::{Serialize, Deserialize};
use serde_json::{to_string_pretty, to_string};
use actix_web::web::{Query, Data};
use actix_web::{HttpResponse, Responder, get};
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

#[get("/api/v1/stats")]
pub async fn server_stats(params: Query<StatsQueryParams>, app_settings: Data<AppSettings>) -> impl Responder {
  let pretty = params.pretty;
  let is_pretty = match pretty {
    Some(pretty) => pretty == 1,
    None => false
  };
  
  let stats = Stats {
    version: String::from("0.2.0"),
    software: Software {
      name: String::from("yaytapi"),
      version: String::from("0.2.0"),
      branch: String::from("development")
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

