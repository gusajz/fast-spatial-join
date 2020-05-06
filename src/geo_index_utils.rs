use std::path;

use super::cli_utils;

use super::geo_finder;
use bincode::ErrorKind;


pub fn load_geo_index(input_path: &path::Path) -> Result<geo_finder::PolygonFinder, Box<ErrorKind>> {
    // TODO: error handling.
    let progress_bar = cli_utils::create_progress_bar_count(false, "Loading index...", None);
    progress_bar.enable_steady_tick(200);

    let file_reader = std::fs::File::open(input_path)?;
    let buf_reader = std::io::BufReader::new(file_reader);
    let result = bincode::deserialize_from(buf_reader);


    progress_bar.finish();
    result
}


pub fn save_geo_index(finder: &geo_finder::PolygonFinder, output_file: &path::Path) {
    let file_writer = std::fs::File::create(output_file).unwrap();
    let buf_writer = std::io::BufWriter::new(file_writer);
    bincode::serialize_into(buf_writer, &finder).unwrap();
}

pub fn create_geo_index<P: AsRef<path::Path>>(
    geo_json_path: P
) -> Result<geo_finder::PolygonFinder, geo_finder::PolygonFinderError> {
    geo_finder::PolygonFinder::new(geo_json_path)
}
