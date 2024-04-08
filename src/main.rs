use scraper::{ElementRef, Html};
use serde::{Deserialize, Serialize};
use std::{
    collections::{hash_map::DefaultHasher, HashSet},
    env,
    error::Error,
    fs::File,
    hash::Hasher,
    io::{Read, Write},
    net::TcpStream,
    path::Path,
};


// https://gist.github.com/strdr4605/b5c97f5268c56e01c1ee9ed9cba76abb

struct StringBuilder {
    literal: String,
    new_line_allowed: bool,
}

impl StringBuilder {
    fn new() -> Self {
        Self {
            literal: String::new(),
            new_line_allowed: true,
        }
    }

    fn append_line(&mut self, line: &str) {
        if !self.literal.contains(line) {
            if line.len() != 0 {
                self.literal.push_str(line);
                self.literal.push('\n');
            }
            if self.new_line_allowed {
                self.literal.push('\n');
                self.new_line_allowed = false;
            } else if line.len() != 0 {
                self.new_line_allowed = true;
            }
        }
    }

    fn get_string(self) -> String {
        self.literal
    }
}

fn query_html<'a>(html: &'a Html, tags: HashSet<&str>) -> Vec<ElementRef<'a>> {
    let mut vector = vec![];
    query_html_recursive(html.root_element(), &mut vector, &tags);
    vector
}

fn query_html_recursive<'a>(
    element: ElementRef<'a>,
    vector: &mut Vec<ElementRef<'a>>,
    tags: &HashSet<&str>,
) {
    for child_ref in element.child_elements() {
        let child = child_ref.value();
        if tags.contains(child.name()) {
            vector.push(child_ref);
        }
        query_html_recursive(child_ref, vector, tags);
    }
}

fn validate_link(url: &String, link: &str) -> String {
    if link.starts_with("//") {
        return format!("https:{link}");
    } else if !link.starts_with("http") {
        return format!("{url}{link}");
    }
    return String::from(link);
}

struct CacheDirectory {
    directory_path: String,
}

impl CacheDirectory {
    fn new(directory_path: String) -> Option<Self> {
        if Path::new(&directory_path).exists() {
            Some(CacheDirectory { directory_path })
        } else {
            match std::fs::create_dir(&directory_path) {
                Ok(_) => Some(CacheDirectory { directory_path }),
                Err(_) => None,
            }
        }
    }

    fn store_cache<T: ToString>(
        &mut self,
        url: &String,
        content: &T,
    ) -> Result<(), std::io::Error> {
        // println!("{url} {}", url.len());
        let mut hasher = DefaultHasher::new();
        hasher.write(url.as_bytes());
        let hash_result = hasher.finish();
        let file_path = format!("{}/{:x}.txt", self.directory_path, hash_result);
        println!("Created cache file {file_path}");
        let mut file = File::create(Path::new(&file_path))?;
        file.write(content.to_string().as_bytes())?;
        Ok(())
    }

    fn get_cache(&mut self, url: &String) -> Option<String> {
        // println!("{url} {}", url.len());
        let mut hasher = DefaultHasher::new();
        hasher.write(url.as_bytes());
        let hash_result = hasher.finish();
        let file_path = format!("{}/{:x}.txt", self.directory_path, hash_result);
        let mut file = match File::open(Path::new(&file_path)) {
            Ok(file) => file,
            _ => {
                // println!("Can't open cache file {file_path}");
                return None;
            }
        };
        let mut content = String::new();
        match file.read_to_string(&mut content) {
            Err(_) => {
                println!("Failed to read cache file");
                return None;
            }
            _ => return Some(content),
        }
    }
}

fn write_request(stream: &mut dyn Write, host: &str, path: &str) -> Result<(), Box<dyn Error>> {
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nAccept: text/html, application/json; charset=utf-8\r\nCache-Control: no-cache; max-age=0\r\n\r\n",
        path, host
    );
    stream.write_all(request.as_bytes())?;
    Ok(())
}

fn read_response(stream: &mut dyn Read) -> Result<String, Box<dyn Error>> {
    let mut response = String::new();
    let mut buf: [u8; 1] = [0];

    let result = stream.read_to_string(&mut response);
    return match result {
        Ok(_) => Ok(response),
        Err(err) => {
            println!("{err}");
            response.clear();
            println!("utf-8 problem");
            loop {
                match stream.read_exact(&mut buf) {
                    Err(_err) => {
                        break;
                    },
                    _ => (),
                }

                response.push(char::from_u32(buf[0] as u32).unwrap());
                // dbg!(&response);
            }
            Ok(response)
        },
    };
}

fn get_request(arg_url: &String) -> Option<(String, bool)> {
    const MAX_REDIRECTIONS: u8 = 10;

    let mut url = if !arg_url.starts_with("https") {
        if arg_url.starts_with("www") {
            "https://".to_owned() + arg_url
        } else {
            "https://www.".to_owned() + arg_url
        }
    } else {
        arg_url.to_owned()
    };
    let mut redirections = 0;

    let mut response_buffer;
    let (response_body, json_type) = 'redirect_loop: loop {
        let parsed_url = url::Url::parse(&url).expect("Wrong URL syntax");
        let host = parsed_url.host_str().ok_or("Missing host").unwrap();
        println!("{host}");
        let scheme = parsed_url.scheme();
        let port = parsed_url.port_or_known_default().unwrap();
        let addr = format!("{}:{}", host, port);
        let path = if parsed_url.path().is_empty() {
            "/"
        } else {
            parsed_url.path()
        };
        response_buffer = if parsed_url.scheme() == "https" {
            let mut builder = native_tls::TlsConnector::builder();
            builder.danger_accept_invalid_certs(true);
            let connector = builder.build().unwrap();
            let stream = TcpStream::connect(&addr).unwrap();
            let mut stream = connector.connect(host, stream).unwrap();
            println!("URL: {url}");

            write_request(&mut stream, host, path).unwrap();
            read_response(&mut stream).unwrap()
        } else {
            let mut stream = TcpStream::connect(&addr).unwrap();
            write_request(&mut stream, host, path).unwrap();
            read_response(&mut stream).unwrap()
        };
        // dbg!(&response_buffer);
        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut inter_response_info;
        inter_response_info = httparse::Response::new(&mut headers);
        inter_response_info
            .parse(response_buffer.as_bytes())
            .expect("Can't parse response");
        let reason = {
            match inter_response_info.reason {
                Some(reason) => reason,
                None => "None",
            }
        };
        if let Some(code) = inter_response_info.code {
            match code / 100 {
                1 => todo!("Not implemented for {code}"),
                2 => {
                    let mut json_type = false;
                    for i in 0..headers.len() {
                        if headers[i].name == "Content-Type" {
                            let mut value = headers[i].value;
                            let mut header_value_buf = String::with_capacity(value.len());
                            std::io::Read::read_to_string(&mut value, &mut header_value_buf)
                                .unwrap();
                            json_type = header_value_buf.contains("application/json");
                            break;
                        }
                    }
                    break 'redirect_loop (response_buffer, json_type);
                }
                3 => {
                    println!("Redirect: {code}");
                    let headers = &mut *inter_response_info.headers;
                    'find_location: for i in 0..headers.len() {
                        if headers[i].name == "Location" {
                            url.clear();
                            std::io::Read::read_to_string(&mut headers[i].value, &mut url).unwrap();
                            if url.starts_with("/") {
                                url = format!("{scheme}://{host}{url}");
                            }
                            break 'find_location;
                        }
                    }
                }
                4 => {
                    println!("Client side error: {code}\n Reason: {reason}");
                    return None;
                }
                5 => {
                    println!("Server side error: {code}\n Reason: {reason}");
                    return None;
                }
                _ => (),
            }
        } else {
            println!("Invalid response");
            return None;
        }
        redirections += 1;
        if redirections > MAX_REDIRECTIONS {
            return None;
        }
    };
    return Some((response_body, json_type));
}

fn display_url(url: &String, cache: &mut Option<CacheDirectory>) {
    let url = if !url.starts_with("https") {
        if url.starts_with("www") {
            "https://".to_owned() + url
        } else {
            "https://www.".to_owned() + url
        }
    } else {
        url.to_owned()
    };

    println!("Accessing {}", url);
    match cache {
        Some(unwrapped_cache) => {
            println!("Cache directory present");
            match unwrapped_cache.get_cache(&url) {
                Some(content) => {
                    println!("Retrieving cached data\n{}", content);
                    return;
                }
                None => (),
            }
        }
        None => (),
    }

    if let Some((response_body, json_type)) = get_request(&url) {
        if !json_type {
            let response_body_start =
            response_body.find("\r\n\r\n").expect("Invalid response");
            let response_body = response_body[response_body_start..]
            .to_owned()
            .trim()
            .to_string();

            let html = scraper::Html::parse_document(&response_body);
            let tags: HashSet<&str> = HashSet::from_iter(
                vec![
                    "h1", "h2", "h3", "h4", "h5", "h6", "span", "p", "img", "a", "button",
                ]
                .into_iter(),
            );

            let element_vector = query_html(&html, tags);
            let mut html_text_builder: StringBuilder = StringBuilder::new();
            for element_ref in element_vector.iter() {
                let element = element_ref.value();
                match element.name() {
                    "a" => match element.attr("href") {
                        Some(link) => {
                            let _name = {
                                let mut name: String = String::new();
                                for element_text in element_ref.text() {
                                    let element_text = element_text.trim();
                                    name.push_str(element_text);
                                }
                                if name.len() != 0 {
                                    name = "(".to_owned() + &name + ")";
                                }
                                name
                            };
                            html_text_builder.append_line(
                                format!("Link: {_name} \"{}\"", validate_link(&url, link)).as_str(),
                            );
                        }
                        None => (),
                    },
                    "img" => match element.attr("src") {
                        Some(link) => {
                            html_text_builder.append_line(
                                format!("Image: \"{}\"", validate_link(&url, link)).as_str(),
                            );
                        }
                        None => (),
                    },
                    _name => {
                        for element_text in element_ref.text() {
                            let element_text = element_text.trim();
                            html_text_builder.append_line(element_text);
                        }
                    }
                }
            }

            let html_text = html_text_builder.get_string();
            match cache {
                Some(cache) => {
                    cache
                        .store_cache(&url, &html_text)
                        .expect("Failed to store cache");
                }
                None => (),
            }
            println!("{}", html_text);
        } else {
            match cache {
                Some(cache) => {
                    cache
                        .store_cache(&url, &response_body)
                        .expect("Failed to store cache");
                }
                None => (),
            }
            println!("{}", response_body);
        }
    } else {
        println!("Error occured");
    }
}

#[derive(Serialize, Deserialize)]
struct LinkResults(Vec<String>);

const JSON_RESULTS_PATH: &'static str = "cache/search_results.json";

async fn search_url(search_text: &String) {
    let url = "https://google.com/search?q=".to_string() + search_text;
    let response = reqwest::get(url).await.expect("Request timed out");

    let body = response.text().await.expect("Failed to parse the response");
    // println!("{}", body);
    let html = scraper::Html::parse_document(&body);
    let tags: HashSet<&str> = HashSet::from_iter(vec!["a"].into_iter());
    let element_vector = query_html(&html, tags);
    let mut search_results: Vec<(String, String)> = Vec::new();
    for element_ref in element_vector.iter() {
        match element_ref.attr("href") {
            Some(link) => {
                if link.starts_with("/url?q=") && !link.contains("google.com") {
                    let mut text = String::new();
                    for element in element_ref.text() {
                        text.push_str(element);
                    }
                    search_results.push((link[7..].to_owned(), text));
                }
            }
            _ => (),
        }
        if search_results.len() == 10 {
            break;
        }
    }
    let mut link_results: LinkResults = LinkResults(vec![]);
    for i in 0..search_results.len() {
        let (link, description) = search_results.get(i).unwrap();
        link_results.0.push(link.clone());
        println!("{}. {}\nLink: {}\n", i + 1, description, link);
    }
    let mut json_file = File::create(Path::new(JSON_RESULTS_PATH)).unwrap();
    let json: String = serde_json::to_string(&link_results).unwrap();
    json_file
        .write(json.as_bytes())
        .expect("Cannot save results in json file.");
}

async fn get_previous(index: usize, cache: &mut Option<CacheDirectory>) {
    let mut json_file =
        File::open(Path::new(JSON_RESULTS_PATH)).expect("No previous search results found");
    let mut json: String = String::new();
    json_file
        .read_to_string(&mut json)
        .expect("Can't read from the search results file for some reason.");
    let link_results =
        serde_json::from_str::<LinkResults>(&json).expect("Search results json file corrupted");
    let link_results = link_results.0;
    let link = link_results
        .get(index)
        .expect("No link results with such index found");
    // dbg!(link);
    display_url(link, cache);
}

const HELP_MESSAGE: &'static str = 
"go2web -u <URL>                 # make an HTTP request to the specified URL and print the response
go2web -s <search-term>         # make an HTTP request to search the term using your favorite search engine and print top 10 results
go2web -p <search-result-index> # same as go2web -u <search-result-selected-link>
go2web -h                       # show this help";

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() <= 1 {
        println!("Please input an argument, consider typing \"go2web -h\"");
        return;
    }

    let mut cache = CacheDirectory::new(".\\cache".to_string());

    match args.get(1).unwrap().as_str() {
        "-h" => {
            println!("{}", HELP_MESSAGE);
        }
        "-u" => {
            let url = args.get(2).expect("URL expected after -u");
            display_url(url, &mut cache);
        }
        "-s" => {
            let search_text = args.get(2).expect("Search term expect after -s");
            search_url(search_text).await;
        }
        "-p" => {
            let search_text = args
                .get(2)
                .expect("Index of the search result expected after -p");
            get_previous(
                search_text
                    .trim()
                    .parse::<usize>()
                    .expect(format!("Number expected, found {}", search_text).as_str()),
                &mut cache,
            ).await;
        }
        invalid_input => {
            println!(
                "Invalid input \"{}\", consider typing \"go2web -h\"",
                invalid_input
            );
            return;
        }
    };
}
