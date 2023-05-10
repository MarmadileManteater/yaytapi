
mod settings;
mod routes;
mod helpers;
use serde_json::to_string_pretty;
use settings::AppSettings;
use actix_web::{HttpServer, App, web::Data};
use std::io::Result;

use unqlite::{UnQLite};

#[tokio::main]
async fn main() -> Result<()> {
  let args: Vec<String> = std::env::args().collect();
  let app_settings = AppSettings::from_cli_args(&args);
  let Ok(app_settings_str) = to_string_pretty(&app_settings) else { todo!() };
  println!("{}", app_settings_str);
  let ip_address = String::from(&app_settings.ip_address);
  let port = String::from(&app_settings.port);
  HttpServer::new(move || {
    let app_settings = (&app_settings).clone();
    App::new()
      .app_data(Data::new(app_settings))
      .service(routes::video::video_endpoint)// -> /api/v1/videos/{video_id}
  }).bind(format!("{}:{}", ip_address, port))?
    .run()
    .await
}
