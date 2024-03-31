use std::{collections::HashSet, env, process::exit};
use futures::executor::block_on;
use linked_hash_set::LinkedHashSet;
use scraper::{node::Element, ElementRef, Html};

fn query_html<'a>(html: Html, tags: HashSet<&str>) -> Vec<Element> {
    let mut vector = vec![];
    query_html_recursive(html.root_element(), &mut vector, &tags);
    vector
}

fn query_html_recursive<'a>(element: ElementRef<'a>, vector: &mut Vec<Element>, tags: &HashSet<&str>) {
    for child_ref in element.child_elements() {
        let child = child_ref.value();
        if tags.contains(child.name()) {
            vector.push(child.clone());
        }
        query_html_recursive(child_ref, vector, tags);
    }
}

async fn request_get(url: &String) {
    let body = reqwest::get(url)
    .await.expect("Request timed out")
    .text()
    .await.expect("Failed to parse the response");

    println!("{}", body);
    let html = scraper::Html::parse_document(&body);
    let tags: HashSet<&str> = HashSet::from_iter(vec!["h1", "h2", "h3", "h4", "h5", "h6", "span", "p", "img", "a", "button"].into_iter());
    let element_vector = query_html(html, tags);
    for element in element_vector.iter() {
        match element.name() {
            _ => (),
        }
    }

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