use super::cli_utils;
use super::geo_finder;

use csv;
use std::io;
use std::time;

use log::{info, warn};

use failure::Fail;
// use std::error::Error;

const DEFAULT_PROPERTY_VALUE: &'static str = "-";

#[derive(Debug)]
pub struct ProcessStats {
    pub total_lines: u32,
    pub error_lines: u32,
}

#[allow(dead_code)]
#[derive(Debug, Fail)]
pub enum FileProcessorError {
    #[fail(display = "invalid toolchain name:")]
    OtherError,
    #[fail(display = "I/O error: {}", _0)]
    Io(io::Error),
    #[fail(display = "Csv error: {}", _0)]
    Csv(csv::Error),
}

#[inline]
fn record_size(record: &csv::StringRecord) -> u64 {
    use std::convert::TryInto;
    let size: usize = record.iter().map(|e| e.len()).sum();
    return size.try_into().unwrap();;
}

#[inline]
fn fill_error_row(
    properties: &Vec<&str>,
    _err_message: &str,
    new_record: &mut csv::StringRecord,
) {
    for _ in 0..properties.len() {
        new_record.push_field("");
    }
    new_record.push_field("error"); // Status
    // new_record.push_field(err_message); // Error message.
}


pub fn spatial_polygons_join(
    geo_finder: &geo_finder::PolygonFinder,
    input_file: &mut io::Read,
    file_size: Option<u64>,
    output_file: &mut io::Write,
    delimiter: u8,
    latitude_idx: usize,
    longitude_idx: usize,
    properties: Vec<&str>,
    no_header: bool,
    write_status: bool
) -> Result<ProcessStats, FileProcessorError> {
    let progress_bar = cli_utils::create_progress_bar_bytes(false, "Processing...", file_size);

    let mut csv_reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false) // Don't care about the headers now.
        .flexible(true)
        // .quote(b'"')
        // .double_quote(false)    // "" instead of \" to escape quotes
        .from_reader(input_file);

    let mut csv_writer = csv::WriterBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .from_writer(output_file);

    let mut total_lines = 0;
    let mut error_lines = 0;

    let start_instant = time::Instant::now();

    let mut records = csv_reader.records();

    let has_header = !no_header;
    if has_header {
        // If the file has a header, process it first and append the columns we want
        if let Some(Ok(header)) = records.next() {
            let mut new_header: Vec<String> = header.iter().map(String::from).collect();

            for property in properties.iter() {
                new_header.push(String::from(*property));
            }


            if write_status {
                new_header.push("status".to_owned());
            }
            // new_header.push("error_message".to_owned());

            csv_writer.write_record(new_header).ok();
        }
    }

    for (line_number, record_result) in csv_reader.records().enumerate() {
        total_lines += 1;

        match record_result {
            Err(e) => {
                warn!("Unable to read line {}: {}", line_number, e);
                error_lines += 1;
            }
            Ok(record) => {

                let mut new_record = record.clone();

                let latitude_opt = record.get(latitude_idx).and_then(|v| v.parse::<f64>().ok());

                let longitude_opt = record
                    .get(longitude_idx)
                    .and_then(|v| v.parse::<f64>().ok());

                if latitude_opt.is_none() || longitude_opt.is_none() {
                    error_lines += 1;
                    fill_error_row(
                        &properties,
                        &format!("INVALID_COORDINATES: {:?}", (latitude_opt, longitude_opt)),
                        &mut new_record,
                    );
                } else {
                    let latitude = latitude_opt.unwrap();
                    let longitude = longitude_opt.unwrap();

                    match geo_finder.find(latitude, longitude) {
                        Some(find_result) => {
                            // info!("Props: {:?}", find_result.props);
                            for prop in &properties {
                                let value: &str = match find_result.props.get(prop as &str) {
                                    Some(r) => r,
                                    None => &DEFAULT_PROPERTY_VALUE
                                }; //TODO: proper error handling

                                new_record.push_field(&value);
                            }

                            if write_status {
                                new_record.push_field("success"); // Status
                            }
                            // new_record.push_field(""); // Error message
                        }
                        None => {
                            error_lines += 1;
                            if write_status {
                                fill_error_row(
                                    &properties,
                                    &format!("COORDINATES_NOT_FOUND: {:?}", (latitude, longitude)),
                                    &mut new_record,
                                )
                            }
                        }
                    }
                }

                // new_record.push_field("\n");
                // warn!("New record {:?}", new_record);
                let write_result = csv_writer.write_record(&new_record);

                progress_bar.inc(record_size(&record));

                if write_result.is_err() {
                    break ;
                    // warn!("Error writing row: {}", write_result.err().unwrap())
                }
            }
        };
    }

    #[allow(unused_must_use)] {
        csv_writer
            .flush();
    }

    progress_bar.finish();

    let end_instant = time::Instant::now();
    let elapsed_secs = (end_instant - start_instant).as_millis() as f32 / 1000.0f32;
    info!(
        "Processed {} rows of data in {} seconds. Avg: {} rows/sec",
        total_lines,
        elapsed_secs,
        (total_lines as f32) / elapsed_secs
    );

    return Ok(ProcessStats {
        total_lines,
        error_lines,
    });
}
