
pub mod video;
pub mod channel;
use serde::{Serialize, Deserialize};
use serde_json::{to_string_pretty, to_string};
use actix_web::web::{Query};
use actix_web::{HttpResponse, Responder, get};
use actix_web::http::StatusCode;

#[derive(Serialize, Deserialize)]
pub struct Software {
  name: String,
  version: String,
  branch: String
}

#[derive(Serialize, Deserialize)]
pub struct Stats {
  version: String,
  software: Software
  // TODO ✏ add the rest of the schema from https://docs.invidious.io/api/#get-apiv1stats
}

#[derive(Serialize, Deserialize)]
pub struct StatsQueryParams {
  pretty: Option<i32>
}

#[get("/api/v1/stats")]
pub async fn server_stats(params: Query<StatsQueryParams>) -> impl Responder {
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
