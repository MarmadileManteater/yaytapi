
use std::{process::Command, fs::File};
use std::io::Write;

fn main() {
  let branch_name = format!("{}", String::from_utf8_lossy(Command::new("git")
    .arg("rev-parse")
    .arg("--abbrev-ref")
    .arg("HEAD")
    .output()
    .expect("failed to get git branch")
    .stdout.as_slice()).replace("\n", ""));
  let commit = format!("{}", String::from_utf8_lossy(Command::new("git")
  .arg("rev-parse")
  .arg("HEAD")
  .output()
  .expect("failed to get commit")
  .stdout.as_slice()).replace("\n", ""));
  let mut git_info = File::create("./git-info.json").unwrap();
  write!(git_info, "{}", format!("{{ \"commit\": \"{}\", \"branch\": \"{}\" }}", &commit, &branch_name))
    .expect("failed to store git info for build");
  if cfg!(target_os = "windows") {
    let mut res = winres::WindowsResource::new();
    res.set_icon("static/icon.ico");
    res.compile().unwrap();
  }
}
