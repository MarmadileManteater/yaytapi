
mod settings;
mod routes;
mod helpers;
use serde_json::to_string_pretty;
use settings::AppSettings;
use actix_web::{HttpServer, App, web::Data};
use std::io::Result;
use actix_web::middleware::Logger;
use env_logger::{Env, init_from_env};

#[tokio::main]
async fn main() -> Result<()> {
  let args: Vec<String> = std::env::args().collect();
  let app_settings = AppSettings::from_cli_args(&args);
  let Ok(app_settings_str) = to_string_pretty(&app_settings) else { todo!() };
  if app_settings.print_config {
    println!("config:");
    println!("{}", app_settings_str);
  } else {
    println!("Listening on: {}:{}", &app_settings.ip_address, &app_settings.port)
  }
  let ip_address = String::from(&app_settings.ip_address);
  let port = String::from(&app_settings.port);
  if app_settings.enable_actix_web_logger {
    init_from_env(Env::default().default_filter_or("info"));
  }
  HttpServer::new(move || {
    let app_settings = (&app_settings).clone();
    App::new()
      .wrap(Logger::default())
      .app_data(Data::new(app_settings))
      .service(routes::server_stats)// -> /api/v1/stats
      .service(routes::video::latest_version)// -> /latest_version
      .service(routes::video::video_endpoint)// -> /api/v1/videos/{video_id}
      .service(routes::video::video_thumbnail_proxy)// -> /vi/{video_id}/{file_name}.jpg
      .service(routes::channel::author_thumbnail_proxy)// -> /ggpht/{author_thumbnail_url:.*}
  }).bind(format!("{}:{}", ip_address, port))?
    .run()
    .await
}
