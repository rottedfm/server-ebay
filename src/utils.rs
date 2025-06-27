// --- src/log.rs
use chrono::Local;
use std::{
    fs::{OpenOptions, create_dir_all},
    io::Write,
    path::Path,
};

use env_logger::Builder;

pub fn setup_logger() -> anyhow::Result<()> {
    let log_dir = Path::new("logs");
    create_dir_all(log_dir)?;

    let now = Local::now().format("%d-%m-%Y").to_string();
    let log_path = log_dir.join(format!("{}.log", now));
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    Builder::new()
        .filter(None, log::LevelFilter::Debug)
        .write_style(env_logger::WriteStyle::Always)
        .format(|buf, record| {
            writeln!(
                buf,
                "[{}] {} - {}",
                record.level(),
                record.target(),
                record.args()
            )
        })
        .target(env_logger::Target::Pipe(Box::new(file)))
        .init();

    println!("® Logging to file {:?}", log_path);
    Ok(())
}
