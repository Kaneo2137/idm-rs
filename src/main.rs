mod lib;
use clap::{arg, Command};
use idm_rs::{Config, Downloader};

fn main() {
    let matches = Command::new("idm-rs")
        .author("Konrad Nowakowski")
        .version("1.0.0")
        .about("Simple download manager written in rust.")
        .arg(arg!(<URL> "URL pointing to the file."))
        .arg(
            arg!(-c --connections <num> "Number of connections to the server.")
                .required(false),
        )
        .arg(
            arg!(-t --threads <num> "Number of threads used for download.")
                .required(false),
        )
        .arg(arg!(-s --singlethreaded "Downloads file using single thread."))
        .arg(arg!(-u --useragent "Sets user agent string."))
        .arg(arg!(-f --filename "Sets name of the file."))
        .get_matches();

    let url = matches.value_of("URL").unwrap();

    let mut configbuilder = Config::builder(url);

    if matches.is_present("singlethreaded") {
        configbuilder.singlecore(true);
    }
    if matches.is_present("connections") {
        configbuilder.connection_number(matches.value_of("connections").unwrap().parse().unwrap());
    }
    if matches.is_present("threads") {
        configbuilder.thread_number(matches.value_of("threads").unwrap().parse().unwrap());
    }
    if matches.is_present("filename") {
        configbuilder.filename(matches.value_of("filename").unwrap().to_string());
    }
    if matches.is_present("useragent") {
        configbuilder.user_agent(matches.value_of("useragent").unwrap().to_string());
    }

    let _downloader = Downloader::from_config(configbuilder.build()).download();

}
