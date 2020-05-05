#[macro_use]
extern crate clap;
use clap::{App, Arg, SubCommand};

#[macro_use]
extern crate failure;
use failure::Error;

use bincode;
use log::{error, info, warn};
use simplelog;
use std::io;
use std::path;

mod cli_utils;
mod file_processor;
mod geo_finder;

use chrono::offset::Local;

#[derive(Debug, Fail)]
pub enum MainError {
    #[fail(display = "Index Deserialization Error")]
    IndexDeserializationError,
}

fn save_finder(finder: &geo_finder::PolygonFinder, output_file: &path::Path) {
    let file_writer = std::fs::File::create(output_file).unwrap();
    let buf_writer = std::io::BufWriter::new(file_writer);
    bincode::serialize_into(buf_writer, &finder).unwrap();
}



fn load_polygons_finder(input_path: &path::Path) -> geo_finder::PolygonFinder {
    // TODO: error handling.
    let progress_bar = cli_utils::create_progress_bar_count(false, "Loading index...", None);
    progress_bar.enable_steady_tick(200);

    let file_reader = std::fs::File::open(input_path).unwrap();
    let buf_reader = std::io::BufReader::new(file_reader);
    let result = bincode::deserialize_from(buf_reader).unwrap();

    progress_bar.finish();
    result
}


fn create_polygons_geo_index<P: AsRef<path::Path>>(
    dest_path: P,
    geojson_path: P,
    force: bool,
) -> Result<(), Error> {
    info!("Generating index from geojson {:?} ...", geojson_path.as_ref());

    let mut dest_file_buffer = dest_path.as_ref().to_path_buf();
    if dest_path.as_ref().is_dir() {
        dest_file_buffer.set_file_name("geo.idx.bin");
    }
    let dest_file: &path::Path = dest_file_buffer.as_path();

    if dest_file.exists() && !force {
        warn!(
            "Index exist in {}. Skiping. Use --force to overwrite",
            dest_file.display()
        );
        return Ok(());
    }

    info!("Generating index into {} ...", dest_file.display());

    let finder_result = geo_finder::PolygonFinder::new(geojson_path);

    match finder_result {
        Ok(finder) => {
            info!("Saving index information into {}", dest_file.display());
            save_finder(&finder, dest_file);
        }
        Err(error) => {
            error!("Error creating geo index: {}", error);
        }
    };

    Ok(())
}



fn main() {
    let local_time = Local::now();
    let time_offset = local_time.offset();
    // Configure logging
    simplelog::TermLogger::init(
        simplelog::LevelFilter::Info,
        simplelog::Config {
            offset: time_offset.clone(),
            ..simplelog::Config::default()
        },
        simplelog::TerminalMode::Stderr,
    )
    .ok();

    match do_main() {
        Ok(_) => info!("Process finished OK"),
        Err(err) => {
            error!("Process finished with an error: {}", err);
            std::process::exit(1);
        }
    };
}

fn run_polygons_classifier(
    index_file_path: &path::Path,
    input_file: &mut io::Read,
    file_size: Option<u64>,
    output_file: &mut io::Write,
    char_delimiter: u8,
    latitude_idx: usize,
    longitude_idx: usize,
    properties: Vec<&str>,
    no_header: bool,
) -> Result<(), Error> {
    info!("Loading index from '{}'.", index_file_path.display());
    let geo_index = load_polygons_finder(&index_file_path);
    info!("Index from '{}' loaded.", index_file_path.display());

    let process_result = file_processor::spatial_polygons_join(
        geo_index,
        input_file,
        file_size,
        output_file,
        char_delimiter,
        latitude_idx,
        longitude_idx,
        properties,
        no_header,
    );

    match process_result {
        Ok(stats) => {
            info!("Stats: {:?}", stats);
            return Ok(());
        }
        Err(err) => return Err(Error::from(err)),
    }
}

fn do_main() -> Result<(), Error> {
    let matches = App::new("locate_points")
                    .version("0.1.0")
                    .author("Gustavo Ajzenman")
                    .about("Spatial join with a Geojson")
                    .subcommand(
                        SubCommand::with_name("generate_index")
                            .about("Generate Geolocation index file from geojsons")
                            .arg(Arg::with_name("output")
                                .short("o")
                                .help("Output path or file for the generated index")
                                .takes_value(true)
                                .default_value(".")
                            )
                            .arg(Arg::with_name("force")
                                .short("f")
                                .long("force")
                                .help("Overwrite indexes")
                                .takes_value(false)
                            )
                            .arg(Arg::with_name("geojson")
                                .short("g")
                                .required(true)
                                .help("Path for the geojson file")
                                .takes_value(true)
                            )
                    )
    
                    .subcommand(
                        SubCommand::with_name("run")
                            .about("Run spatial join with index file")
                            .arg(Arg::with_name("output")
                                    .short("o")
                                    .long("output")
                                    .help("Sets the output file to create.")
                                    .takes_value(true)
                                    .required(false)
                            )
                            .arg(Arg::with_name("index")
                                .short("x")
                                .long("index")
                                .help("Sets the index file or directory to use")
                                .takes_value(true)
                                .default_value("geo.idx.bin")
                                .required(false)
                            )
                            .arg(Arg::with_name("input")
                                .short("i")
                                .long("input")
                                .help("Sets the input file to use (must have 'latitude' and 'longitude' fields). If omitted, stdin will be used.")
                                .takes_value(true)
                            )
                            .arg(Arg::with_name("delimiter")
                                .short("d")
                                .long("delimiter")
                                .help("Delimiter for the CSV fields")
                                .takes_value(true)
                                .required(false)
                                .default_value("\t"),
                            )
                            .arg(Arg::with_name("latitude")
                                .long("latitude")
                                .help("Sets the column number that contains the latitude. 1 based.")
                                .takes_value(true)
                                .required(true)
                            )
                            .arg(Arg::with_name("longitude")
                                .long("longitude")
                                .help("Sets the column number that contains the longitude. 1 based.")
                                .takes_value(true)
                                .required(true)
                            )
                            .arg(Arg::with_name("no-header")
                                 .long("no-header")
                                 .help("Specifies that this CSV file does not contain a header")
                                )
                            .arg(Arg::with_name("properties")
                                .multiple(true)
                                .takes_value(true)
                                .required(true)
                                .long("properties")
                                .short("p")
                            )
                    )
                    .get_matches();

    if let Some(generate_matches) = matches.subcommand_matches("generate_index") {
        return create_polygons_geo_index(
            generate_matches.value_of("output").unwrap_or_default(),
            generate_matches.value_of("geojson").unwrap_or_default(),
            generate_matches.is_present("force"),
        );
    }

    if let Some(run_matches) = matches.subcommand_matches("run") {
        let properties: Vec<_> = run_matches.values_of("properties").unwrap().collect();
        let input_file_path = run_matches.value_of("input");
        let index_path = run_matches.value_of("index").unwrap_or_default();

        // 1 based.
        let latitude_idx = value_t!(run_matches, "latitude", usize).unwrap() - 1;
        let longitude_idx = value_t!(run_matches, "longitude", usize).unwrap() - 1;

        // Parse the delimiter. Should be exactly one character.
        let delimiter = run_matches
            .value_of("delimiter")
            .unwrap_or_default()
            .replace("\\t", "\t");
        let char_delimiter: u8 = delimiter.as_bytes()[0];
        info!("Using the following delimiter: {:?}", char_delimiter);

        let no_header = run_matches.is_present("no-header");

        let stdin = io::stdin();
        let (mut input_file, input_file_size): (Box<io::Read>, Option<u64>) = match input_file_path
        {
            Some(path) => {
                let input_file = std::fs::File::open(path)?;
                let file_size = input_file.metadata()?.len();
                // let estimated_size = estimate_row_count(&mut input_file)?;
                (Box::new(input_file), Some(file_size))
            }
            None => {
                info!("Reading from stdin");
                (Box::new(stdin.lock()), None)
            }
        };


        let output_file_path = run_matches.value_of("output");

        let stdout = io::stdout();
        let mut output_file: Box<io::Write> = match output_file_path
        {
            Some(path) => {
                info!("Writing to file {}.", path);

                let output_file: Box<io::Write> = Box::new(std::fs::File::create(path)?);
                Box::new(output_file)
            }
            None => {
                info!("Reading from stdin");
                Box::new(stdout.lock())
            }
        };



        return run_polygons_classifier(
                path::Path::new(index_path),
                input_file.as_mut(),
                input_file_size,
                output_file.as_mut(),
                char_delimiter,
                latitude_idx,
                longitude_idx,
                properties,
                no_header,
            );
    }

    return Ok(());
}
