use serde::{Deserialize, Serialize};
use regex::Regex;

#[derive(Deserialize, Serialize, Clone)]
pub struct AppSettings {
  pub decipher_streams: bool,
  pub enable_local_streaming: bool,
  pub enable_cors: bool,
  pub cache_timeout: i32,
  pub cache_requests: bool,
  pub ip_address: String,
  pub port: String,
  pub db_name: String
}

impl AppSettings {
  pub fn from_cli_args(args: &Vec<String>) -> AppSettings {
    let enabled_all_features = args.contains(&String::from("--all-on"));
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
    AppSettings {
      decipher_streams: args.contains(&String::from("--decipher-streams")) || enabled_all_features,
      enable_local_streaming: args.contains(&String::from("--enable-local-streaming")) || enabled_all_features,
      enable_cors: args.contains(&String::from("--enable-cors")) || enabled_all_features,
      cache_timeout: 60,// 1 minute
      cache_requests: !args.contains(&String::from("--no-cache")) && true,
      ip_address: String::from(ip_address),
      port: String::from(port),
      db_name: String::from("yaytapi.db")
    }
  }
}
