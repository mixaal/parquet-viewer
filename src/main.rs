use std::error::Error;

use parquet_viewer::console;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut console = console::Console::new(
        // "https://objectstorage.us-phoenix-1.oraclecloud.com/n/oraclebigdatadb/b/bucket-20250414-1013/o/",
        "/mnt/c/Users/micha/Downloads/",
    )?;
    console.process_console_input().await;
    Ok(())
}
