use std::error::Error;
use std::io::Read;
use std::{fs, vec};

use crate::provider::Provider;

pub struct LocalFs {
    endpoint: String,
}

impl LocalFs {
    pub fn new(endpoint: String) -> Self {
        Self { endpoint }
    }

    pub fn get_full_path(&self, path: &str) -> String {
        if path.len() == 0 {
            return self.endpoint.clone();
        }
        if self.endpoint.ends_with('/') {
            format!("{}{}", self.endpoint, path)
        } else {
            format!("{}/{}", self.endpoint, path)
        }
    }

    pub fn set_endpoint(&mut self, endpoint: String) {
        self.endpoint = endpoint;
    }

    fn get_parent(&self) -> String {
        let path = std::path::Path::new(&self.endpoint);
        if let Some(parent) = path.parent() {
            parent.to_string_lossy().to_string()
        } else {
            self.endpoint.clone()
        }
    }
}

#[async_trait::async_trait]
impl Provider for LocalFs {
    // Read the contents of a file
    async fn get(
        &self,
        path: String,
        _range: Option<(u64, u64)>,
    ) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut file = fs::File::open(path)?;
        let mut contents = vec![];
        file.read(&mut contents)?;
        Ok(contents)
    }

    // List files in a directory
    async fn list(&self, path: String) -> Result<Vec<Vec<String>>, Box<dyn Error>> {
        let url = self.get_full_path(&path);
        println!("Listing URL: {:?}", url); // Debug print

        if url.ends_with("zip") {
            let files = crate::zip::zip_list_from_local_file(&url)?;
            return Ok(files); // Return empty list as zip_list handles printing
        } else {
            let mut files = vec![];
            for entry in fs::read_dir(url)? {
                let entry = entry?;
                let file_name = entry.file_name().into_string().unwrap_or_default();
                files.push(vec![file_name]);
            }
            return Ok(files);
        }
    }

    async fn view(&self, path: String, _max_rows: usize) -> Result<(), Box<dyn Error>> {
        if self.endpoint.ends_with("zip") {
            let files = crate::zip::zip_list_from_local_file(&self.endpoint)?;
            println!("Files in ZIP:");
            for (index, row) in files.iter().enumerate() {
                let filename = &row[0];

                if filename.starts_with(&path) {
                    println!("Filename: {}, Index: {}", filename, index);
                    let content = crate::zip::zip_extract_from_local_file(&self.endpoint, index)?;
                    if path.ends_with(".parquet") {
                        println!("Viewing Parquet file: {}", filename);
                        let temp_file_path = "temp_view_file.parquet";
                        std::fs::write(temp_file_path, &content)?;
                        crate::pqt::parquet_view(temp_file_path.to_string(), 100)?;
                        let _ = std::fs::remove_file(temp_file_path);
                    } else {
                        let readable_content = String::from_utf8_lossy(&content);
                        println!("File contents:\n{}", readable_content);
                    }
                }
            }

            return Ok(());
        } else {
            let r = self.get(path, None).await?;
            println!("File contents: {:?}", r);
        }
        Ok(())
    }

    fn change_dir(&mut self, path: &str) -> Result<(), Box<dyn Error>> {
        if path == "." || path.is_empty() || path == "./" {
            return Ok(());
        }
        if path == ".." {
            let parent = self.get_parent();
            self.endpoint = parent;
            return Ok(());
        }
        if path.starts_with("/") || path.starts_with("http") {
            self.endpoint = path.to_string();
            return Ok(());
        }
        if self.endpoint.ends_with("zip") {
            return Err("Cannot change directory inside a ZIP file".into());
        }

        self.endpoint = self.get_full_path(path);

        // std::env::set_current_dir(path)?;

        Ok(())
    }

    fn get_current_dir(&self) -> String {
        self.endpoint.clone()
        // std::env::current_dir()
        //     .unwrap_or_else(|_| std::path::PathBuf::from("."))
        //     .to_string_lossy()
        //     .to_string()
    }
}
