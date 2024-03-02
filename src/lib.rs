use std::{
    sync::mpsc::{Receiver, Sender},
    thread::{self, JoinHandle},
};

use url::Url;

use reqwest::blocking::get;
use scraper::{Html, Selector};

pub enum Message {
    DONE(u8, String, usize),
    WAITING(u8),
    BUSY(u8, String),
    ERROR(u8, String),
    CONTENT(u8, String, String),
}

#[derive(PartialEq, Debug)]
pub enum WorkerStatus {
    WAITING,
    BUSY(String),
    ERROR(String),
}

#[derive(PartialEq)]
pub enum JobStatus {
    INPROGRESS(u8),
    QUEUED,
}

pub struct Worker {
    pub id: u8,
    pub status: WorkerStatus,
    pub handle: JoinHandle<()>,
    pub channel: Sender<Message>,
}

impl Worker {
    pub fn new(
        id: u8,
        channel: Sender<Message>,
        message_transmitter: Sender<Message>,
        message_receiver: Receiver<Message>,
    ) -> Worker {
        let worker_id = id.clone();
        message_transmitter
            .clone()
            .send(Message::WAITING(id.clone()))
            .unwrap();
        let handle = thread::spawn(move || loop {
            let message = match message_receiver.recv() {
                Ok(message) => message,
                Err(_) => break,
            };

            let job_content = match message {
                Message::DONE(..) => break,
                Message::WAITING(_) => continue,
                Message::BUSY(..) => continue,
                Message::ERROR(..) => break,
                Message::CONTENT(worker_id, root, url) => (worker_id, root, url),
            };

            let new_urls = match fetch_page(job_content.1, job_content.2.clone()) {
                Ok(urls) => urls,
                Err(_) => {
                    message_transmitter
                        .send(Message::ERROR(worker_id.clone(), job_content.2.clone()))
                        .unwrap();
                    break;
                }
            };

            for url in new_urls.0 {
                message_transmitter
                    .send(Message::CONTENT(
                        worker_id.clone(),
                        job_content.2.clone(),
                        url,
                    ))
                    .unwrap();
            }
            message_transmitter
                .send(Message::DONE(
                    worker_id.clone(),
                    job_content.2.clone(),
                    new_urls.1,
                ))
                .unwrap();
        });

        Worker {
            id,
            status: WorkerStatus::WAITING,
            channel,
            handle,
        }
    }
}

fn is_same_host(root: &String, url: String) -> bool {
    let parsed_url = match Url::parse(&url) {
        Ok(parsed) => parsed,
        Err(_) => return false,
    };

    match Url::parse(&root) {
        Ok(url) => url.host_str().unwrap() == parsed_url.host_str().unwrap(),
        Err(_) => false,
    }
}

fn fetch_page(root: String, url: String) -> Result<(Vec<String>, usize), String> {
    let body_result = match get(url.clone()) {
        Ok(body) => body.text(),
        Err(error) => {
            println!("Error getting url: {}", error);
            return Err(error.to_string());
        }
    };

    let body = match body_result {
        Ok(body) => body,
        Err(error) => return Err(error.to_string()),
    };

    let document = Html::parse_document(&body);
    let selector = Selector::parse("a").unwrap();

    let mut new_found_urls = vec![];

    for link in document.select(&selector) {
        let attr_option = link.value().attr("href");
        let href = match attr_option {
            Some(attr) => attr,
            None => continue,
        };
        if href.starts_with("/") {
            new_found_urls.push((root.clone() + href).to_string());
        } else if href.starts_with("mailto:")
            || href.starts_with("javascript")
            || href.starts_with("tel")
            || href.starts_with("#")
        {
            continue;
        }
        if is_same_host(&root, href.to_string()) {
            new_found_urls.push(href.to_string());
        }
    }
    Ok((new_found_urls, body.len()))
}
