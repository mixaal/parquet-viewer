use std::error::Error;

pub struct FileContent {
    pub(crate) filename: String,
    pub(crate) content: Vec<u8>,
}

#[async_trait::async_trait]
pub trait Provider {
    // List contents of a path or URL
    async fn list_dir(
        &self,
        cwd: &String,
        path: &String,
    ) -> Result<Vec<Vec<String>>, Box<dyn std::error::Error>>;

    async fn list_zip(
        &self,
        zip_file: &String,
        glob_match: &String,
    ) -> Result<Vec<Vec<String>>, Box<dyn Error>>;

    // get file contents
    async fn get_file(&self, _path: &String) -> Result<Vec<FileContent>, Box<dyn Error>>;

    // get file contents from zip
    async fn get_file_from_zip(
        &self,
        zip_file: &String,
        glob: &String,
    ) -> Result<Vec<FileContent>, Box<dyn Error>>;
}

pub mod fs;
pub mod http;
