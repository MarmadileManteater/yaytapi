use serde::{Deserialize, Serialize};
use regex::Regex;
use unqlite::UnQLite;
use mongodb::{Client, options::ClientOptions};
use std::str::FromStr;
use crate::helpers::DbWrapper;

#[derive(Deserialize, Serialize, Clone)]
pub enum DbType {
  UnQLite,
  MongoDb
}

#[derive(Deserialize, Serialize, Clone)]
pub struct AppSettings {
  pub publish_settings_inside_stats: bool,
  // --print-config
  pub print_config: bool, 
  // always enabled unless explicitly disabled by
  // --no-logs
  pub enable_actix_web_logger: bool,
  // Sorts the output of yayti to the order of invidious
  // DEFAULTS: true
  // can be disabled with `--no-sort`
  pub sort_to_inv_schema: bool,
  // Whether or not to return every key in each schema or to return only the non-null keys
  // DEFAULTS: true
  // can be disabled with `--hide-null-fields`
  pub retain_null_keys: bool,
  // Optionally include the innertube response used to parse out the data (useful for debugging & development)
  // DEFAULTS: false
  // can be enabled with `--return-innertube`
  pub return_innertube_response: bool,
  // Optionally use the android `/player` endpoint to bypass the need to decipher (similar to how invidious works)
  // DEFAULTS: false
  // can be enabled with `--use-android-endpoint`
  pub use_android_endpoint_for_streams: bool,
  pub decipher_streams: bool,
  // Optionally pre-decipher all streams in every video endpoint response instead of lazily providing a link to `/decipher_stream`
  pub decipher_on_video_endpoint: bool,
  pub enable_local_streaming: bool, // 📝 UNIMPLEMENTED
  pub enable_cors: bool, // 📝 UNIMPLEMENTED
  pub cache_timeout: i32,
  pub cache_requests: bool,
  // can be set with `--ip-address=127.0.0.1`
  pub ip_address: String,
  // can be set with `--port=8080`
  pub port: String,
  // the public facing URL for the server (what will be returned in `formatStream` urls and the like)
  // can be set with `--public-url=https://yaytapi.marmadilemanteater.dev`
  pub pub_url: Option<String>,
  pub db_connection_string: Option<String>,
  pub db_name: String,
  pub db_type: DbType,
  // can be set with `--workers=[0-9]+`
  pub num_of_workers: usize,
  // a directory that contains playlist definitions in json format
  // each playlist file must contain an array of video urls or video ids
  // it will sus out the video ids from any valid yt, iv, or yaytapi video link
  // ex: 
  // [
  //  "https://youtube.com/watch?v=PxeFyxrUWt0",
  //  "khBwYuNGU6U"
  // ]
  // can be set with `--playlists-path=/path/to/playlists/`
  pub playlists_path: Option<String>
}

impl AppSettings {
  pub fn from_cli_args(args: &Vec<String>) -> AppSettings {
    let args_string = args.join(" ");
    let Ok(ip_re) = Regex::new(r#"--ip=([0-9]+\.[0-9]+\.[0-9]+\.[0-9]+)"#) else { todo!() };
    let ip_address = match ip_re.captures(&args_string) {
      Some(ip_address_captures) => ip_address_captures.get(1).unwrap().as_str(),
      None => "127.0.0.1"
    };
    let Ok(port_re) = Regex::new(r#"--port=([0-9]+)"#) else { todo!() };
    let port = match port_re.captures(&args_string) {
      Some(port_captures) => port_captures.get(1).unwrap().as_str(),
      None => "8080"
    };
    let Ok(db_type_re) = Regex::new(r#"--mongo-db=([^ ]+)"#) else { todo!() };
    let (db_type, db_connection_string) = match db_type_re.captures(&args_string) {
      Some(db_type_captures) => {
        (DbType::MongoDb, Some(String::from(db_type_captures.get(1).unwrap().as_str())))
      },
      None => (DbType::UnQLite, None)
    };
    let Ok(public_url_re) = Regex::new(r#"--public-url=([^ ]+)"#) else { todo!() };
    let public_url = match public_url_re.captures(&args_string) {
      Some(public_url_re_captures) => {
        Some(String::from(public_url_re_captures.get(1).unwrap().as_str()))
      },
      None => None
    };
    let Ok(db_name_re) = Regex::new(r#"--db-name=([^ ]+)"#) else { todo!() };
    let db_name = match db_name_re.captures(&args_string) {
      Some(db_name_captures) => db_name_captures.get(1).unwrap().as_str(),
      None => match db_type {
        DbType::UnQLite => "yaytapi.db",
        DbType::MongoDb => "local"
      }
    };
    let Ok(num_of_workers_re) = Regex::new(r#"--workers=([0-9]+)"#) else { todo!() };
    let num_of_workers = match num_of_workers_re.captures(&args_string) {
      Some(db_name_captures) => i32::from_str(db_name_captures.get(1).unwrap().as_str()).unwrap_or(1) as usize,
      None => 1
    };
    let Ok(playlists_dir_re) = Regex::new(r#"--playlists-path=([^ ]+)"#) else { todo!() };
    let playlist_dir = match playlists_dir_re.captures(&args_string) {
      Some(playlist_dir_captures) => {
        Some(String::from(playlist_dir_captures.get(1).unwrap().as_str()))
      },
      None => None
    };
    AppSettings {
      publish_settings_inside_stats: args.contains(&String::from("--publish-settings")),
      print_config: args.contains(&String::from("--print-config")),
      enable_actix_web_logger: !args.contains(&String::from("--no-logs")),
      sort_to_inv_schema: !args.contains(&String::from("--no-sort")),
      retain_null_keys: !args.contains(&String::from("--hide-null-fields")),
      return_innertube_response: args.contains(&String::from("--return-innertube")),
      use_android_endpoint_for_streams: args.contains(&String::from("--use-android-endpoint")),
      decipher_streams: args.contains(&String::from("--decipher-streams")),
      decipher_on_video_endpoint: args.contains(&String::from("--pre-decipher-streams")),
      enable_local_streaming: args.contains(&String::from("--enable-local-streaming")),
      enable_cors: args.contains(&String::from("--enable-cors")),
      cache_timeout: 60,// 1 minute
      cache_requests: !args.contains(&String::from("--no-cache")),
      ip_address: String::from(ip_address),
      port: String::from(port),
      pub_url: public_url,
      db_connection_string: db_connection_string,
      db_name: String::from(db_name),
      db_type: db_type,
      num_of_workers: num_of_workers,
      playlists_path: playlist_dir
    }
  }
  pub async fn get_json_db(&self) -> DbWrapper {
    match self.db_type {
      DbType::UnQLite => {
        DbWrapper::unqlite(UnQLite::create(&self.db_name))
      },
      DbType::MongoDb => {
        // Parse a connection string into an options struct.
        let client_options = ClientOptions::parse(self.db_connection_string.as_ref().unwrap()).await.unwrap();
        // Get a handle to the deployment.
        let client = Client::with_options(client_options).unwrap();
        let db = client.database(&self.db_name);
        DbWrapper::mongodb(db)
      }
    }
  }
}
