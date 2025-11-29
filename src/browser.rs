use std::error::Error;

use crate::{
    pqt,
    provider::{fs::LocalFs, http::PublicHttpEndpoint},
};

pub struct FileBrowser {
    endpoint: String,
    http: PublicHttpEndpoint,
    localfs: LocalFs,
}

impl FileBrowser {
    pub fn new(endpoint: String) -> Result<Self, Box<dyn Error>> {
        let http = PublicHttpEndpoint::new()?;
        let localfs = LocalFs::new();
        Ok(Self {
            endpoint,
            http,
            localfs,
        })
    }

    fn get_full_path(&self, path: &str) -> String {
        if path.len() == 0 {
            return self.endpoint.clone();
        }
        if self.endpoint.ends_with('/') {
            format!("{}{}", self.endpoint, path)
        } else {
            format!("{}/{}", self.endpoint, path)
        }
    }

    fn get_parent(&self) -> String {
        let path = std::path::Path::new(&self.endpoint);
        if let Some(parent) = path.parent() {
            parent.to_string_lossy().to_string()
        } else {
            self.endpoint.clone()
        }
    }
    pub(crate) fn change_dir(&mut self, path: &str) -> Result<(), Box<dyn Error>> {
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

    pub(crate) fn get_current_dir(&self) -> String {
        self.endpoint.clone()
        // std::env::current_dir()
        //     .unwrap_or_else(|_| std::path::PathBuf::from("."))
        //     .to_string_lossy()
        //     .to_string()
    }

    // fn get_provider(&mut self) -> &mut dyn crate::provider::Provider {
    //     if self.endpoint.starts_with("http://") || self.endpoint.starts_with("https://") {
    //         &mut self.http
    //     } else {
    //         &mut self.localfs
    //     }
    // }

    fn get_provider(&self) -> &dyn crate::provider::Provider {
        if self.endpoint.starts_with("http://") || self.endpoint.starts_with("https://") {
            &self.http
        } else {
            &self.localfs
        }
    }

    pub(crate) async fn list(&self, path: String) -> Result<Vec<Vec<String>>, Box<dyn Error>> {
        let provider = self.get_provider();
        if self.endpoint.ends_with("zip") {
            provider.list_zip(&self.endpoint, &path).await
        } else {
            provider.list_dir(&self.endpoint, &path).await
        }
    }

    pub(crate) async fn view(&self, path: String, max_rows: usize) -> Result<(), Box<dyn Error>> {
        let provider = self.get_provider();
        let url = self.get_full_path(&path);
        let files = if self.endpoint.ends_with("zip") {
            provider.get_file_from_zip(&self.endpoint, &path).await
        } else {
            provider.get_file(&url).await
        }?;

        for file in files.iter() {
            if file.filename.ends_with(".parquet") {
                println!("Viewing Parquet file:");
                pqt::parquet_view_from_slice(&file.content, max_rows)?;
            } else {
                let readable_content = String::from_utf8_lossy(&file.content);
                println!("File contents:\n{}", readable_content);
            }
        }

        Ok(())
    }
}
