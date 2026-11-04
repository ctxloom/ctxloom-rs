//! A GitHub API on a local port, for tests: no network. It records every
//! request and answers by a function of it; [`Stub::github`] answers as a
//! small GitHub does (search, a repository, its default branch, the Git Data
//! API a pull request is made through, and the pulls themselves).
//!
//! Compiled for this crate's tests and, under `test-support`, for the
//! binary's acceptance tests. Std only.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::{json, Value};

/// One request the stub received.
#[derive(Clone, Debug)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub query: String,
    /// Header names lowercased.
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.as_str())
    }

    /// The body as json.
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or_else(|e| panic!("{e}: {:?}", self.body))
    }

    /// `METHOD /path`.
    pub fn line(&self) -> String {
        format!("{} {}", self.method, self.path)
    }

    /// The value of a query parameter, percent-decoded.
    pub fn param(&self, name: &str) -> Option<String> {
        self.query.split('&').find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            (key == name).then(|| decode(value))
        })
    }
}

/// An answer: a status and a json body.
#[derive(Clone, Debug)]
pub struct Reply {
    pub status: u16,
    pub body: String,
}

impl Reply {
    pub fn new(status: u16, body: &str) -> Reply {
        Reply {
            status,
            body: body.to_string(),
        }
    }

    pub fn json(status: u16, body: &Value) -> Reply {
        Reply::new(status, &body.to_string())
    }
}

type Responder = Box<dyn Fn(&Request) -> Reply + Send + Sync>;

/// A listening stub.
pub struct Stub {
    url: String,
    seen: Arc<Mutex<Vec<Request>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Stub {
    /// Answer every request with `responder`'s reply.
    pub fn start(responder: impl Fn(&Request) -> Reply + Send + Sync + 'static) -> Stub {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free local port");
        listener.set_nonblocking(true).expect("non-blocking");
        let url = format!("http://{}", listener.local_addr().expect("an address"));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let responder: Arc<Responder> = Arc::new(Box::new(responder));
        let thread = {
            let (seen, stop) = (seen.clone(), stop.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let (seen, responder) = (seen.clone(), responder.clone());
                            std::thread::spawn(move || serve(stream, &seen, &responder));
                        }
                        Err(_) => std::thread::sleep(Duration::from_millis(2)),
                    }
                }
            })
        };
        Stub {
            url,
            seen,
            stop,
            thread: Some(thread),
        }
    }

    /// Answer every request with one reply.
    pub fn replying(status: u16, body: &str) -> Stub {
        let reply = Reply::new(status, body);
        Stub::start(move |_| reply.clone())
    }

    /// A GitHub whose API is at `prefix` (empty for the root, `/api/v3` for an
    /// Enterprise's). It wants `token` on what writes, when given; a request
    /// that writes with none, or another, is a 401.
    pub fn github(token: Option<&str>, prefix: &str) -> Stub {
        Stub::github_failing(token, prefix, None)
    }

    /// The same, but a request whose `METHOD /path` ends with `at` is answered
    /// with `status` and the message, as GitHub answers an error.
    pub fn github_failing(
        token: Option<&str>,
        prefix: &str,
        failing: Option<(&str, u16, &str)>,
    ) -> Stub {
        let (token, prefix) = (token.map(str::to_string), prefix.to_string());
        let failing =
            failing.map(|(at, status, message)| (at.to_string(), status, message.to_string()));
        let blobs = AtomicUsize::new(0);
        Stub::start(move |request| {
            if let Some((at, status, message)) = &failing {
                if request.line().ends_with(at.as_str()) {
                    return Reply::json(*status, &json!({"message": message}));
                }
            }
            github(request, token.as_deref(), &prefix, &blobs)
        })
    }

    /// The stub's address, `http://127.0.0.1:<port>`.
    pub fn url(&self) -> String {
        self.url.clone()
    }

    /// Every request so far, in order.
    pub fn requests(&self) -> Vec<Request> {
        self.seen.lock().expect("the requests").clone()
    }

    /// `METHOD /path` of every request so far.
    pub fn lines(&self) -> Vec<String> {
        self.requests().iter().map(Request::line).collect()
    }
}

impl Drop for Stub {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve(stream: TcpStream, seen: &Mutex<Vec<Request>>, responder: &Responder) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut reader = BufReader::new(stream);
    let Some(request) = read(&mut reader) else {
        return;
    };
    let reply = responder(&request);
    seen.lock().expect("the requests").push(request);
    let text = reply.body;
    let head = format!(
        "HTTP/1.1 {} Stub\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        reply.status,
        text.len()
    );
    let mut stream = reader.into_inner();
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(text.as_bytes());
    let _ = stream.flush();
}

fn read(reader: &mut BufReader<TcpStream>) -> Option<Request> {
    let mut first = String::new();
    reader.read_line(&mut first).ok()?;
    let mut parts = first.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':')?;
        headers.push((name.to_ascii_lowercase(), value.trim().to_string()));
    }
    let length = headers
        .iter()
        .find(|(name, _)| name == "content-length")
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some(Request {
        method,
        path: path.to_string(),
        query: query.to_string(),
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

fn decode(text: &str) -> String {
    let bytes = text.replace('+', " ").into_bytes();
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let hex = bytes
            .get(at + 1..at + 3)
            .and_then(|pair| std::str::from_utf8(pair).ok())
            .and_then(|pair| u8::from_str_radix(pair, 16).ok());
        match (bytes[at], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                at += 3;
            }
            (byte, _) => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The sha the base branch is at, the tree under it, and what the pull
/// request's commit and tree are.
pub const BASE_SHA: &str = "b45e000000000000000000000000000000000000";
pub const BASE_TREE: &str = "7ee0000000000000000000000000000000000000";
pub const NEW_TREE: &str = "7ee1000000000000000000000000000000000001";
pub const NEW_COMMIT: &str = "c0de000000000000000000000000000000000001";

fn github(request: &Request, token: Option<&str>, prefix: &str, blobs: &AtomicUsize) -> Reply {
    let Some(path) = request.path.strip_prefix(prefix) else {
        return Reply::json(404, &json!({"message": "Not Found"}));
    };
    let writes = request.method == "POST";
    if writes {
        let sent = request.header("authorization");
        let wanted = token.map(|t| format!("Bearer {t}"));
        match (sent, wanted) {
            (None, _) => return Reply::json(401, &json!({"message": "Requires authentication"})),
            (Some(sent), Some(wanted)) if sent != wanted => {
                return Reply::json(401, &json!({"message": "Bad credentials"}))
            }
            _ => {}
        }
    }
    let segments: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    match (request.method.as_str(), segments.as_slice()) {
        ("GET", ["search", "repositories"]) => Reply::json(200, &search_results()),
        ("GET", ["repos", _, _]) => Reply::json(200, &json!({"default_branch": "main"})),
        ("GET", ["repos", _, _, "git", "ref", "heads", "main"]) => {
            Reply::json(200, &json!({"object": {"sha": BASE_SHA}}))
        }
        ("GET", ["repos", _, _, "git", "commits", BASE_SHA]) => {
            Reply::json(200, &json!({"tree": {"sha": BASE_TREE}}))
        }
        ("POST", ["repos", _, _, "git", "blobs"]) => {
            let n = blobs.fetch_add(1, Ordering::SeqCst) + 1;
            Reply::json(201, &json!({"sha": format!("b10b{n:036}")}))
        }
        ("POST", ["repos", _, _, "git", "trees"]) => Reply::json(201, &json!({"sha": NEW_TREE})),
        ("POST", ["repos", _, _, "git", "commits"]) => {
            Reply::json(201, &json!({"sha": NEW_COMMIT}))
        }
        ("POST", ["repos", _, _, "git", "refs"]) => {
            Reply::json(201, &json!({"object": {"sha": NEW_COMMIT}}))
        }
        ("POST", ["repos", owner, repo, "pulls"]) => Reply::json(
            201,
            &json!({"html_url": format!("https://github.com/{owner}/{repo}/pull/7")}),
        ),
        _ => Reply::json(404, &json!({"message": "Not Found"})),
    }
}

fn search_results() -> Value {
    json!({
        "total_count": 4,
        "items": [
            {
                "name": "ctxloom-golang",
                "owner": {"login": "bob"},
                "description": "bob's context",
                "stargazers_count": 10,
                "html_url": "https://github.com/bob/ctxloom-golang"
            },
            {
                "name": "dotfiles",
                "owner": {"login": "carol"},
                "description": "named nothing like it",
                "stargazers_count": 99,
                "html_url": "https://github.com/carol/dotfiles"
            },
            {
                "name": "ctxloom",
                "owner": {"login": "alice"},
                "description": null,
                "stargazers_count": 3,
                "html_url": "https://github.com/alice/ctxloom"
            },
            {
                "name": "ctxloomer",
                "owner": {"login": "dave"},
                "description": "only a prefix of the name",
                "stargazers_count": 1,
                "html_url": "https://github.com/dave/ctxloomer"
            }
        ]
    })
}
