
mod settings;
mod routes;
mod helpers;
mod local;
use local::local_playlist_to_iv;
use serde_json::{to_string_pretty, from_str, Value, json};
use settings::AppSettings;
use actix_web::{HttpServer, App, web::Data};
use std::{io::Result, fs};
use actix_web::middleware::Logger;
use actix_cors::Cors;
use env_logger::{Env, init_from_env};
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

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
  match &app_settings.playlists_path {
    Some(path) => {
      log::info!("Loading custom playlists . . .");
      let directory_paths = std::fs::read_dir(path).map(|res| {
        let mut directory: Vec::<String> = vec![];
        for entry in res {
          match entry {
            Ok(entry) => {
              match entry.file_name().into_string() {
                Ok(str) => directory.push(str),
                Err(_) => {}
              }
            },
            Err(_) => {

            }
          }
        }
        directory
      });
      for file_name in directory_paths.unwrap() {
        if file_name.ends_with(".json") {
          // only scan json files directly in the top level
          let local_uri = format!("{}/{}", path, file_name);
          match fs::read(&local_uri).map(|res| {
            format!("{}", String::from_utf8_lossy(res.as_ref()))
          }) {
            Ok(file_contents) => {
              match from_str::<Value>(&file_contents) {
                Ok(resulting_json) => {
                  let db = &app_settings.get_json_db().await;
                  match db.seek_for_json("local-playlist", &file_name).await {
                    Some(_) => {
                      log::info!("Skipping because already loaded into db: {}", &local_uri);
                    },
                    None => {
                      let playlist = local_playlist_to_iv(&file_name, &resulting_json, &app_settings).await;
                      // println!("{:#?}", playlist.map(|p| p.into_inv()));
                      let json = json!(playlist.map(|p| p.into_inv()));
                      db.insert_json("local-playlist", &file_name, &json).await;
                      log::info!("Loaded: {}", &local_uri);
                    }
                  }
                },
                Err(error) => {
                  log::error!("{}", error)
                }
              }
            },
            Err(error) => {
              log::error!("{}", error)
            }
          }
          
        }
      }
    },
    None => {}
  };
  let workers = app_settings.num_of_workers;
  HttpServer::new(move || {
    let enable_cors = app_settings.enable_cors;
    let app_settings = (&app_settings).clone();
    App::new()
      .wrap(Logger::default())
      .app_data(Data::new(app_settings))
      .service(routes::server_stats)// -> /api/v1/stats
      .service(routes::video::latest_version)// -> /latest_version
      .service(routes::video::videoplayback)// -> /videoplayback
      .service(routes::video::decipher_stream)// -> /decipher_stream
      .service(routes::video::video_endpoint)// -> /api/v1/videos/{video_id}
      .service(routes::video::video_thumbnail_proxy)// -> /vi/{video_id}/{file_name}.jpg
      .service(routes::channel::author_thumbnail_proxy)// -> /ggpht/{author_thumbnail_url:.*}
      .service(routes::playlist::playlist_endpoint)
      .service(routes::static_files)
      .wrap(
        if enable_cors {
          Cors::permissive()
        } else {
          Cors::default()
        }
      )
  }).bind(format!("{}:{}", ip_address, port))?
  .workers(workers)
  .run()
  .await
}
