#[cfg(feature = "unqlite")]
use unqlite::{UnQLite, KV, Cursor, Direction::Exact};
use serde_json::{json, from_str,to_string, Value};
use serde::{Serialize, Deserialize};
use mongodb::{Database};
use mongodb::bson::{doc};
use log::{warn,error};
use chrono::Utc;
use reqwest::header::{HeaderMap};
use actix_web::{HttpResponseBuilder, HttpRequest};
use crate::settings::{AppSettings, DbType};

pub trait JsonDb {
  fn seek_for_json(&self, key: &str) -> Option<Value>;
  fn insert_json(&self, key: &str, value: &Value);
  fn delete(&self, key: &str);
}
#[cfg(feature = "unqlite")]
impl JsonDb for UnQLite {
  fn seek_for_json(&self, key: &str) -> Option<Value> {
    match self.seek(key, Exact) {
      Some(cursor) => {
        let value = cursor.value();
        let json_string = String::from_utf8_lossy(&value);
        match from_str::<Value>(&json_string) {
          Ok(obj) => Some(obj),
          Err(err) => {
            warn!("seek for json failed: {}", err);
            None
          }
        }
      },
      None => None
    }
  }
  fn insert_json(&self, key: &str, value: &Value) {
    let Ok(string_value) = to_string(value) else { todo!() };
    match self.kv_store(key, string_value) {
      Ok(_) => {},
      Err(error) => error!("insert_json failed: {}", error)
    }
  }
  fn delete(&self, key: &str) {
    #[cfg(feature = "unqlite")]
    match self.seek(key, Exact) {
      Some(cursor) => {
        cursor.delete();
      },
      None => {
        warn!("attempted to delete non-existent key");
      }
    };
  }
}

#[derive(Serialize, Deserialize)]
pub struct JsonKVPair {
  pub key: String,
  pub value: Value
}

pub struct DbWrapper {
  pub mongodb: Option<Database>,
  #[cfg(feature = "unqlite")]
  pub unqlite: Option<UnQLite>,
  pub unqlite: Option<String>,
  pub preference: DbType
}

// todo✏ figure out better solution for concurency
impl DbWrapper {
  pub fn none() -> DbWrapper {
    DbWrapper { mongodb: None, unqlite: None, preference: DbType::None }
  }
  #[cfg(feature = "unqlite")]
  pub fn unqlite(unqlite: UnQLite) -> DbWrapper {
    DbWrapper {
      mongodb: None,
      unqlite: Some(unqlite),
      preference: DbType::UnQLite
    }
  }
  pub fn mongodb(mongodb: Database) -> DbWrapper {
    DbWrapper {
      mongodb: Some(mongodb),
      unqlite: None,
      preference: DbType::MongoDb
    }
  }
  // TODO ✏ add better error handling for timeouts per https://docs.rs/mongodb/latest/mongodb/#warning-about-timeouts--cancellation
  pub async fn seek_for_json(&self, collection_name: &str, key: &str) -> Option<Value> {
    match self.preference {
      DbType::UnQLite => {
        let Some(db) = &self.unqlite else { todo!() };
        #[cfg(feature = "unqlite")]
        return db.seek_for_json(&format!("{}-{}", collection_name, key));
        None
      },
      DbType::MongoDb => {
        let Some(db) = &self.mongodb else { todo!() };
        let collection = db.collection::<Value>(&format!("yayti.{}", collection_name));
        match collection.find_one(doc! { "key": key }, None).await {
          Ok(found) => match found {
            Some(found) => Some(json!(found["value"])),
            None => None
          },
          Err(_) => None
        }
      },
      DbType::None => None
    }
  }
  pub async fn insert_json(&self, collection_name: &str, key: &str, value: &Value) {
    match self.preference {
      DbType::UnQLite => {
        let Some(db) = &self.unqlite else { todo!() };
        #[cfg(feature = "unqlite")]
        db.insert_json(&format!("{}-{}", collection_name, key), value);
      },
      DbType::MongoDb => {
        let Some(db) = &self.mongodb else { todo!() };
        let collection = db.collection::<Value>(&format!("yayti.{}", collection_name));
        match collection.insert_one(json!(JsonKVPair { key: String::from(key), value: value.clone() }), None).await {
          Ok(_) => {},
          Err(error) => error!("❌ insert_json failed: {}", error)
        }
      },
      DbType::None => {}
    }
  }
  pub async fn delete(&self, collection_name: &str, key: &str) {
    match self.preference {
      DbType::UnQLite => {
        let Some(db) = &self.unqlite else { todo!() };
        #[cfg(feature = "unqlite")]
        db.delete(&format!("{}-{}", collection_name, key))
      },
      DbType::MongoDb => {
        let Some(db) = &self.mongodb else { todo!() };
        let collection = db.collection::<Value>(&format!("yayti.{}", collection_name));
        match collection.delete_one(doc!{ "key": key }, None).await {
          Ok(_) => {},
          Err(error) => error!("❌ delete failed: {}", error)
        }
      },
      DbType::None => {}
    }
  }
}


pub async fn get_previous_data(collection: &str, key: &str, db: &DbWrapper, app_settings: &AppSettings) -> Option<Value> {
  if app_settings.cache_requests {
    match db.seek_for_json(collection, key).await {
      Some(json) => {
        match json["timestamp"].as_i64() {
          Some(timestamp) => {
            let current_timestamp = Utc::now().timestamp();
            let offset = current_timestamp - timestamp;
            if offset as u64 > app_settings.cache_timeout {
              db.delete(collection, key).await;
              None
            } else {
              Some(json)
            }
          },
          None => Some(json)
        }
      },
      None => None
    }
  } else {
    None
  }
 }
pub trait ActixHeadersIntoReqwest {
  fn get_reqwest_headers(&self) -> HeaderMap;
}

impl ActixHeadersIntoReqwest for HttpRequest {
  fn get_reqwest_headers(&self) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for (header_name, header_value) in self.headers().into_iter() {
      headers.insert(header_name, header_value.into());
    };
    headers
  }
}
 
pub trait ReqwestHeadersIntoResponseBuilder {
  fn add_headers_to_builder(&self, builder: HttpResponseBuilder) -> HttpResponseBuilder;
}

impl ReqwestHeadersIntoResponseBuilder for HeaderMap {
  fn add_headers_to_builder(&self, mut builder: HttpResponseBuilder) -> HttpResponseBuilder {
    for (header_name, header_value) in self {
      if header_name != "content-length" {
        builder.insert_header((header_name, header_value.to_str().unwrap()));
      }
    }
    builder
  }
}
