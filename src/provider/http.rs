use reqwest::{Client, header};
use serde::Deserialize;
use std::error::Error;

use crate::provider::{FileContent, Provider};
pub struct PublicHttpEndpoint {
    pub(crate) client: Client,
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
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let client = Client::new();
        Ok(Self { client })
    }

    // Get file (supports byte ranges)
    async fn get(
        &self,
        url: &String,
        range: Option<(u64, u64)>,
    ) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut request = self.client.get(url);
        if let Some((start, end)) = range {
            let range_header = format!("bytes={}-{}", start, end);
            request = request.header(header::RANGE, range_header);
        }
        let response = request.send().await?;
        let bytes = response.bytes().await?;
        Ok(bytes.to_vec())
    }
}
#[async_trait::async_trait]
impl Provider for PublicHttpEndpoint {
    // List contents of a URL
    async fn list_dir(&self, url: &String) -> Result<Vec<Vec<String>>, Box<dyn Error>> {
        println!("Listing URL: {}", url); // Debug print

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

    async fn list_zip(
        &self,
        url: &String,
        glob: &String,
    ) -> Result<Vec<Vec<String>>, Box<dyn Error>> {
        println!("Listing URL: {}", url); // Debug print

        let files = crate::zip::zip_list_http(&self.client, &url).await?;
        Ok(files) // Return empty list as zip_list handles printing
    }

    async fn get_file(&self, url: &String) -> Result<Vec<FileContent>, Box<dyn Error>> {
        let content = self.get(url, None).await?;
        Ok(vec![FileContent {
            filename: url.clone(),
            content,
        }])
    }

    async fn get_file_from_zip(
        &self,
        zip_file: &String,
        path: &String,
    ) -> Result<Vec<FileContent>, Box<dyn Error>> {
        let files = crate::zip::zip_list_http(&self.client, &zip_file).await?;
        let mut content_collection = vec![];
        println!("Files in ZIP:");
        for row in files.iter() {
            let filename = &row[0];
            let offset = row[1].parse::<u64>()?;
            let compressed_size = row[2].parse::<u64>()?;

            if filename.starts_with(path) {
                println!(
                    "Filename: {}, Offset: {}, Compressed Size: {}",
                    filename, offset, compressed_size
                );

                // Step 1: Fetch just the local file header (512 bytes should be enough)
                let header_range = Some((offset, offset + 511));
                println!("Fetching header with range: {:?}", header_range);
                let header_data = self.get(zip_file, header_range).await?;

                // Step 2: Parse local file header to get exact sizes
                let (filename_len, extra_len) = crate::zip::parse_local_file_header(&header_data)?;
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
                let content = self.get(zip_file, range).await?;
                println!("Fetched {} bytes", content.len());

                // Step 5: Decompress
                let uncompressed = crate::zip::decompress_zip_entry(&content)?;
                println!("Decompressed to {} bytes", uncompressed.len());

                content_collection.push(FileContent {
                    filename: path.clone(),
                    content: uncompressed,
                });
            }
        }
        Ok(content_collection)
    }
}
