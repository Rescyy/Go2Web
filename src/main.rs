use std::{env, process::exit};
use futures::executor::block_on;
use linked_hash_set::LinkedHashSet;

async fn request_get(url: &String) {
    let body = reqwest::get(url)
    .await.expect("Request timed out")
    .text()
    .await.expect("Failed to parse the response");

    // println!("{}", body);
    let html = scraper::Html::parse_document(&body);
    let selector = scraper::Selector::parse("*").unwrap();
    let mut html_text_vec: LinkedHashSet<String> = LinkedHashSet::new();
    for element in html.select(&selector) {
        
        // let vector = element.text().collect::<Vec<_>>();
        // for text in vector.iter() {
        //     let text = text.trim();
        //     if !html_text_vec.contains(text) {
        //         html_text_vec.insert(text.to_string());
        //     }
        // }
    }
    let mut html_text: String = String::new();
    for text in html_text_vec.into_iter() {
        html_text.push_str(text.as_str());
        html_text.push_str("\n");
    }

    println!("{}", html_text);
}

const HELP_MESSAGE: &'static str = 
"go2web -u <URL>         # make an HTTP request to the specified URL and print the response
go2web -s <search-term> # make an HTTP request to search the term using your favorite search engine and print top 10 results
go2web -h               # show this help";

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() <= 1 {
        println!("Please input an argument, consider typing \"go2web -h\"");
        return;
    }

    match args.get(1).unwrap().as_str() {
        "-h" => {
            println!("{}", HELP_MESSAGE);
        },
        "-u" => {
            let url = args.get(2).expect("URL expected after -u");
            let future = request_get(url); // Nothing is printed
            block_on(future); // `future` is run and "hello, world!" is printed
        }
        invalid_input => {
            println!("Invalid input \"{}\", consider typing \"go2web -h\"", invalid_input);
            return;
        }
    };

}