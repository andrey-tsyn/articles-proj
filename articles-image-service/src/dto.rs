use std::ops::Deref;

use serde::Serialize;

use crate::common::web::ErrorInfo;
use crate::image_service::{ImageProcessingStatus, ImageProcessingTask, TaskId};

#[derive(Serialize)]
pub struct ImageProcessingTaskDto {
    pub id: TaskId,
    pub error: Option<ErrorInfo>,
    pub status: String,
}

impl From<&ImageProcessingTask> for ImageProcessingTaskDto {
    fn from(value: &ImageProcessingTask) -> Self {
        let mut error = None;

        let status = match value.get_status() {
            ImageProcessingStatus::WaitingForImageAttachment => "Waiting for image",
            ImageProcessingStatus::Waiting => "Waiting for process",
            ImageProcessingStatus::InProgress => "In progress",
            ImageProcessingStatus::Completed(_) => "Completed",
            ImageProcessingStatus::Canceled(err) => {
                let err = err.deref();
                error = Some(ErrorInfo {
                    error_code: 0,
                    msg: err.to_string(),
                    human_msg: err.to_string(),
                });

                "Error"
            }
        }
        .to_string();

        ImageProcessingTaskDto {
            id: value.id,
            error,
            status,
        }
    }
}

impl From<ImageProcessingTask> for ImageProcessingTaskDto {
    fn from(value: ImageProcessingTask) -> Self {
        return ImageProcessingTaskDto::from(&value);
    }
}
