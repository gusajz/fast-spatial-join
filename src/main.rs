#[macro_use]
extern crate clap;
use clap::{App, Arg, SubCommand, ArgGroup};

#[macro_use]
extern crate failure;
use failure::Error;

use log::{error, info, warn};
use simplelog;
use std::io;
use std::path::{Path};

mod cli_utils;
mod file_processor;
mod geo_finder;

mod geo_index_utils;

use chrono::offset::Local;
// use test::run_tests;

#[derive(Debug, Fail)]
pub enum MainError {
    #[fail(display = "Index Deserialization Error")]
    IndexDeserializationError,
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



fn create_geo_index_command<P: AsRef<Path>>(
    dest_path: P,
    geo_json_path: P,
    force: bool,
) -> Result<(), Error> {
    info!("Generating index from geo-json {:?} ...", geo_json_path.as_ref());

    let mut dest_file_buffer = dest_path.as_ref().to_path_buf();
    if dest_path.as_ref().is_dir() {
        dest_file_buffer.set_file_name("geo.idx.bin");
    }
    let dest_file: &Path = dest_file_buffer.as_path();

    if dest_file.exists() && !force {
        warn!(
            "Index exist in {}. Skipping. Use --force to overwrite",
            dest_file.display()
        );
        return Ok(());
    }

    info!("Generating index into {} ...", dest_file.display());

    let finder_result = geo_index_utils::create_geo_index(geo_json_path);

    match finder_result {
        Ok(finder) => {
            info!("Saving index information into {}", dest_file.display());
            geo_index_utils::save_geo_index(&finder, dest_file);
        }
        Err(error) => {
            error!("Error creating geo index: {}", error);
        }
    };

    Ok(())
}

fn join_command(
    geo_index: &geo_finder::PolygonFinder,
    input_file: &mut io::Read,
    file_size: Option<u64>,
    output_file: &mut io::Write,
    char_delimiter: u8,
    latitude_idx: usize,
    longitude_idx: usize,
    properties: Vec<&str>,
    no_header: bool,
    write_status: bool
) -> Result<(), Error> {
    // info!("Loading index from '{}'.", index_file_path.display());
    // let geo_index = geo_index_utils::load_geo_index(&index_file_path)?;
    // info!("Index from '{}' loaded.", index_file_path.display());

    let process_result = file_processor::spatial_polygons_join(
        &geo_index,
        input_file,
        file_size,
        output_file,
        char_delimiter,
        latitude_idx,
        longitude_idx,
        properties,
        no_header,
        write_status
    );

    return match process_result {
        Ok(stats) => {
            info!("Stats: {:?}", stats);
            Ok(())
        }
        Err(err) => Err(Error::from(err)),
    }
}

fn do_main() -> Result<(), Error> {
    let matches = App::new("locate_points")
                    .version("0.1.0")
                    .author("Gustavo Ajzenman")
                    .about("Spatial join with a geo-json file")
                    .subcommand(
                        SubCommand::with_name("generate_index")
                            .about("Generate Geolocation index file from geo-json")
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
                            .arg(Arg::with_name("geo-json")
                                .short("g")
                                .required(true)
                                .help("Path for the geo-json file")
                                .takes_value(true)
                            )
                    )
    
                    .subcommand(
                        SubCommand::with_name("join")
                            .about("Run spatial join")
                            .arg(Arg::with_name("output")
                                    .short("o")
                                    .long("output")
                                    .help("Sets the output file to create.")
                                    .takes_value(true)
                                    .required(false)
                            )
                            .group(ArgGroup::with_name("geo-file-arg")
                                .args(&["index", "geo-file"])
                                .required(true))
                            .arg(Arg::with_name("index")
                                .short("x")
                                .long("index")
                                .help("Use an index instead of a geo-json file.")
                                .takes_value(true)
                            )
                            .arg(Arg::with_name("geo-file")
                                .short("g")
                                .long("geo-file")
                                .help("Path for the geo-json file. Index will be generated on the fly.")
                                .takes_value(true)
                            )
                            .arg(Arg::with_name("input")
                                .short("i")
                                .long("input")
                                .help("Sets the input file to use. If omitted, stdin will be used.")
                                .takes_value(true)
                            )
                            .arg(Arg::with_name("delimiter")
                                .short("d")
                                .long("delimiter")
                                .help("Delimiter for input file fields")
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
                            .arg(Arg::with_name("with-header")
                                 .long("with-header")
                                 .help("Specifies that the input file contains a header.")
                                )
                            .arg(Arg::with_name("write-join-status")
                                .long("write-join-status")
                                .help("Write an extra column with indicating whether the join succeed or not for each row.")
                            )
                            .arg(Arg::with_name("properties")
                                .multiple(true)
                                .takes_value(true)
                                .required(true)
                                .help("Propertied of the geo-json or index file to use.")
                                .long("properties")
                                .short("p")
                            )
                    )
                    .get_matches();

    if let Some(generate_matches) = matches.subcommand_matches("generate_index") {
        return create_geo_index_command(
            generate_matches.value_of("output").unwrap_or_default(),
            generate_matches.value_of("geo-json").unwrap_or_default(),
            generate_matches.is_present("force"),
        );
    }

    if let Some(run_matches) = matches.subcommand_matches("join") {

        let properties: Vec<_> = run_matches.values_of("properties")
            .unwrap_or_default()
            .collect();


        let input_file_path = run_matches.value_of("input");

        let geo_index = if run_matches.is_present("index") {
            let geo_index_path = Path::new(
                run_matches.value_of("index").expect("index"));

            geo_index_utils::load_geo_index(geo_index_path)?
        } else if run_matches.is_present("geo-file") {
            let geo_json_path = Path::new(
                run_matches.value_of("geo-file").expect("geo-file"));

            geo_index_utils::create_geo_index(geo_json_path)?
        } else {
            error!("Either geo-file or index must be indicated.");
            std::process::exit(1)
        };


        // 1 based.
        let latitude_idx = value_t!(run_matches, "latitude", usize).expect("latitude") - 1;
        let longitude_idx = value_t!(run_matches, "longitude", usize).expect("longitude") - 1;

        // Parse the delimiter. Should be exactly one character.
        let delimiter = run_matches
            .value_of("delimiter")
            .unwrap_or_default()
            .replace("\\t", "\t");
        let char_delimiter: u8 = delimiter.as_bytes()[0];
        info!("Using the following delimiter: {:?}", char_delimiter);

        let no_header = !run_matches.is_present("with-header");

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

        let write_status = run_matches.is_present("write-join-status");

        return join_command(
                &geo_index,
                input_file.as_mut(),
                input_file_size,
                output_file.as_mut(),
                char_delimiter,
                latitude_idx,
                longitude_idx,
                properties,
                no_header,
                write_status
            );
    }

    return Ok(());
}
