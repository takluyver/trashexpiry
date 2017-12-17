extern crate chrono;
extern crate clap;
extern crate ini;
extern crate xdg;

use ini::Ini;
use clap::{Arg, App};
use chrono::prelude::*;
use std::fs;
use std::io;
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

impl Config {
    fn default() -> Config {
        return Config{
            delete_after_days: 60,
            warn_after_days: 50,
        };
    }
    
    fn load() -> Config {
        let dflt = Config::default();
        let ini_path = match xdg::BaseDirectories::new().unwrap().find_config_file("trashexpiry.ini") {
            Some(p) => p,
            None => return dflt
        };
        let ini = match Ini::load_from_file(ini_path) {
            Ok(data) => data,
            Err(e) => {
                println!("Error reading config file: {}", e);
                return dflt;
            }
        };
        let delete_after_s = ini.get_from::<String>(None, "delete_after_days");
        let delete_after = delete_after_s.map(|s| {
            i64::from_str(s).unwrap_or_else(|_| {
                println!("Invalid integer {:?} for delete_after_days; using default.", s);
                dflt.delete_after_days
            })
        }).unwrap_or(dflt.delete_after_days);
        
        let warn_after = ini.get_from::<String>(None, "warn_after_days").map(|s| {
            i64::from_str(s).unwrap_or_else(|_| {
                println!("Invalid integer {:?} for warn_after_days; using default.", s);
                dflt.warn_after_days
            })
        }).unwrap_or(dflt.warn_after_days);

        Config {
            delete_after_days: delete_after,
            warn_after_days: warn_after,
        }
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
