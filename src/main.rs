use reqwest::blocking::get;
use scraper::{Html, Selector};
use std::collections::HashSet;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    url: String,

    #[arg(short, long)]
    only_text: bool,
}

fn main() {
    let args = Args::parse();

    let target = args.url;

    let mut seen_pages: HashSet<String> = HashSet::new();
    let mut url_queue: Vec<String> = vec![];

    let body = get(target).unwrap().text().unwrap();

    let document = Html::parse_document(&body);
    let selector = Selector::parse("a").unwrap();

    for link in document.select(&selector) {
        println!("{:?}", link.value());
    }
