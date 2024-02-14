pub type StringPath = String;

pub mod web {
    use serde::Serialize;

    #[derive(Serialize)]
    pub struct ErrorInfo {
        pub error_code: i32,
        pub msg: String,
        pub human_msg: String,
    }
}

pub mod error {
    use std::fmt::Debug;
    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum CommonError {
        #[error("Illegal argument error: {0}")]
        IllegalArgument(String),
    }
}
