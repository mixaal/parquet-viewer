use reqwest::{Client, header};
use serde::Deserialize;
use std::error::Error;

use crate::provider::Provider;
pub struct PublicHttpEndpoint {
    pub(crate) client: Client,
    endpoint: String,
}

#[derive(Deserialize)]
struct ListResponse {
    objects: Vec<ListEntry>,
}

#[derive(Deserialize)]
struct ListEntry {
    name: Option<String>,
}

impl PublicHttpEndpoint {
    pub fn new(endpoint: String) -> Result<Self, Box<dyn Error>> {
        let client = Client::new();
        Ok(Self { client, endpoint })
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

    pub(crate) fn set_endpoint(&mut self, endpoint: String) {
        self.endpoint = endpoint;
    }
}
#[async_trait::async_trait]
impl Provider for PublicHttpEndpoint {
    // List contents of a URL
    async fn list(&self, path: String) -> Result<Vec<Vec<String>>, Box<dyn Error>> {
        let url = self.get_full_path(&path);
        println!("Listing URL: {}", url); // Debug print

        if url.ends_with("zip") {
            let files = crate::zip::zip_list_http(&self.client, &url).await?;
            Ok(files) // Return empty list as zip_list handles printing
        } else {
            let response = self.client.get(url).send().await?;
            let body = response.text().await?;
            println!("Response Body: {}", body); // Debug print
            let list: ListResponse = serde_json::from_str(&body)?;
            let mut files = vec![];
            for entry in list.objects {
                if let Some(name) = entry.name {
                    files.push(vec![name]);
                }
            }
            Ok(files)
        }
    }

    // Get file (supports byte ranges)
    async fn get(&self, url: String, range: Option<(u64, u64)>) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut request = self.client.get(url);
        if let Some((start, end)) = range {
            let range_header = format!("bytes={}-{}", start, end);
            request = request.header(header::RANGE, range_header);
        }
        let response = request.send().await?;
        let bytes = response.bytes().await?;
        Ok(bytes.to_vec())
    }

    async fn view(&self, path: String, _max_rows: usize) -> Result<(), Box<dyn Error>> {
        if self.endpoint.ends_with("zip") {
            let files = crate::zip::zip_list_http(&self.client, &self.endpoint).await?;

            println!("Files in ZIP:");
            for row in files.iter() {
                let filename = &row[0];
                let offset = row[1].parse::<u64>()?;
                let compressed_size = row[2].parse::<u64>()?;

                if filename.starts_with(&path) {
                    println!(
                        "Filename: {}, Offset: {}, Compressed Size: {}",
                        filename, offset, compressed_size
                    );

                    // Step 1: Fetch just the local file header (512 bytes should be enough)
                    let header_range = Some((offset, offset + 511));
                    println!("Fetching header with range: {:?}", header_range);
                    let header_data = self.get(self.endpoint.clone(), header_range).await?;

                    // Step 2: Parse local file header to get exact sizes
                    let (filename_len, extra_len) =
                        crate::zip::parse_local_file_header(&header_data)?;
                    println!(
                        "Local file header: filename_len={}, extra_len={}",
                        filename_len, extra_len
                    );

                    // Step 3: Calculate exact size needed
                    let header_size = 30 + filename_len + extra_len;
                    let total_size = header_size + compressed_size;

                    // Step 4: Fetch the complete entry
                    let range = Some((offset, offset + total_size - 1));
                    println!("Fetching complete entry with range: {:?}", range);
                    let content = self.get(self.endpoint.clone(), range).await?;
                    println!("Fetched {} bytes", content.len());

                    // Step 5: Decompress
                    let uncompressed = crate::zip::decompress_zip_entry(&content)?;
                    println!("Decompressed to {} bytes", uncompressed.len());

                    if !path.ends_with("parquet") {
                        println!("Not a Parquet file, skipping view.");
                        continue;
                    }
                    // Step 6: View Parquet
                    let temp_file_path = "temp_view_file.parquet";
                    std::fs::write(temp_file_path, &uncompressed)?;
                    crate::pqt::parquet_view(temp_file_path.to_string(), 100)?;

                    let _ = std::fs::remove_file(temp_file_path);
                }
            }
        } else {
            let url = self.get_full_path(&path);
            println!("Viewing URL: {}", url); // Debug print

            let content = self.get(url, None).await?;

            let temp_file_path = "temp_view_file.parquet";
            std::fs::write(temp_file_path, &content).unwrap();
            crate::pqt::parquet_view(temp_file_path.to_string(), 100)?;

            let _ = std::fs::remove_file(temp_file_path);
        }
        Ok(())
    }

    fn change_dir(&mut self, path: &str) -> Result<(), Box<dyn Error>> {
        self.endpoint = self.get_full_path(path);
        println!("Changed directory to: {}", self.endpoint);
        Ok(())
    }

    fn get_current_dir(&self) -> String {
        self.endpoint.clone()
    }
}
