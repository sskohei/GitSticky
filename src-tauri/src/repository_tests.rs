//! A local HTTP fake exercises the production Git API transaction code without
//! contacting GitHub or reading the user's credentials, keys or database.
use super::*;
use crate::{
    auth::{AppConfig, TokenSet},
    secure_store::SecretStore,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

#[derive(Default)]
struct Store {
    value: Mutex<Option<Vec<u8>>>,
    fail: AtomicBool,
}
impl SecretStore for Store {
    fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>> {
        Ok(self.value.lock().unwrap().clone().map(Zeroizing::new))
    }
    fn save(&self, value: &[u8]) -> Result<()> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(crate::secure_store::storage_error());
        }
        *self.value.lock().unwrap() = Some(value.to_vec());
        Ok(())
    }
    fn clear(&self) -> Result<()> {
        *self.value.lock().unwrap() = None;
        Ok(())
    }
}
struct Git {
    counter: u64,
    head: String,
    commits: BTreeMap<String, (String, String)>,
    trees: BTreeMap<String, BTreeMap<String, String>>,
    blobs: BTreeMap<String, String>,
    fail_ref: bool,
    lose_ref_reply: bool,
    concurrent_ref: bool,
    mutations: usize,
}
impl Git {
    fn new() -> Self {
        let tree = format!("{:040x}", 1);
        let head = format!("{:040x}", 2);
        Self {
            counter: 2,
            head: head.clone(),
            commits: BTreeMap::from([(head, (String::new(), tree.clone()))]),
            trees: BTreeMap::from([(tree, BTreeMap::new())]),
            blobs: BTreeMap::new(),
            fail_ref: false,
            lose_ref_reply: false,
            concurrent_ref: false,
            mutations: 0,
        }
    }
    fn next(&mut self) -> String {
        self.counter += 1;
        format!("{:040x}", self.counter)
    }
    fn files(&self) -> BTreeMap<String, String> {
        self.trees[&self.commits[&self.head].1].clone()
    }
    fn tamper(&mut self, path: &str, content: Option<String>) {
        let mut files = self.files();
        if let Some(content) = content {
            let sha = self.next();
            self.blobs.insert(sha.clone(), content);
            files.insert(path.into(), sha);
        } else {
            files.remove(path);
        }
        let tree = self.next();
        self.trees.insert(tree.clone(), files);
        let head = self.next();
        self.commits.insert(head.clone(), (self.head.clone(), tree));
        self.head = head;
    }
    fn handle(&mut self, method: &str, path: &str, body: Value) -> (u16, Value) {
        let path = path
            .split('?')
            .next()
            .unwrap()
            .strip_prefix("/repos/owner/notes")
            .unwrap();
        if method != "GET" {
            self.mutations += 1;
        }
        match (method, path) {
            ("GET", "") => (200, json!({"private":true})),
            ("GET", "/branches/main") => (
                200,
                json!({"commit":{"sha":self.head,"commit":{"tree":{"sha":self.commits[&self.head].1}}}}),
            ),
            ("GET", p) if p.starts_with("/git/trees/") => {
                let tree = &self.trees[p.strip_prefix("/git/trees/").unwrap()];
                (
                    200,
                    json!({"truncated":false,"tree":tree.iter().map(|(path,sha)| json!({"path":path,"sha":sha,"mode":"100644","type":"blob"})).collect::<Vec<_>>()}),
                )
            }
            ("GET", p) if p.starts_with("/git/blobs/") => {
                let content = &self.blobs[p.strip_prefix("/git/blobs/").unwrap()];
                (
                    200,
                    json!({"encoding":"base64","size":content.len(),"content":STANDARD.encode(content)}),
                )
            }
            ("POST", "/git/blobs") => {
                let sha = self.next();
                self.blobs
                    .insert(sha.clone(), body["content"].as_str().unwrap().into());
                (201, json!({"sha":sha}))
            }
            ("POST", "/git/trees") => {
                let mut files = self.trees[body["base_tree"].as_str().unwrap()].clone();
                for change in body["tree"].as_array().unwrap() {
                    let path = change["path"].as_str().unwrap();
                    match change["sha"].as_str() {
                        Some(sha) => {
                            files.insert(path.into(), sha.into());
                        }
                        None => {
                            files.remove(path);
                        }
                    }
                }
                let sha = self.next();
                self.trees.insert(sha.clone(), files);
                (201, json!({"sha":sha}))
            }
            ("POST", "/git/commits") => {
                let sha = self.next();
                self.commits.insert(
                    sha.clone(),
                    (
                        body["parents"][0].as_str().unwrap().into(),
                        body["tree"].as_str().unwrap().into(),
                    ),
                );
                (201, json!({"sha":sha}))
            }
            ("PATCH", "/git/refs/heads/main") => {
                assert_eq!(body["force"], false);
                if self.concurrent_ref {
                    self.concurrent_ref = false;
                    self.tamper("unmanaged.txt", Some("concurrent".into()));
                }
                let next = body["sha"].as_str().unwrap();
                if self.fail_ref || self.commits[next].0 != self.head {
                    return (422, json!({"message":"not fast forward"}));
                }
                self.head = next.into();
                if self.lose_ref_reply {
                    self.lose_ref_reply = false;
                    return (500, json!({"message":"lost reply"}));
                }
                (200, json!({"object":{"sha":next}}))
            }
            _ => panic!("Unexpected API request: {method} {path}"),
        }
    }
}
struct Server {
    address: String,
    git: Arc<Mutex<Git>>,
    stopped: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Server {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let git = Arc::new(Mutex::new(Git::new()));
        let stopped = Arc::new(AtomicBool::new(false));
        let state = git.clone();
        let stop = stopped.clone();
        let worker = thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream.set_nonblocking(false).unwrap();
                        stream
                            .set_read_timeout(Some(Duration::from_secs(5)))
                            .unwrap();
                        let mut input = Vec::new();
                        let mut chunk = [0u8; 4096];
                        let header_end = loop {
                            let n = stream.read(&mut chunk).unwrap();
                            assert!(n > 0);
                            input.extend_from_slice(&chunk[..n]);
                            if let Some(i) = input.windows(4).position(|w| w == b"\r\n\r\n") {
                                break i + 4;
                            }
                        };
                        let headers = String::from_utf8(input[..header_end].to_vec()).unwrap();
                        assert!(headers
                            .to_lowercase()
                            .contains("authorization: bearer test-token"));
                        let size = headers
                            .lines()
                            .find_map(|line| {
                                line.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        while input.len() < header_end + size {
                            let n = stream.read(&mut chunk).unwrap();
                            assert!(n > 0);
                            input.extend_from_slice(&chunk[..n]);
                        }
                        let mut first = headers.lines().next().unwrap().split_whitespace();
                        let method = first.next().unwrap();
                        let path = first.next().unwrap();
                        let body = if size == 0 {
                            Value::Null
                        } else {
                            serde_json::from_slice(&input[header_end..header_end + size]).unwrap()
                        };
                        let (status, response) = state.lock().unwrap().handle(method, path, body);
                        let response = serde_json::to_vec(&response).unwrap();
                        write!(stream,"HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",response.len()).unwrap();
                        stream.write_all(&response).unwrap();
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1))
                    }
                    Err(e) => panic!("{e}"),
                }
            }
        });
        Self {
            address,
            git,
            stopped,
            worker: Some(worker),
        }
    }
    fn credentials(&self) -> Credentials {
        let store = Arc::new(Store::default());
        store
            .save(
                &serde_json::to_vec(&TokenSet {
                    access_token: "test-token".into(),
                    refresh_token: None,
                    expires_at: None,
                    refresh_expires_at: None,
                    client_id: String::new(),
                    mode: "legacy_pat".into(),
                    must_reconnect: false,
                })
                .unwrap(),
            )
            .unwrap();
        let mut credentials = Credentials::new(
            AppConfig {
                client_id: String::new(),
                slug: String::new(),
            },
            store,
            Arc::new(crate::oauth::GitHubOAuth::new().unwrap()),
        );
        credentials.api_base =
            Some(reqwest::Url::parse(&format!("http://{}/", self.address)).unwrap());
        credentials
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
        let joined = self.worker.take().unwrap().join();
        if !thread::panicking() {
            joined.unwrap();
        }
    }
}
fn settings() -> Settings {
    Settings {
        repository_owner: "owner".into(),
        repository_name: "notes".into(),
        ..Default::default()
    }
}
async fn setup(server: &Server) -> (Credentials, Arc<Store>, SyncKeys, NoteCipher) {
    let credentials = server.credentials();
    let store = Arc::new(Store::default());
    let keys = SyncKeys::new(store.clone());
    let key = NoteCipher::random();
    crate::crypto::register_sync_key(&settings(), key.encoded().to_string(), &keys, &credentials)
        .await
        .unwrap();
    list(&settings(), &credentials, &keys).await.unwrap();
    (credentials, store, keys, key)
}
#[tokio::test]
async fn rejects_unsigned_replacement_deletion_and_complete_snapshot_replay() {
    let server = Server::new();
    let (credentials, _, keys, _) = setup(&server).await;
    let id = uuid::Uuid::new_v4().to_string();
    let old = write(&settings(), &id, Some("old"), None, &credentials, &keys)
        .await
        .unwrap()
        .unwrap();
    let old_head = server.git.lock().unwrap().head.clone();
    let latest = write(
        &settings(),
        &id,
        Some("latest"),
        Some(&old),
        &credentials,
        &keys,
    )
    .await
    .unwrap()
    .unwrap();
    let latest_head = server.git.lock().unwrap().head.clone();
    let old_content = server.git.lock().unwrap().blobs[&old].clone();
    server
        .git
        .lock()
        .unwrap()
        .tamper(&format!("notes/{id}.md"), Some(old_content));
    assert!(list(&settings(), &credentials, &keys).await.is_err());
    server.git.lock().unwrap().head = latest_head.clone();
    server
        .git
        .lock()
        .unwrap()
        .tamper(&format!("notes/{id}.md"), None);
    assert!(list(&settings(), &credentials, &keys).await.is_err());
    server.git.lock().unwrap().head = old_head;
    assert!(list(&settings(), &credentials, &keys).await.is_err());
    server.git.lock().unwrap().head = latest_head;
    assert_eq!(
        read(&settings(), &id, &latest, &credentials, &keys)
            .await
            .unwrap()
            .content,
        "latest"
    );
    write(&settings(), &id, None, Some(&latest), &credentials, &keys)
        .await
        .unwrap();
    assert!(list(&settings(), &credentials, &keys)
        .await
        .unwrap()
        .is_empty());
}
#[tokio::test]
async fn reregistering_another_key_cannot_reopen_plaintext_or_remove_checkpoint() {
    let server = Server::new();
    let (credentials, store, keys, _) = setup(&server).await;
    let before = store.load().unwrap().unwrap().to_vec();
    let id = uuid::Uuid::new_v4().to_string();
    server
        .git
        .lock()
        .unwrap()
        .tamper(&format!("notes/{id}.md"), Some("---\nlegacy".into()));
    server.git.lock().unwrap().tamper(MANIFEST_PATH, None);
    assert!(crate::crypto::register_sync_key(
        &settings(),
        NoteCipher::random().encoded().to_string(),
        &keys,
        &credentials
    )
    .await
    .is_err());
    assert_eq!(&*store.load().unwrap().unwrap(), &before);
}
#[tokio::test]
async fn rotation_is_atomic_resumable_and_other_devices_must_register_new_key() {
    let server = Server::new();
    let (credentials, store, keys, old_key) = setup(&server).await;
    let id = uuid::Uuid::new_v4().to_string();
    let old_sha = write(&settings(), &id, Some("secret"), None, &credentials, &keys)
        .await
        .unwrap()
        .unwrap();
    let new_key = NoteCipher::random();
    let old_head = server.git.lock().unwrap().head.clone();
    server.git.lock().unwrap().fail_ref = true;
    assert!(rotate(
        &settings(),
        Some(new_key.encoded().to_string()),
        &keys,
        &credentials
    )
    .await
    .is_err());
    assert_eq!(server.git.lock().unwrap().head, old_head);
    assert!(keys.key(&settings()).await.is_err());
    let restarted = SyncKeys::new(store);
    server.git.lock().unwrap().fail_ref = false;
    server.git.lock().unwrap().lose_ref_reply = true;
    assert!(rotate(&settings(), None, &restarted, &credentials)
        .await
        .is_err());
    let switched_head = server.git.lock().unwrap().head.clone();
    assert_ne!(switched_head, old_head);
    rotate(&settings(), None, &restarted, &credentials)
        .await
        .unwrap();
    assert_eq!(server.git.lock().unwrap().head, switched_head);
    let files = list(&settings(), &credentials, &restarted).await.unwrap();
    assert_ne!(files[0].sha, old_sha);
    assert_eq!(
        read(&settings(), &id, &files[0].sha, &credentials, &restarted)
            .await
            .unwrap()
            .content,
        "secret"
    );
    let content = server.git.lock().unwrap().blobs[&files[0].sha].clone();
    assert!(decrypt_sync(&old_key, &settings(), &id, &content).is_err());
    let second = SyncKeys::new(Arc::new(Store::default()));
    assert!(crate::crypto::register_sync_key(
        &settings(),
        old_key.encoded().to_string(),
        &second,
        &credentials
    )
    .await
    .is_err());
    crate::crypto::register_sync_key(
        &settings(),
        new_key.encoded().to_string(),
        &second,
        &credentials,
    )
    .await
    .unwrap();
    assert_eq!(
        list(&settings(), &credentials, &second)
            .await
            .unwrap()
            .len(),
        1
    );
}
#[tokio::test]
async fn failed_secure_storage_and_concurrent_branch_changes_do_not_publish_rotation() {
    let server = Server::new();
    let (credentials, store, keys, _) = setup(&server).await;
    let key = NoteCipher::random();
    let head = server.git.lock().unwrap().head.clone();
    let mutations = server.git.lock().unwrap().mutations;
    store.fail.store(true, Ordering::SeqCst);
    assert!(rotate(
        &settings(),
        Some(key.encoded().to_string()),
        &keys,
        &credentials
    )
    .await
    .is_err());
    assert_eq!(server.git.lock().unwrap().mutations, mutations);
    assert_eq!(server.git.lock().unwrap().head, head);
    store.fail.store(false, Ordering::SeqCst);
    server.git.lock().unwrap().concurrent_ref = true;
    assert!(rotate(
        &settings(),
        Some(key.encoded().to_string()),
        &keys,
        &credentials
    )
    .await
    .is_err());
    assert!(server
        .git
        .lock()
        .unwrap()
        .files()
        .contains_key("unmanaged.txt"));
    rotate(&settings(), None, &keys, &credentials)
        .await
        .unwrap();
    assert!(server
        .git
        .lock()
        .unwrap()
        .files()
        .contains_key("unmanaged.txt"));
}
