#![allow(dead_code)]

use std::{
    fs::File,
    io::{Read, Seek, Write},
    sync::mpsc,
    vec,
};

use threadpool::ThreadPool;

pub struct ConfigBuilder {
    url: reqwest::Url,
    connections: u8,
    threads: u8,
    singlecore: Option<bool>,
    filename: Option<String>,
    user_agent: Option<String>,
}

impl ConfigBuilder {
    pub fn new(url: &str) -> ConfigBuilder {
        ConfigBuilder {
            url: reqwest::Url::parse(url).unwrap(),
            connections: 8,
            threads: num_cpus::get() as u8,
            singlecore: None,
            filename: None,
            user_agent: None,
        }
    }

    pub fn connection_number(&mut self, num: u8) -> &mut ConfigBuilder {
        self.connections = num;
        self
    }

    pub fn thread_number(&mut self, num: u8) -> &mut ConfigBuilder {
        self.threads = num;
        self
    }

    pub fn singlecore(&mut self, val: bool) -> &mut ConfigBuilder {
        self.singlecore = Some(val);
        self
    }

    pub fn filename(&mut self, fname: String) -> &mut ConfigBuilder {
        self.filename = Some(fname);
        self
    }

    pub fn user_agent(&mut self, agent: String) -> &mut ConfigBuilder {
        self.user_agent = Some(agent);
        self
    }

    pub fn build(self) -> Config {
        Config {
            url: self.url,
            connections: self.connections,
            threads: self.threads,
            singlecore: self.singlecore,
            filename: self.filename,
            user_agent: self.user_agent,
        }
    }
}

#[derive(Debug)]
pub struct Config {
    url: reqwest::Url,
    connections: u8,
    threads: u8,
    singlecore: Option<bool>,
    filename: Option<String>,
    user_agent: Option<String>,
}

impl Config {
    pub fn new(url: &str) -> Config {
        Config {
            url: reqwest::Url::parse(url).unwrap(),
            connections: 8,
            threads: num_cpus::get() as u8,
            singlecore: None,
            filename: None,
            user_agent: None,
        }
    }

    pub fn builder(url: &str) -> ConfigBuilder {
        ConfigBuilder::new(url)
    }
}

pub struct Downloader {
    config: Config,
}

impl Downloader {
    pub fn from_url(url: &str) -> Downloader {
        Downloader {
            config: Config::new(url),
        }
    }

    pub fn from_config(config: Config) -> Downloader {
        Downloader { config }
    }

    pub fn download(self) {
        use reqwest::header::*;

        let mut headers = HeaderMap::default();
        headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
        headers.insert(CONNECTION, HeaderValue::from_static("keep-alive"));

        let client = reqwest::blocking::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap();

        let test_req = client.get(self.config.url.clone()).send().unwrap();

        if test_req
            .headers()
            .contains_key(reqwest::header::TRANSFER_ENCODING)
        {
            println!("{:?}", test_req.headers());
            todo!("Implement stuff related to TRANSFER_ENCODING header");
        }

        let filename = self.config.filename.unwrap_or_else(|| {
            if test_req
                .headers()
                .contains_key(reqwest::header::CONTENT_DISPOSITION)
            {
                String::try_from(
                    test_req.headers()[reqwest::header::CONTENT_DISPOSITION]
                        .to_str()
                        .unwrap()
                        .split('=')
                        .last()
                        .unwrap()
                        .replace(&['\'', '"'], ""),
                )
                .unwrap()
            } else {
                test_req
                    .url()
                    .to_string()
                    .split('/')
                    .last()
                    .unwrap()
                    .split('?')
                    .next()
                    .unwrap()
                    .to_string()
            }
        });

        let content_length = test_req.content_length().unwrap();

        let threads = ThreadPool::new(self.config.threads.into());

        let (tx, rx) = mpsc::channel();
        {
            let chunk_size;
            if self.config.singlecore.unwrap_or_else(|| false) || self.config.connections == 1 {
                chunk_size = content_length;
            } else {
                chunk_size = (content_length + self.config.connections as u64 - 1) / self.config.connections as u64;
            }

            for i in 0..self.config.connections as u64 {
                let tx = tx.clone();
                let range = (chunk_size * i, chunk_size * (i + 1) - 1);
                let range_text = format!("bytes={}-{}", range.0, range.1);
                let url = self.config.url.clone();
                let req = client.get(url).header(RANGE, range_text);
                threads.execute(move || {
                    Downloader::download_chunk(tx, req, range);
                });
            }
        }

        let mut file = File::create(filename).unwrap();
        file.write(vec![0; content_length as usize].as_slice())
            .unwrap();
        let mut size: isize = content_length as isize;

        while size > 0 {
            let chunk = rx.recv().unwrap();
            size -= chunk.byte_count as isize;

            file.seek(std::io::SeekFrom::Start(chunk.offset)).unwrap();
            file.write_all(&chunk.data).unwrap();
            file.flush().unwrap();

            println!("{}", size);
        }
    }

    fn download_chunk(
        tx: mpsc::Sender<Chunk>,
        req: reqwest::blocking::RequestBuilder,
        mut range: (u64, u64),
    ) {
        let size = range.1 - range.0;
        let mut req = req.send().unwrap();
        loop {
            let mut data = vec![0u8; size as usize];
            let byte_count = req.read(data.as_mut_slice()).unwrap();
            data.truncate(byte_count);
            if data.is_empty() {
                break;
            }
            data.truncate(byte_count);
            tx.send(Chunk {
                data,
                byte_count: byte_count as u64,
                offset: range.0,
            })
            .unwrap();

            range.0 += byte_count as u64;
        }
    }
}

struct Chunk {
    byte_count: u64,
    offset: u64,
    data: Vec<u8>,
}
