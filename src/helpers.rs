use unqlite::{UnQLite, KV, Cursor, Direction::Exact};
use serde_json::{from_str,to_string};
use serde::{Serialize, Deserialize};

pub trait JsonDb {
  fn seek_for_json<T: for<'a> Deserialize<'a>>(&self, key: &str) -> Option<T>;
  fn insert_json<T: Serialize>(&self, key: &str, value: &T);
  fn delete(&self, key: &str);
}

impl JsonDb for UnQLite {
  fn seek_for_json<T: for<'a> Deserialize<'a>>(&self, key: &str) -> Option<T> {
    match self.seek(key, Exact) {
      Some(cursor) => {
        let value = cursor.value();
        let json_string = String::from_utf8_lossy(&value);
        match from_str::<T>(&json_string) {
          Ok(obj) => Some(obj),
          Err(_) => None
        }
      },
      None => None
    }
  }
  fn insert_json<T: Serialize>(&self, key: &str, value: &T) {
    let Ok(string_value) = to_string(value) else { todo!() };
    self.kv_store(key, string_value);
  }
  fn delete(&self, key: &str) {
    match self.seek(key, Exact) {
      Some(cursor) => {
        cursor.delete();
      },
      None => {}
    };
  }
}
