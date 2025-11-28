use std::{error::Error, fs::File};

use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

use crate::utils::print_rows;

pub fn parquet_view(path: String, max_rows: usize) -> Result<(), Box<dyn Error>> {
    let file = File::open(&path)?;
    let parquet_reader = ParquetRecordBatchReaderBuilder::try_new(file)?
        .with_batch_size(8192)
        .with_limit(10)
        .build()?;

    let mut batches = Vec::new();

    for batch in parquet_reader {
        batches.push(batch?);
    }

    let mut rows = vec![];
    let mut col_max_len = vec![];
    for batch in batches.iter() {
        let mut i = 0;
        loop {
            let mut row = vec![];
            if i >= batch.num_rows() {
                break;
            }
            if i > max_rows {
                break;
            }
            let mut colno = 0;
            for f in batch.schema().fields() {
                let name = f.name().clone();
                if i == 0 {
                    let l = name.len();
                    row.push(name);
                    col_max_len.push(l);
                } else {
                    if let Some(column) = batch.column_by_name(&name) {
                        let value =
                            arrow::util::display::array_value_to_string(column.as_ref(), i - 1)
                                .expect("can't display value");
                        let l = value.len();
                        row.push(value);
                        if colno < col_max_len.len() && col_max_len[colno] < l {
                            col_max_len[colno] = l;
                        }
                        colno += 1;
                    }
                }
            }
            i += 1;
            rows.push(row);
        }
    }

    print_rows(&rows, &col_max_len, true);
    Ok(())
}
