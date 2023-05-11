

use actix_web::web::{Path};
use actix_web::{HttpResponse, Responder, get};
use reqwest::Client;
use substring::Substring;

#[get("/ggpht/{author_thumbnail_url:.*}")]
pub async fn author_thumbnail_proxy(params: Path<String>) -> impl Responder {
  let mut author_thumbnail_url = String::from(&params.into_inner());
  // Remove any starting slashes because ft sometimes adds an extra slash, and yt3.ggpht.com doesn't like that.
  while author_thumbnail_url.starts_with("/") {
    author_thumbnail_url = String::from(author_thumbnail_url.substring(1, author_thumbnail_url.len())); 
  }
  let client = Client::new();
  match client.get(format!("https://yt3.ggpht.com/{}", author_thumbnail_url)).send().await {
    Ok(thumbnail_response) => {
      HttpResponse::Ok().content_type(thumbnail_response.headers()["Content-Type"].to_str().unwrap()).streaming(thumbnail_response.bytes_stream())
    },
    Err(_err) => HttpResponse::Ok().body("error")
  }
}
