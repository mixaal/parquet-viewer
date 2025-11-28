use std::error::Error;

#[async_trait::async_trait]
pub trait Provider {
    // List contents of a path or URL
    async fn list(&self, path: String) -> Result<Vec<Vec<String>>, Box<dyn std::error::Error>>;

    // Get file (supports byte ranges)
    async fn get(&self, url: String, range: Option<(u64, u64)>) -> Result<Vec<u8>, Box<dyn Error>>;

    // view file contents
    async fn view(&self, _path: String, _max_rows: usize) -> Result<(), Box<dyn Error>>;

    // Change directory (for filesystem providers)
    fn change_dir(&mut self, _path: &str) -> Result<(), Box<dyn Error>>;

    fn get_current_dir(&self) -> String;
}
