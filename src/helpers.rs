use unqlite::{UnQLite, KV, Cursor, Direction::Exact};
use serde_json::{json, from_str,to_string, Value};
use serde::{Serialize, Deserialize};
use mongodb::{Database};
use mongodb::bson::{doc};
use log::{warn,error};
use crate::settings::DbType;

pub trait JsonDb {
  fn seek_for_json(&self, key: &str) -> Option<Value>;
  fn insert_json(&self, key: &str, value: &Value);
  fn delete(&self, key: &str);
}

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
  pub unqlite: Option<UnQLite>,
  pub preference: DbType
}

impl DbWrapper {
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
  pub async fn seek_for_json(&self, collection_name: &str, key: &str) -> Option<Value> {
    match self.preference {
      DbType::UnQLite => {
        let Some(db) = &self.unqlite else { todo!() };
        db.seek_for_json(&format!("{}-{}", collection_name, key))
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
      }
    }
  }
  pub async fn insert_json(&self, collection_name: &str, key: &str, value: &Value) {
    match self.preference {
      DbType::UnQLite => {
        let Some(db) = &self.unqlite else { todo!() };
        db.insert_json(&format!("{}-{}", collection_name, key), value);
      },
      DbType::MongoDb => {
        let Some(db) = &self.mongodb else { todo!() };
        let collection = db.collection::<Value>(&format!("yayti.{}", collection_name));
        match collection.insert_one(json!(JsonKVPair { key: String::from(key), value: value.clone() }), None).await {
          Ok(_) => {},
          Err(error) => error!("❌ insert_json failed: {}", error)
        }
      }
    }
  }
  pub async fn delete(&self, collection_name: &str, key: &str) {
    match self.preference {
      DbType::UnQLite => {
        let Some(db) = &self.unqlite else { todo!() };
        db.delete(&format!("{}-{}", collection_name, key))
      },
      DbType::MongoDb => {
        let Some(db) = &self.mongodb else { todo!() };
        let collection = db.collection::<Value>(&format!("yayti.{}", collection_name));
        match collection.delete_one(doc!{ "key": key }, None).await {
          Ok(_) => {},
          Err(error) => error!("❌ delete failed: {}", error)
        }
      }
    }
  }
}
