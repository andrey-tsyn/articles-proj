use crate::common::StringPath;
use crate::image::save_as_jpeg;
use crate::utils::string_utils;
use image::DynamicImage;
use std::collections::{HashMap, VecDeque};
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use std::{env, thread};
use thiserror::Error;
use uuid::Uuid;

pub type TaskId = Uuid;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Image in ImageProcessingTask already assigned")]
    ImageAlreadyAssigned,
    #[error("Task with id {0} not found")]
    TaskNotFound(TaskId),
}

#[derive(Debug, Clone)]
pub enum ImageProcessingStatus {
    WaitingForImageAttachment,
    Waiting,
    InProgress,
    Completed(Arc<anyhow::Result<StringPath>>),
    Canceled(Arc<anyhow::Error>),
}

pub struct ImageProcessingTask {
    pub id: TaskId,

    status: ImageProcessingStatus,
    folder: PathBuf,
    file_name: String,
    image: Option<Arc<DynamicImage>>,
    processing_func: Option<fn(&Arc<DynamicImage>) -> Result<Arc<DynamicImage>, anyhow::Error>>,
    callback: Option<fn()>,
}

impl ImageProcessingTask {
    pub fn get_status(&self) -> &ImageProcessingStatus {
        &self.status
    }
}

impl Clone for ImageProcessingTask {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            status: self.status.clone(),
            folder: self.folder.clone(),
            file_name: self.file_name.clone(),
            image: self.image.clone(),
            processing_func: self.processing_func.clone(),
            callback: self.callback.clone(),
        }
    }
}

pub struct ImageProcessingService {
    pub max_in_progress_tasks_count: u32,
    pub image_quality: u8,
    pub image_folder: PathBuf,

    tasks: Mutex<HashMap<TaskId, ImageProcessingTask>>,
    waiting_task_ids: Mutex<VecDeque<TaskId>>,
    in_progress_tasks_count: AtomicU32,

    completed_tasks_id_tx: Sender<(TaskId, ImageProcessingStatus)>,
    completed_tasks_id_rx: Mutex<Receiver<(TaskId, ImageProcessingStatus)>>,
}

impl Default for ImageProcessingService {
    fn default() -> Self {
        let (tx, rx): (
            Sender<(TaskId, ImageProcessingStatus)>,
            Receiver<(TaskId, ImageProcessingStatus)>,
        ) = mpsc::channel();
        let rx = Mutex::new(rx);

        Self {
            tasks: Mutex::new(HashMap::new()),
            waiting_task_ids: Mutex::new(VecDeque::new()),
            in_progress_tasks_count: AtomicU32::new(0),
            max_in_progress_tasks_count: 10,
            image_quality: 30,
            image_folder: env::current_dir().unwrap(),
            completed_tasks_id_rx: rx,
            completed_tasks_id_tx: tx,
        }
    }
}

impl ImageProcessingService {
    pub fn new(max_in_progress_tasks_count: u32, image_quality: u8) -> Self {
        Self {
            max_in_progress_tasks_count,
            image_quality,
            ..ImageProcessingService::default()
        }
    }

    pub fn new_with_custom_path(
        max_in_progress_tasks_count: u32,
        image_quality: u8,
        image_folder: PathBuf,
    ) -> Self {
        Self {
            max_in_progress_tasks_count,
            image_quality,
            image_folder,
            ..ImageProcessingService::default()
        }
    }
}

impl ImageProcessingService {
    pub async fn create_image_processing_task(
        &self,
        image: Option<Arc<DynamicImage>>,
        name: Option<&str>,
        sub_folder: Option<&str>,
        processing_func: Option<fn(&Arc<DynamicImage>) -> Result<Arc<DynamicImage>, anyhow::Error>>,
        callback: Option<fn()>,
    ) -> TaskId {
        let mut image_folder = self.image_folder.clone();
        let callback = callback.clone();

        if let Some(sub_folder) = sub_folder {
            let sub_folders = string_utils::split_path_str_to_folder_names(sub_folder);

            for sub_folder in sub_folders {
                image_folder.push(sub_folder);
            }
        }

        let with_image = image.is_some();

        let name = match name {
            None => string_utils::generate_random_name(15),
            Some(s) => s.to_string(),
        };

        let id = Uuid::new_v4();

        let task = ImageProcessingTask {
            id,
            status: if with_image {
                ImageProcessingStatus::WaitingForImageAttachment
            } else {
                ImageProcessingStatus::Waiting
            },
            folder: image_folder,
            file_name: name,
            image,
            processing_func,
            callback,
        };

        {
            let mut tasks = self.tasks.lock().unwrap();
            tasks.insert(task.id, task);
        }

        if with_image {
            let mut waiting_tasks = self.waiting_task_ids.lock().unwrap();
            waiting_tasks.push_back(id);
        }

        self.start_next_task_if_possible();
        log::trace!("ImageProcessingTask with id {} created!", id);
        id
    }

    pub fn attach_image_to_task(&self, id: TaskId, image: DynamicImage) -> Result<(), Error> {
        let mut tasks = self.tasks.lock().unwrap();

        if let Some(task) = tasks.get_mut(&id) {
            if task.image.is_none() {
                task.image = Some(Arc::new(image));
                task.status = ImageProcessingStatus::Waiting;
                let mut waiting_tasks = self.waiting_task_ids.lock().unwrap();
                waiting_tasks.push_back(task.id);
            } else {
                return Err(Error::ImageAlreadyAssigned);
            }
        } else {
            return Err(Error::TaskNotFound(id));
        }

        Ok(())
    }

    fn start_next_task_if_possible(&self) {
        let id = 'block: {
            let mut waiting_task_ids = self.waiting_task_ids.lock().unwrap();

            if (self.in_progress_tasks_count.load(Ordering::Acquire)
                >= self.max_in_progress_tasks_count)
                || waiting_task_ids.is_empty()
            {
                break 'block None;
            }

            self.in_progress_tasks_count.fetch_add(1, Ordering::Release);

            Some(waiting_task_ids.pop_front().unwrap())
        };

        if let Some(id) = id {
            self.change_image_processing_state_by_id(id, ImageProcessingStatus::InProgress);
            let tasks = self.tasks.lock().unwrap();
            let task = tasks.get(&id).expect("Task not found!");

            self.process_task(task);
        }
    }

    fn process_task(&self, task: &ImageProcessingTask) {
        let id = task.id;
        let folder = task.folder.clone();
        let file_name = task.file_name.clone();
        let image = task.image.clone();
        let image_quality = self.image_quality;
        let processing_func = task.processing_func.clone();
        let callback = task.callback.clone();
        let tx = self.completed_tasks_id_tx.clone();

        thread::spawn(move || {
            log::trace!("ImageProcessingTask with id {} was started.", id);
            let mut result_path = folder.clone();
            result_path.set_file_name(format!("{}.{}", file_name, "jpg"));

            let mut image = image.unwrap();

            if let Some(proc_func) = processing_func {
                image = match proc_func(&image) {
                    Ok(img) => img,
                    Err(e) => {
                        tx.send((id, ImageProcessingStatus::Completed(Arc::new(Err(e)))))
                            .unwrap();
                        return;
                    }
                };
            }

            let result = match save_as_jpeg(&result_path, &image, image_quality) {
                Ok(res) => Ok(res),
                Err(e) => Err(e),
            };

            tx.send((id, ImageProcessingStatus::Completed(Arc::new(result))))
                .unwrap();

            if let Some(callback) = callback {
                callback();
            }
        });
    }

    pub async fn start_update_loop(&self, update_delay: Duration) {
        log::info!("Update loop started!");

        loop {
            tokio::time::sleep(update_delay).await;

            let rx = self.completed_tasks_id_rx.lock().unwrap();

            for (id, status) in rx.try_iter() {
                if let ImageProcessingStatus::Completed(result) = &status {
                    match result.deref() {
                        Ok(path) => {
                            log::trace!("Task with id {} is done! Image path: {}", id, path)
                        }

                        Err(e) => log::error!(
                            "An error occurred while executing task with id {}. Error: {:?}",
                            id,
                            e
                        ),
                    }
                }

                self.change_image_processing_state_by_id(id, status);
                self.in_progress_tasks_count.fetch_sub(1, Ordering::Release);
                self.start_next_task_if_possible();
            }
        }
    }

    pub fn remove_image_processing_task(&self, id: TaskId) -> bool {
        let mut map = self.tasks.lock().unwrap();

        if let Some(_) = map.remove(&id) {
            true
        } else {
            false
        }
    }

    pub fn cancel_task(&self, task_id: TaskId, reason: anyhow::Error) {
        self.change_image_processing_state_by_id(
            task_id,
            ImageProcessingStatus::Canceled(Arc::new(reason)),
        );
    }

    fn change_image_processing_state_by_id(
        &self,
        id: TaskId,
        status: ImageProcessingStatus,
    ) -> bool {
        let mut map = self.tasks.lock().unwrap();

        if let Some(task) = map.get_mut(&id) {
            task.status = status;
            true
        } else {
            false
        }
    }

    pub fn get_task_by_id(&self, task_id: TaskId) -> Option<ImageProcessingTask> {
        let map = self.tasks.lock().unwrap();
        let task = map.get(&task_id);

        match task {
            Some(task) => Some(task.clone()),
            None => None,
        }
    }
}
