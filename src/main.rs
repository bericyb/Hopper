use hopper::{JobStatus, Message, Worker, WorkerStatus};
use std::{
    collections::{HashMap, HashSet},
    io::Write,
    sync::mpsc,
    time::Instant,
};

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    url: String,

    #[arg(short, long)]
    only_text: bool,

    #[arg(short, long)]
    num_workers: u8,
}

fn main() {
    let args = Args::parse();

    let start = Instant::now();

    let mut finished_pages: HashSet<String> = HashSet::new();
    let mut url_queue: HashMap<String, JobStatus> = HashMap::new();
    let mut total_bytes = 0;

    url_queue.insert(args.url.clone(), JobStatus::QUEUED);

    let (worker_transmitter, worker_messages) = mpsc::channel::<Message>();

    let mut workers = HashMap::new();

    for i in 0..args.num_workers {
        let (master_transmitter, worker_receiver) = mpsc::channel::<Message>();

        workers.insert(
            i,
            Worker::new(
                i,
                master_transmitter,
                worker_transmitter.clone(),
                worker_receiver,
            ),
        );
    }

    println!("Created {} workers", args.num_workers);

    let mut worker_id = args.num_workers + 1;

    // Check messages from workers and set worker status
    // Distribute messages to workers
    loop {
        loop {
            let possible_message = worker_messages.try_recv();

            let message_content = match possible_message {
                Ok(message) => message,
                Err(_) => break,
            };

            match message_content {
                Message::DONE(id, url, bytes) => {
                    finished_pages.insert(url.clone());
                    total_bytes = total_bytes + bytes;
                    if let Some(worker) = workers.get_mut(&id) {
                        worker.status = WorkerStatus::WAITING;
                        for job in url_queue.iter_mut() {
                            if *job.1 == JobStatus::QUEUED {
                                worker.status = WorkerStatus::BUSY(job.0.to_string());
                                *job.1 = JobStatus::INPROGRESS(worker.id);
                                worker
                                    .channel
                                    .send(Message::CONTENT(
                                        worker.id,
                                        args.url.clone(),
                                        job.0.to_string(),
                                    ))
                                    .unwrap();
                                break;
                            }
                        }
                    }
                }
                Message::WAITING(id) => {
                    if let Some(worker) = workers.get_mut(&id) {
                        for job in url_queue.iter_mut() {
                            if *job.1 == JobStatus::QUEUED {
                                worker.status = WorkerStatus::BUSY(job.0.to_string());
                                *job.1 = JobStatus::INPROGRESS(worker_id);
                                worker
                                    .channel
                                    .send(Message::CONTENT(
                                        worker.id,
                                        args.url.clone(),
                                        job.0.to_string(),
                                    ))
                                    .unwrap();
                                break;
                            }
                        }
                    }
                }
                Message::BUSY(..) => {}
                Message::ERROR(id, error) => {
                    workers.remove(&id);
                    println!("Experienced error from worker:{id} : {error}");

                    println!("Creating new worker...");

                    let (master_transmitter, worker_receiver) = mpsc::channel::<Message>();

                    workers.insert(
                        worker_id,
                        Worker::new(
                            worker_id,
                            master_transmitter,
                            worker_transmitter.clone(),
                            worker_receiver,
                        ),
                    );

                    worker_id += 1;
                }
                Message::CONTENT(_worker_id, _job_url, found_url) => {
                    if !finished_pages.contains(&found_url.clone())
                        && !url_queue.contains_key(&found_url)
                    {
                        url_queue.insert(found_url, JobStatus::QUEUED);
                    }
                }
            }
        }

        let mut num_waiting_workers = 0;

        let mut num_in_progress_jobs = 0;
        let mut num_queued_jobs = 0;
        for job in url_queue.iter() {
            if *job.1 == JobStatus::QUEUED {
                num_queued_jobs = num_queued_jobs + 1;
            } else {
                num_in_progress_jobs = num_in_progress_jobs + 1;
            }
        }

        for worker in workers.iter_mut() {
            // Assign job to waiting workers
            if worker.1.status == WorkerStatus::WAITING {
                for job in url_queue.iter_mut() {
                    if *job.1 == JobStatus::QUEUED {
                        worker
                            .1
                            .channel
                            .send(Message::CONTENT(
                                worker.0.clone(),
                                args.url.clone(),
                                job.0.clone(),
                            ))
                            .unwrap();

                        // println!("Sent job to worker: {}", worker.0);
                        *job.1 = JobStatus::INPROGRESS(worker.0.clone());
                        worker.1.status = WorkerStatus::BUSY(job.0.clone());
                        break;
                    }
                }

                if worker.1.status == WorkerStatus::WAITING {
                    num_waiting_workers = num_waiting_workers + 1;
                }
            }
        }

        print!(
            "{} urls scraped | Job Queue: {}    \r",
            finished_pages.len(),
            url_queue.len()
        );

        if num_waiting_workers == workers.len() {
            println!("All workers are waiting for work...\nEnding hopper");
            break;
        }
    }

    let duration = start.elapsed();
    println!("Time elapsed is: {:?}", duration);

    println!("Bytes downloaded: {}", total_bytes);

    for worker in workers {
        let _ = worker
            .1
            .channel
            .send(Message::DONE(worker.1.id, "Done".to_string(), 0));
        let _ = worker.1.handle.join();
    }

    let mut file = std::fs::File::create("scraped_pages.txt").unwrap();

    for entry in &finished_pages {
        writeln!(file, "{}", entry).unwrap();
    }
}
