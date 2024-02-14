use std::env;
use std::string::String;
use std::sync::Arc;
use std::time::Duration;

use crate::image_service::ImageProcessingService;
use actix_web::web::Data;
use actix_web::{App, HttpServer};

pub mod common;
mod dto;
mod handlers;
mod image;
mod image_service;
mod utils;

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv()?;
    init_logger()?;

    let path = utils::fs_utils::string_path_to_absolute(
        env::var("STATIC_FILES_PATH").unwrap_or(String::from("static/")),
    );

    log::info!("Image save path: {:?}", path);

    let image_service = Arc::new(ImageProcessingService::new_with_custom_path(3, 20, path));

    let image_service_cloned = image_service.clone();

    tokio::spawn(async move {
        image_service
            .start_update_loop(Duration::from_millis(100))
            .await;
    });

    HttpServer::new(move || {
        App::new()
            // Add handlers
            .service(handlers::upload)
            .service(handlers::get_image_task_by_id)
            // Add middlewares
            .wrap(actix_web::middleware::Logger::default())
            // Add services
            .app_data(Data::new(image_service_cloned.clone()))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await?;

    Ok(())
}

fn init_logger() -> Result<(), fern::InitError> {
    let log_level = env::var("LOG_LEVEL").unwrap_or_else(|_| "INFO".into());
    let log_level = log_level.parse().unwrap_or(log::LevelFilter::Info);

    let mut builder = fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}][{}][{}] {}",
                chrono::Local::now().format("%H:%M:%S"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(log_level)
        .chain(std::io::stderr());

    if let Ok(log_file) = env::var("LOG_FILE") {
        let log_file = std::fs::File::create(log_file)?;
        builder = builder.chain(log_file);
    }

    builder.apply()?;

    log::trace!("TRACE output enabled");
    log::debug!("DEBUG output enabled");
    log::info!("INFO output enabled");
    log::warn!("WARN output enabled");
    log::error!("ERROR output enabled");

    Ok(())
}
