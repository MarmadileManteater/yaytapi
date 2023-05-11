use serde::{Deserialize, Serialize};
use regex::Regex;
use unqlite::UnQLite;
use mongodb::{Client, options::ClientOptions};
use crate::helpers::DbWrapper;

#[derive(Deserialize, Serialize, Clone)]
pub enum DbType {
  UnQLite,
  MongoDb
}

#[derive(Deserialize, Serialize, Clone)]
pub struct AppSettings {
  pub decipher_streams: bool,
  pub enable_local_streaming: bool,
  pub enable_cors: bool,
  pub cache_timeout: i32,
  pub cache_requests: bool,
  pub ip_address: String,
  pub port: String,
  pub db_connection_string: Option<String>,
  pub db_name: String,
  pub db_type: DbType
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
    let Ok(db_type_re) = Regex::new(r#"--mongo-db=([^ ]+)"#) else { todo!() };
    let (db_type, db_connection_string) = match db_type_re.captures(&args_string) {
      Some(db_type_captures) => {
        (DbType::MongoDb, Some(String::from(db_type_captures.get(1).unwrap().as_str())))
      },
      None => (DbType::UnQLite, None)
    };
    let Ok(db_name_re) = Regex::new(r#"--db-name=([^ ]+)"#) else { todo!() };
    let db_name = match db_name_re.captures(&args_string) {
      Some(db_name_captures) => db_name_captures.get(1).unwrap().as_str(),
      None => match db_type {
        DbType::UnQLite => "yaytapi.db",
        DbType::MongoDb => "local"
      }
    };
    AppSettings {
      decipher_streams: args.contains(&String::from("--decipher-streams")) || enabled_all_features,
      enable_local_streaming: args.contains(&String::from("--enable-local-streaming")) || enabled_all_features,
      enable_cors: args.contains(&String::from("--enable-cors")) || enabled_all_features,
      cache_timeout: 60,// 1 minute
      cache_requests: !args.contains(&String::from("--no-cache")) && true,
      ip_address: String::from(ip_address),
      port: String::from(port),
      db_connection_string: db_connection_string,
      db_name: String::from(db_name),
      db_type: db_type
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
