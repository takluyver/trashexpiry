extern crate chrono;
extern crate clap;
extern crate ini;
extern crate xdg;

use ini::Ini;
use clap::{Arg, App};
use chrono::prelude::*;
use std::fs;
use std::io;
use std::io::prelude::*;
use std::iter::Iterator;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

/// Represents an item in trash, based on the XDG Trash specification.
#[derive(Debug)]
struct TrashInfo {
    info_file: PathBuf,
    trashed_file: PathBuf,
    original_path: PathBuf,
    deletion_date: DateTime<Local>,
}


/// From a trash info file, find the corresponding trashed file or directory.
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
    /// Load a trash info file (see the XDG trash spec).
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
    
    /// Discard this item from trash. Deletes both the trashed data and the associated info file.
    fn delete(self) -> Result<(), io::Error> {
        if self.trashed_file.is_dir() {
            fs::remove_dir_all(self.trashed_file)?;
        } else {
            fs::remove_file(self.trashed_file)?;
        }
        fs::remove_file(self.info_file)?;
        Ok(())
    }
}

#[derive(Debug)]
struct Config {
    delete_after_days: i64,
    warn_after_days: i64,
}

/// Find any config files that exist - `trashexpiry.ini` in any XDG config folders.
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
    /// Create the default config.
    fn default() -> Config {
        return Config{
            delete_after_days: 60,
            warn_after_days: 50,
        };
    }

    /// Load config from files, providing the defaults for any values not set.
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

/// Install and enable the systemd service and timer.
fn install_timer() -> Result<(), io::Error> {
    let basedirs = xdg::BaseDirectories::new()?;
    let systemd_dir = basedirs.get_config_home().join("systemd/user");
    println!("Installing to {}", systemd_dir.to_string_lossy());
    
    let service_template = include_str!("trashexpiry.service");
    let trashexpiry_bin = std::env::current_exe()?;
    let service_content = service_template.replace("trashexpiry_bin", trashexpiry_bin.to_str().unwrap());
    let mut service_file = fs::File::create(systemd_dir.join("trashexpiry.service"))?;
    service_file.write_all(&service_content.into_bytes())?;
    println!("Written trashexpiry.service");

    let mut timer_file = fs::File::create(systemd_dir.join("trashexpiry.timer"))?;
    timer_file.write_all(include_bytes!("trashexpiry.timer"))?;
    println!("Written trashexpiry.timer");

    if Command::new("systemctl").args(&["--user", "enable", "trashexpiry.timer"]).status()?.success() {
        println!("Installed timer; old trash will be cleared daily");
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::Other, "systemctl indicated failure"))
    }
}

fn main() {
    let version = env!("CARGO_PKG_VERSION");
    let matches = App::new("Trash Expiry")
                    .version(version)
                    .author("Thomas Kluyver")
                    .about("Remove old items from trash.")
                    .arg(Arg::with_name("install")
                         .long("install-timer")
                         .help("Install a systemd timer to run Trashexpiry daily"))
                    .get_matches();
    
    if matches.is_present("install") {
        if let Err(e) = install_timer() {
            println!("Error installing timer: {}", e);
            std::process::exit(1);
        } else {
            return;
        }
    }
    
    let now = Local::now();
    let config = Config::load();
    println!("Trashexpiry config:");
    println!("  After {} days, warn", config.warn_after_days);
    println!("  After {} days, delete", config.delete_after_days);

    let mut status = 0;

    let tip = {
        let basedirs = xdg::BaseDirectories::new().unwrap();
        basedirs.get_data_home().join("Trash/info")
    };
    println!("Trash info dir: {}", tip.to_string_lossy());

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
                        status = 1;
                    });
                } else if days_ago >= config.warn_after_days {
                    let days_left = config.delete_after_days - days_ago;
                    println!("{}\n ╰ Will be erased in {} days (deleted {} days ago)",
                        ti.original_path.to_string_lossy(), days_left, days_ago);
                }
            },
            Err(e) => {
                println!("Error reading trash info: {}", e);
                status = 1;
            }
        }
    };
    std::process::exit(status);
}
