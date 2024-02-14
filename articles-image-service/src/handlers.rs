use crate::common::web::ErrorInfo;
use crate::dto::ImageProcessingTaskDto;
use crate::image_service::{ImageProcessingService, TaskId};
use actix_multipart::Multipart;
use actix_web::http::header::ContentType;
use actix_web::{get, post, web, HttpResponse, Responder};
use futures_util::{StreamExt, TryStreamExt};
use image::io::Reader;
use std::io::Cursor;
use std::str::FromStr;
use std::sync::Arc;

#[post("/upload")]
pub async fn upload(
    mut payload: Multipart,
    image_service: web::Data<Arc<ImageProcessingService>>,
) -> impl Responder {
    let mut ids: Vec<TaskId> = Vec::new();

    while let Ok(Some(mut field)) = payload.try_next().await {
        let content_type = field.content_disposition();

        if !content_type.is_form_data() {
            return HttpResponse::BadRequest()
                .content_type(ContentType::plaintext())
                .body("Content type is not form data.");
        }

        let mut data: Vec<u8> = Vec::with_capacity(1024 * 1024);

        log::debug!("start reading");
        while let Some(chunk) = field.next().await {
            data.extend_from_slice(&mut chunk.unwrap());
        }

        log::debug!("file readed");
        let task_id = image_service
            .create_image_processing_task(None, None, None, None, None)
            .await;
        log::debug!("task created");

        ids.push(task_id.clone());

        // TODO: performance issue
        let image_service_cloned = image_service.clone();

        tokio::spawn(async move {
            let cursor = Cursor::new(data.as_slice());
            let reader = Reader::new(cursor);

            let image = match reader
                .with_guessed_format()
                .expect("The image format could not be determined")
                .decode()
            {
                Ok(img) => img,
                Err(e) => {
                    image_service_cloned.cancel_task(task_id, e.into());
                    return;
                }
            };

            image_service_cloned
                .attach_image_to_task(task_id, image)
                .unwrap();
        });
        // performance issue end

        log::info!("file processed ");
    }

    HttpResponse::Ok()
        .content_type(ContentType::json())
        .body(format!("{:?}", ids))
}

#[get("/tasks/{id}")]
pub async fn get_image_task_by_id(
    path: web::Path<uuid::Uuid>,
    image_service: web::Data<Arc<ImageProcessingService>>,
) -> impl Responder {
    let id = path.into_inner();

    //TODO: Вынести в нормальную константу
    let errorishe = ErrorInfo {
        error_code: 404,
        msg: String::from_str("not found").unwrap(),
        human_msg: String::from_str("not found").unwrap(),
    };

    match image_service.get_task_by_id(id) {
        Some(task) => {
            let task_dto = ImageProcessingTaskDto::from(task);
            let json = serde_json::to_string(&task_dto).unwrap();
            HttpResponse::Ok()
                .content_type(ContentType::json())
                .body(json)
        }
        None => {
            let json = serde_json::to_string(&errorishe).unwrap();
            HttpResponse::Ok()
                .content_type(ContentType::json())
                .body(json)
        }
    }
}
