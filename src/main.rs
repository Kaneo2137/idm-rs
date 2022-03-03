mod lib;
use clap::{arg, Command};
use idm_rs::{Config, Downloader};

/*
 *  TODO:
 *  - Wyodrębnij cały proces pobierania do oddzielnej struktury
 *  - Obecnie sposób pobierania jest trochę hacky, próba z wieloma wątkami i chunki?
 *  - Jakiś sposób konfiguracji z urla i opcji, aby łatwiej korzystało się z biblioteki
 */

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

    return;

/*
    let start = std::time::Instant::now();

    let mut requests = vec![];

    for offset in offsets {
        let req = client.get(url).header(RANGE, offset);
        let fut = tokio::task::spawn(download_chunk(req));
        requests.push(fut);
    }

    let results =
        thread::spawn(move || futures::executor::block_on(futures::future::join_all(requests)))
            .join()
            .unwrap();

    //let results = futures::future::join_all(requests).await;

    let mut file = vec![];
    for result in results {
        let res: Result<Vec<u8>, reqwest::Error> = result.unwrap();
        file.append(&mut res.unwrap());
    }

    println!(
        "Elapsed since download start: {:?}",
        std::time::Instant::now().duration_since(start)
    );

    let mut out = File::create(filename).expect("failed to create file");
    out.write(&file).unwrap();
*/
}

//async fn download_chunk(req: RequestBuilder) -> Result<Vec<u8>, reqwest::Error> {
//    let bytes = req.send().await?.bytes().await?;
//    Ok(bytes.to_vec())
//}
