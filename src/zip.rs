use std::io::Read;

use reqwest::{Client, header};

pub async fn zip_list_http(
    client: &Client,
    url: &str,
) -> Result<Vec<Vec<String>>, Box<dyn std::error::Error>> {
    // Step 1: Get file size
    let head_response = client.head(url).send().await?;
    let file_size = head_response
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .ok_or("Server doesn't provide content-length")?;

    println!("File size: {}", file_size);

    // Step 2: Fetch the last 64KB to find EOCD
    let eocd_size = 65557u64.min(file_size);
    let eocd_start = file_size - eocd_size;

    let range_header = format!("bytes={}-{}", eocd_start, file_size - 1);
    println!("Fetching EOCD with range: {}", range_header);

    let response = client
        .get(url)
        .header(header::RANGE, range_header)
        .send()
        .await?;

    let eocd_bytes = response.bytes().await?;
    println!("Received {} bytes", eocd_bytes.len());

    // Step 3: Check if ZIP64 or regular ZIP
    let mut cursor = std::io::Cursor::new(eocd_bytes.to_vec());

    match check_zip_format(&mut cursor) {
        ZipFormat::Zip64(eocd64_offset) => {
            println!("Detected ZIP64 format, EOCD64 at offset {}", eocd64_offset);
            let local_offset = eocd64_offset - eocd_start;
            println!("Adjusted EOCD64 offset in buffer: {}", local_offset);
            handle_zip64(client, url, &eocd_bytes.to_vec(), local_offset).await
        }
        ZipFormat::Regular(cd_offset, cd_size) => {
            println!("Detected regular ZIP format");
            handle_regular_zip(client, url, cd_offset, cd_size).await
        }
    }
}

enum ZipFormat {
    Zip64(u64),        // EOCD64 offset
    Regular(u64, u64), // CD offset, CD size
}

fn check_zip_format(cursor: &mut std::io::Cursor<Vec<u8>>) -> ZipFormat {
    use byteorder::{LittleEndian, ReadBytesExt};

    let data = cursor.get_ref();

    const EOCD_SIGNATURE: u32 = 0x06054b50;
    const EOCD64_LOCATOR_SIGNATURE: u32 = 0x07064b50;
    const EOCD_MIN_SIZE: usize = 22;

    // Find the regular EOCD (present in both regular and ZIP64)
    let mut eocd_pos = None;
    for i in (0..=data.len().saturating_sub(EOCD_MIN_SIZE)).rev() {
        let sig = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
        if sig == EOCD_SIGNATURE {
            eocd_pos = Some(i);
            println!("Found EOCD at buffer offset {}", i);
            break;
        }
    }

    let eocd_pos = match eocd_pos {
        Some(pos) => pos,
        None => panic!("Could not find EOCD signature"),
    };

    // Check for ZIP64 EOCD locator just before EOCD
    if eocd_pos >= 20 {
        let locator_pos = eocd_pos - 20;
        let sig = u32::from_le_bytes([
            data[locator_pos],
            data[locator_pos + 1],
            data[locator_pos + 2],
            data[locator_pos + 3],
        ]);

        if sig == EOCD64_LOCATOR_SIGNATURE {
            println!("Found ZIP64 EOCD locator at buffer offset {}", locator_pos);

            // Read the ZIP64 EOCD offset
            cursor.set_position(locator_pos as u64);
            cursor.read_u32::<LittleEndian>().unwrap(); // signature
            cursor.read_u32::<LittleEndian>().unwrap(); // disk number
            let eocd64_offset = cursor.read_u64::<LittleEndian>().unwrap();

            return ZipFormat::Zip64(eocd64_offset);
        }
    }

    // Regular ZIP - parse EOCD
    cursor.set_position(eocd_pos as u64);
    cursor.read_u32::<LittleEndian>().unwrap(); // signature
    cursor.read_u16::<LittleEndian>().unwrap(); // disk number
    cursor.read_u16::<LittleEndian>().unwrap(); // disk with central directory
    cursor.read_u16::<LittleEndian>().unwrap(); // entries on this disk
    cursor.read_u16::<LittleEndian>().unwrap(); // total entries
    let cd_size = cursor.read_u32::<LittleEndian>().unwrap() as u64;
    let cd_offset = cursor.read_u32::<LittleEndian>().unwrap() as u64;

    println!("Regular ZIP: CD offset={}, size={}", cd_offset, cd_size);
    ZipFormat::Regular(cd_offset, cd_size)
}

async fn handle_zip64(
    client: &Client,
    url: &str,
    bytes: &Vec<u8>,
    local_offset: u64,
) -> Result<Vec<Vec<String>>, Box<dyn std::error::Error>> {
    use byteorder::{LittleEndian, ReadBytesExt};
    // Parse EOCD64
    let mut cursor = std::io::Cursor::new(&bytes[local_offset as usize..]);
    const EOCD64_SIGNATURE: u32 = 0x06064b50;
    let sig = cursor.read_u32::<LittleEndian>()?;
    if sig != EOCD64_SIGNATURE {
        return Err(format!("Invalid EOCD64 signature: 0x{:08x}", sig).into());
    }

    cursor.read_u64::<LittleEndian>()?; // size_of_eocd64
    cursor.read_u16::<LittleEndian>()?; // version_made_by
    cursor.read_u16::<LittleEndian>()?; // version_needed
    cursor.read_u32::<LittleEndian>()?; // disk_number
    cursor.read_u32::<LittleEndian>()?; // disk_with_cd
    cursor.read_u64::<LittleEndian>()?; // entries_on_disk
    cursor.read_u64::<LittleEndian>()?; // total_entries
    let cd_size = cursor.read_u64::<LittleEndian>()?;
    let cd_offset = cursor.read_u64::<LittleEndian>()?;

    println!(
        "ZIP64 Central directory: offset={}, size={}",
        cd_offset, cd_size
    );

    fetch_and_parse_zip(client, url, cd_offset, cd_size).await
}

async fn handle_regular_zip(
    client: &Client,
    url: &str,
    cd_offset: u64,
    cd_size: u64,
) -> Result<Vec<Vec<String>>, Box<dyn std::error::Error>> {
    fetch_and_parse_zip(client, url, cd_offset, cd_size).await
}

pub fn zip_list_from_central_directory(
    cd_data: &[u8],
) -> Result<Vec<Vec<String>>, Box<dyn std::error::Error>> {
    use byteorder::{LittleEndian, ReadBytesExt};
    use std::io::Cursor;

    let mut cursor = Cursor::new(cd_data);
    let mut rows = vec![];

    const CENTRAL_FILE_HEADER_SIGNATURE: u32 = 0x02014b50;

    while cursor.position() < cd_data.len() as u64 {
        // Check if we have enough bytes for a header
        if cursor.position() + 46 > cd_data.len() as u64 {
            break;
        }

        let signature = cursor.read_u32::<LittleEndian>()?;

        if signature != CENTRAL_FILE_HEADER_SIGNATURE {
            // Reached end of central directory entries
            break;
        }

        cursor.read_u16::<LittleEndian>()?; // version made by
        cursor.read_u16::<LittleEndian>()?; // version needed
        cursor.read_u16::<LittleEndian>()?; // flags
        cursor.read_u16::<LittleEndian>()?; // compression method
        cursor.read_u16::<LittleEndian>()?; // last mod time
        cursor.read_u16::<LittleEndian>()?; // last mod date
        cursor.read_u32::<LittleEndian>()?; // crc32
        let compressed_size = cursor.read_u32::<LittleEndian>()?;
        cursor.read_u32::<LittleEndian>()?; // uncompressed size
        let filename_len = cursor.read_u16::<LittleEndian>()? as usize;
        let extra_len = cursor.read_u16::<LittleEndian>()? as usize;
        let comment_len = cursor.read_u16::<LittleEndian>()? as usize;
        cursor.read_u16::<LittleEndian>()?; // disk number start
        cursor.read_u16::<LittleEndian>()?; // internal file attributes
        cursor.read_u32::<LittleEndian>()?; // external file attributes
        let local_header_offset = cursor.read_u32::<LittleEndian>()?;

        // Read filename
        let mut filename_bytes = vec![0u8; filename_len];
        cursor.read_exact(&mut filename_bytes)?;
        let filename = String::from_utf8_lossy(&filename_bytes).to_string();

        // Skip extra field
        cursor.set_position(cursor.position() + extra_len as u64);

        // Skip comment
        cursor.set_position(cursor.position() + comment_len as u64);

        // Check for ZIP64 extra field if values are maxed out
        let mut actual_compressed_size = compressed_size as u64;
        let mut actual_offset = local_header_offset as u64;

        if compressed_size == 0xFFFFFFFF || local_header_offset == 0xFFFFFFFF {
            // Need to parse extra field for ZIP64 values
            // Go back and read extra field
            let current_pos = cursor.position();
            cursor.set_position(current_pos - comment_len as u64 - extra_len as u64);

            let extra_data = if extra_len > 0 {
                let mut extra = vec![0u8; extra_len];
                cursor.read_exact(&mut extra)?;
                extra
            } else {
                vec![]
            };

            // Parse ZIP64 extra field (0x0001)
            if let Some((size64, offset64)) = parse_zip64_extra_field(&extra_data) {
                if compressed_size == 0xFFFFFFFF {
                    actual_compressed_size = size64;
                }
                if local_header_offset == 0xFFFFFFFF {
                    actual_offset = offset64;
                }
            }

            // Restore position
            cursor.set_position(current_pos);
        }

        let row = vec![
            filename,
            format!("{}", actual_offset),
            format!("{}", actual_compressed_size),
        ];
        rows.push(row);
    }

    Ok(rows)
}

fn parse_zip64_extra_field(extra_data: &[u8]) -> Option<(u64, u64)> {
    use byteorder::{LittleEndian, ReadBytesExt};
    use std::io::Cursor;

    let mut cursor = Cursor::new(extra_data);

    while cursor.position() + 4 <= extra_data.len() as u64 {
        let header_id = cursor.read_u16::<LittleEndian>().ok()?;
        let data_size = cursor.read_u16::<LittleEndian>().ok()? as usize;

        if header_id == 0x0001 {
            // ZIP64 extended information
            let mut compressed_size = None;
            let mut offset = None;

            // The order is: uncompressed size, compressed size, relative header offset, disk start number
            // But we only get the fields that were 0xFFFFFFFF in the regular header

            if cursor.position() + 8 <= extra_data.len() as u64 {
                cursor.read_u64::<LittleEndian>().ok()?; // uncompressed size
            }
            if cursor.position() + 8 <= extra_data.len() as u64 {
                compressed_size = Some(cursor.read_u64::<LittleEndian>().ok()?);
            }
            if cursor.position() + 8 <= extra_data.len() as u64 {
                offset = Some(cursor.read_u64::<LittleEndian>().ok()?);
            }

            return Some((compressed_size.unwrap_or(0), offset.unwrap_or(0)));
        } else {
            // Skip this extra field
            cursor.set_position(cursor.position() + data_size as u64);
        }
    }

    None
}

async fn fetch_and_parse_zip(
    client: &Client,
    url: &str,
    cd_offset: u64,
    cd_size: u64,
) -> Result<Vec<Vec<String>>, Box<dyn std::error::Error>> {
    // Fetch the central directory
    let cd_range = format!("bytes={}-{}", cd_offset, cd_offset + cd_size - 1);
    println!("Requesting central directory range: {}", cd_range);

    let cd_response = client
        .get(url)
        .header(header::RANGE, cd_range)
        .send()
        .await?;

    let cd_bytes = cd_response.bytes().await?;
    println!("CD received {} bytes", cd_bytes.len());

    // Parse the central directory directly instead of using zip crate
    zip_list_from_central_directory(&cd_bytes)
}

use flate2::read::DeflateDecoder;

pub(crate) fn decompress_zip_entry(data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use byteorder::{LittleEndian, ReadBytesExt};
    use std::io::Cursor;

    let mut cursor = Cursor::new(data);

    // Parse local file header
    const LOCAL_FILE_HEADER_SIGNATURE: u32 = 0x04034b50;

    let signature = cursor.read_u32::<LittleEndian>()?;
    if signature != LOCAL_FILE_HEADER_SIGNATURE {
        return Err(format!("Invalid local file header signature: 0x{:08x}", signature).into());
    }

    cursor.read_u16::<LittleEndian>()?; // version needed
    cursor.read_u16::<LittleEndian>()?; // flags
    let compression_method = cursor.read_u16::<LittleEndian>()?;
    cursor.read_u16::<LittleEndian>()?; // last mod time
    cursor.read_u16::<LittleEndian>()?; // last mod date
    cursor.read_u32::<LittleEndian>()?; // crc32
    cursor.read_u32::<LittleEndian>()?; // compressed size
    cursor.read_u32::<LittleEndian>()?; // uncompressed size
    let filename_len = cursor.read_u16::<LittleEndian>()? as u64;
    let extra_len = cursor.read_u16::<LittleEndian>()? as u64;

    // Skip filename and extra field
    cursor.set_position(cursor.position() + filename_len + extra_len);

    // Now cursor is at the start of compressed data
    let compressed_data_start = cursor.position() as usize;
    let compressed_data = &data[compressed_data_start..];

    // Decompress based on compression method
    match compression_method {
        0 => {
            // Stored (no compression)
            Ok(compressed_data.to_vec())
        }
        8 => {
            // Deflate compression
            let mut decoder = DeflateDecoder::new(compressed_data);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;
            Ok(decompressed)
        }
        _ => Err(format!("Unsupported compression method: {}", compression_method).into()),
    }
}

pub(crate) fn parse_local_file_header(
    data: &[u8],
) -> Result<(u64, u64), Box<dyn std::error::Error>> {
    use byteorder::{LittleEndian, ReadBytesExt};
    use std::io::Cursor;

    let mut cursor = Cursor::new(data);

    const LOCAL_FILE_HEADER_SIGNATURE: u32 = 0x04034b50;

    let signature = cursor.read_u32::<LittleEndian>()?;
    if signature != LOCAL_FILE_HEADER_SIGNATURE {
        return Err(format!("Invalid local file header signature: 0x{:08x}", signature).into());
    }

    cursor.read_u16::<LittleEndian>()?; // version needed
    cursor.read_u16::<LittleEndian>()?; // flags
    cursor.read_u16::<LittleEndian>()?; // compression method
    cursor.read_u16::<LittleEndian>()?; // last mod time
    cursor.read_u16::<LittleEndian>()?; // last mod date
    cursor.read_u32::<LittleEndian>()?; // crc32
    cursor.read_u32::<LittleEndian>()?; // compressed size
    cursor.read_u32::<LittleEndian>()?; // uncompressed size
    let filename_len = cursor.read_u16::<LittleEndian>()? as u64;
    let extra_len = cursor.read_u16::<LittleEndian>()? as u64;

    Ok((filename_len, extra_len))
}

pub(crate) fn zip_list_from_local_file(
    file: &str,
) -> Result<Vec<Vec<String>>, Box<dyn std::error::Error>> {
    let reader = std::fs::File::open(file)?;
    let mut archive = zip::ZipArchive::new(reader)?;
    let mut rows = vec![];

    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        let filename = file.name().to_string();
        let compressed_size = file.compressed_size();
        let offset = file.data_start();

        rows.push(vec![
            filename,
            format!("{}", offset),
            format!("{}", compressed_size),
        ]);
    }

    Ok(rows)
}

pub(crate) fn zip_extract_from_local_file(
    file: &str,
    index: usize,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let reader = std::fs::File::open(file)?;
    let mut archive = zip::ZipArchive::new(reader)?;

    let mut file = archive.by_index(index)?;
    let mut contents = vec![];
    file.read_to_end(&mut contents)?;

    Ok(contents)
}
