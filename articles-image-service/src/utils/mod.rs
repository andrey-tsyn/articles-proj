pub mod fs_utils {
    use std::env;
    use std::path::PathBuf;

    pub fn string_path_to_absolute(input_path: String) -> PathBuf {
        let input_path = PathBuf::from(input_path);

        return if input_path.is_absolute() {
            input_path
        } else {
            let mut current_path = env::current_dir().expect("Failed to get current directory");
            current_path.push(input_path);
            current_path
        }
    }
}


pub mod string_utils {
    use std::str::Split;
    use rand::distributions::Alphanumeric;
    use rand::Rng;

    pub fn generate_random_name(len: usize) -> String {
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(len)
            .map(char::from)
            .collect::<String>()
    }

    pub fn split_path_str_to_folder_names(s: &str) -> Split<&str> {
        let sub_folders: Split<&str>;
        if s.contains("/") {
            sub_folders = s.split("/");
        } else if s.contains("\\") {
            sub_folders = s.split("\\");
        } else {
            sub_folders = s.split("\t\t\t");
        }

        sub_folders
    }
}
