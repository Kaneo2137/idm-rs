#![allow(dead_code)]

use std::{
    fs::File,
    io::{Read, Seek, Write},
    path::Path,
    sync::mpsc,
    vec,
};

use reqwest::{
    blocking::{Client, Response},
    header::*,
};
use threadpool::ThreadPool;

pub struct ConfigBuilder {
    url: reqwest::Url,
    connections: usize,
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

    pub fn connection_number(&mut self, num: usize) -> &mut ConfigBuilder {
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
    connections: usize,
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

    pub fn download(&self) {
        let mut headers = HeaderMap::default();
        headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
        headers.insert(CONNECTION, HeaderValue::from_static("keep-alive"));
        {
            let agent = self
                .config
                .user_agent
                .clone()
                .unwrap_or_else(|| "idm-rs/1.0.0".to_string());
            headers.insert(USER_AGENT, HeaderValue::from_str(agent.as_str()).unwrap());
        }

        let client = reqwest::blocking::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap();

        let initial_req = client.get(self.config.url.clone()).send().unwrap();

        if self.config.singlecore.unwrap_or_else(|| false)
            || initial_req.headers().contains_key(TRANSFER_ENCODING)
            || !initial_req.headers().contains_key(CONTENT_LENGTH)
        {
        let filename: String = self
            .config
            .filename
            .clone()
            .unwrap_or_else(|| Downloader::get_filename(&initial_req));

            self.singlecore_download(filename, client);
        } else {
            self.multicore_download(initial_req, client);
        }
    }

    fn singlecore_download(&self, filename: String, client: Client) {
        let mut file = File::create(filename).unwrap();
        let mut resp = client.get(self.config.url.as_ref()).send().unwrap();
        let mut buf = vec![0; 10000];
        loop {
            let bytes = resp.read(&mut buf).unwrap();
            if bytes == 0 {
                break;
            }

            file.write(&buf).unwrap();
        }
    }

    fn multicore_download(&self, resp: Response, client: Client) {
        let content_length = resp.content_length().unwrap() as usize;
        let chunk_size = (content_length + self.config.connections - 1) / self.config.connections;

        let (sender, receiver) = mpsc::channel();
        let threadpool = ThreadPool::new(self.config.threads.into());

        for i in 0..self.config.connections {
            let tx = sender.clone();
            let range = (chunk_size * i, chunk_size * (i + 1) - 1);
            let req = client.get(self.config.url.as_ref());
            threadpool.execute(move || {
                Downloader::download_chunk(tx, req, range);
            });
        }

        let filename: String = self
            .config
            .filename
            .clone()
            .unwrap_or_else(|| Downloader::get_filename(&resp));

        let mut file = File::create(filename).unwrap();
        file.write(vec![0; content_length].as_slice()).unwrap();
        let mut size = content_length;

        while size != 0 {
            let chunk = receiver.recv().unwrap();
            size -= chunk.byte_count;

            file.seek(std::io::SeekFrom::Start(chunk.offset as u64))
                .unwrap();
            file.write_all(&chunk.data).unwrap();
            file.flush().unwrap();
        }
    }

    fn get_filename(req: &Response) -> String {
        let mut filename;
        if req
            .headers()
            .contains_key(reqwest::header::CONTENT_DISPOSITION)
        {
            filename = String::try_from(
                req.headers()[reqwest::header::CONTENT_DISPOSITION]
                    .to_str()
                    .unwrap()
                    .split('=')
                    .last()
                    .unwrap()
                    .replace(&['\'', '"'], ""),
            )
            .unwrap()
        } else {
            filename = req
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

        while Path::new(&filename).exists() {
            let mut name = format!(
                "{}_new",
                filename.split(".").next().expect("thats a weird file")
            );
            // making sure that things like file.tar.gz get renamed properly
            for (index, word) in filename.split(".").into_iter().enumerate() {
                if index == 0 {
                    continue;
                }
                name = name + "." + &word.to_owned();
            }
            filename = name;
        }

        filename
    }

    fn download_chunk(
        tx: mpsc::Sender<Chunk>,
        req: reqwest::blocking::RequestBuilder,
        mut range: (usize, usize),
    ) {
        let mut req = req
            .header(RANGE, format!("bytes={}-{}", range.0, range.1))
            .send()
            .unwrap();
        let size = range.1 - range.0;
        loop {
            let mut data = vec![0u8; size];
            let byte_count = req.read(data.as_mut_slice()).unwrap();
            if byte_count == 0 {
                break;
            }
            data.truncate(byte_count as usize);
            tx.send(Chunk {
                data,
                byte_count,
                offset: range.0,
            })
            .unwrap();

            range.0 += byte_count;
        }
    }
}

struct Chunk {
    byte_count: usize,
    offset: usize,
    data: Vec<u8>,
}
