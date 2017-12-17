extern crate chrono;
extern crate clap;
extern crate ini;
extern crate xdg;

use ini::Ini;
use clap::{Arg, App};
use chrono::prelude::*;
use std::fs;
use std::io;
use std::iter::Iterator;
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Debug)]
struct TrashInfo {
    info_file: PathBuf,
    trashed_file: PathBuf,
    original_path: PathBuf,
    deletion_date: DateTime<Local>,
}

fn info_to_file(info_file: &Path) -> Result<PathBuf, String> {
    let trash_dir = match info_file.parent() {
        Some(info_dir) => {
            match info_dir.parent() {
                Some(t) => t,
                None => {return Err("Couldn't go up to trash dir".to_string())}
            }
        },
        None => {return Err("Couldn't go up to trash info dir".to_string())}
    };
    let mut res = PathBuf::from(trash_dir);
    res.push("files");
    match info_file.file_stem() {
        Some(n) => res.push(n),
        None => {return Err("No trash info file name".to_string())}
    };
    Ok(res)
}

impl TrashInfo {
    fn from_info_file(info_file: &Path) -> Result<TrashInfo, String> {
        let info = Ini::load_from_file(info_file).map_err(|err| err.to_string())?;
        let sec = match info.section(Some("Trash Info")) {
            Some(s) => s,
            None => {return Err("No [Trash Info] section".to_string());}
        };
        let orig_path = match sec.get("Path") {
            Some(p) => p,
            None => {return Err("No Path key".to_string());}
        };
        let deletion_date = match sec.get("DeletionDate") {
            Some(date_str) => Local.datetime_from_str(date_str, "%Y-%m-%dT%H:%M:%S").map_err(|err| err.to_string())?,
            None => {return Err("No DeletionDate key".to_string());}
        };
        return Ok(TrashInfo{
            info_file: PathBuf::from(info_file),
            trashed_file: info_to_file(info_file)?,
            original_path: PathBuf::from(orig_path),
            deletion_date: deletion_date,
        })
    }
    
    fn delete(self) -> Result<(), io::Error> {
        fs::remove_file(self.trashed_file)?;
        fs::remove_file(self.info_file)?;
        Ok(())
    }
}

#[derive(Debug)]
struct Config {
    delete_after_days: i64,
    warn_after_days: i64,
}

fn find_config_files<P>(relpath: P) -> Vec<PathBuf> where P:AsRef<Path> {
    let basedirs = xdg::BaseDirectories::new().unwrap();
    // Ordered from least preferred to most preferred
    let config_home = [basedirs.get_config_home()];
    let extra_config_dirs = basedirs.get_config_dirs();
    let config_dirs = config_home.iter().chain(extra_config_dirs.iter().rev());
    let mut files = Vec::new();
    for dir in config_dirs {
        let file = dir.join(&relpath);
        if file.is_file() {
            files.push(file);
        }
    }
    files
}

impl Config {
    fn default() -> Config {
        return Config{
            delete_after_days: 60,
            warn_after_days: 50,
        };
    }
    
    fn load() -> Config {
        let mut cfg = Config::default();
        for config_file in find_config_files("trashexpiry.ini") {
            println!("Loading config from {:?}", config_file);
            let ini = match Ini::load_from_file(config_file) {
                Ok(data) => data,
                Err(e) => {
                    println!("Error reading config file: {}", e);
                    continue;
                }
            };
            
            if let Some(s) = ini.get_from::<String>(None, "delete_after_days") {
                match i64::from_str(s) {
                    Ok(i) => cfg.delete_after_days = i,
                    Err(_) => println!("Invalid integer {:?} for delete_after_days", s)
                }
            }

            if let Some(s) = ini.get_from::<String>(None, "warn_after_days") {
                match i64::from_str(s) {
                    Ok(i) => cfg.warn_after_days = i,
                    Err(_) => println!("Invalid integer {:?} for warn_after_days", s)
                }
            }
        }

        cfg
    }
}

fn main() {
    let version = env!("CARGO_PKG_VERSION");
    let matches = App::new("Trash Expiry")
                    .version(version)
                    .author("Thomas Kluyver")
                    .about("Remove old items from trash.")
                    .get_matches();
    
    let now = Local::now();
    let config = Config::load();
    println!("Trashexpiry config:");
    println!("  After {} days, warn", config.warn_after_days);
    println!("  After {} days, delete", config.delete_after_days);

    let tip = Path::new("/home/takluyver/.local/share/Trash/info"); 
    for tif_res in tip.read_dir().unwrap() {
        let tif = match tif_res {
            Ok(dir_entry) => dir_entry.path(),
            Err(e) => {
                println!("Error getting path: {}", e);
                continue;
            }
        };
        match tif.extension() {
            Some(s) => {
                if s != "trashinfo" {
                    println!("Not a '.trashinfo' file: {:?}", tif);
                    continue
                }
            },
            None => {
                println!("Not a '.trashinfo' file: {:?}", tif);
                continue
            },
        }
        match TrashInfo::from_info_file(&tif) {
            Ok(ti) => {
                let days_ago = now.signed_duration_since(ti.deletion_date).num_days();
                if days_ago >= config.delete_after_days {
                    println!("{}\n ╰ Erasing (deleted {} days ago)",
                        ti.original_path.to_string_lossy(), days_ago);
                    ti.delete().unwrap_or_else(|e| {
                        println!(" ! Error erasing: {}", e);
                    });
                } else if days_ago >= config.warn_after_days {
                    let days_left = config.delete_after_days - days_ago;
                    println!("{}\n ╰ Will be erased in {} days (deleted {} days ago)",
                        ti.original_path.to_string_lossy(), days_left, days_ago);
                }
            },
            Err(e) => {println!("Error reading trash info: {}", e)}
        }
    };
}
