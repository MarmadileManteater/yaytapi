use unqlite::{UnQLite, KV, Cursor, Direction::Exact};
use serde_json::{json, from_str,to_string, Value};
use serde::{Serialize, Deserialize};
use mongodb::{Collection};
use mongodb::bson::{doc};
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
            println!("⚠ warning: {}", err);
            None
          }
        }
      },
      None => None
    }
  }
  fn insert_json(&self, key: &str, value: &Value) {
    let Ok(string_value) = to_string(value) else { todo!() };
    self.kv_store(key, string_value);
  }
  fn delete(&self, key: &str) {
    match self.seek(key, Exact) {
      Some(cursor) => {
        cursor.delete();
      },
      None => {
        println!("⚠ warning: attempted to delete non-existent key");
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
  pub mongodb: Option<Collection<Value>>,
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
  pub fn mongodb(mongodb: Collection::<Value>) -> DbWrapper {
    DbWrapper {
      mongodb: Some(mongodb),
      unqlite: None,
      preference: DbType::MongoDb
    }
  }
  pub async fn seek_for_json(&self, key: &str) -> Option<Value> {
    match self.preference {
      DbType::UnQLite => {
        let Some(db) = &self.unqlite else { todo!() };
        db.seek_for_json(key)
      },
      DbType::MongoDb => {
        let Some(db) = &self.mongodb else { todo!() };
        match db.find_one(doc! { "key": key }, None).await {
          Ok(found) => match found {
            Some(found) => Some(json!(found["value"].as_object())),
            None => None
          },
          Err(_) => None
        }
      }
    }
  }
  pub async fn insert_json(&self, key: &str, value: &Value) {
    match self.preference {
      DbType::UnQLite => {
        let Some(db) = &self.unqlite else { todo!() };
        db.insert_json(key, value);
      },
      DbType::MongoDb => {
        let Some(db) = &self.mongodb else { todo!() };
        db.insert_one(json!(JsonKVPair { key: String::from(key), value: value.clone() }), None).await;
      }
    }
  }
  pub async fn delete(&self, key: &str) {
    match self.preference {
      DbType::UnQLite => {
        let Some(db) = &self.unqlite else { todo!() };
        db.delete(key)
      },
      DbType::MongoDb => {
        let Some(db) = &self.mongodb else { todo!() };
        db.delete_one(doc!{ "key": key }, None).await;
      }
    }
  }
}
