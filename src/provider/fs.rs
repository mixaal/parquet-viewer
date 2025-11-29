use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::vec;

use crate::provider::{FileContent, Provider};

pub struct LocalFs {}

impl LocalFs {
    pub fn new() -> Self {
        Self {}
    }

    fn get_local_file_content(path: &String) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut buf: Vec<u8> = vec![];
        let mut file = File::open(path)?;
        file.read_to_end(&mut buf)?;
        Ok(buf)
    }
}

#[async_trait::async_trait]
impl Provider for LocalFs {
    // Read the contents of a file

    // List files in a directory
    async fn list_dir(
        &self,
        cwd: &String,
        path: &String,
    ) -> Result<Vec<Vec<String>>, Box<dyn Error>> {
        println!("Listing {path} from: {cwd}"); // Debug print

        let mut files = vec![];
        let pattern = format!("{cwd}/{path}");
        for entry in glob::glob(&pattern)? {
            if let Ok(path) = entry {
                let file_name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                files.push(vec![file_name]);
            }
        }
        return Ok(files);
    }

    async fn list_zip(
        &self,
        zip_file: &String,
        glob: &String,
    ) -> Result<Vec<Vec<String>>, Box<dyn Error>> {
        println!("Listing zip: {zip_file}/{glob}"); // Debug print

        let files = crate::zip::zip_list_from_local_file(&zip_file)?;
        return Ok(files); // Return empty list as zip_list handles printing
    }

    async fn get_file(&self, path: &String) -> Result<Vec<FileContent>, Box<dyn Error>> {
        let content = Self::get_local_file_content(path)?;
        Ok(vec![FileContent {
            filename: path.clone(),
            content,
        }])
    }

    async fn get_file_from_zip(
        &self,
        path: &String,
        glob: &String,
    ) -> Result<Vec<FileContent>, Box<dyn Error>> {
        let mut file_collection = vec![];
        let files = crate::zip::zip_list_from_local_file(path)?;
        println!("Files in ZIP:");
        for (index, row) in files.iter().enumerate() {
            let filename = &row[0];

            if filename.starts_with(glob) {
                println!("Filename: {}, Index: {}", filename, index);
                let content = crate::zip::zip_extract_from_local_file(path, index)?;
                file_collection.push(FileContent {
                    filename: filename.clone(),
                    content,
                });
            }
        }

        return Ok(file_collection);
    }
}
