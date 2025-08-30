use std::{
    collections::HashMap,
    ffi::OsStr,
    path::Path,
    process::Stdio,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
};

use serde::Serialize;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::mpsc::{Receiver, Sender, channel},
};

use super::{Error, Request, RequestID, Requester, Response, Result, Runner};

pub struct StandardRunnerBuilder {
    cmd: Command,
}

impl StandardRunnerBuilder {
    // new creates a new StandardRunnerBuilder.
    pub fn new<S: AsRef<OsStr>>(cmd: S) -> Self {
        let mut cmd = Command::new(cmd);
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped());
        Self { cmd }
    }

    // working dir set the working directory for the LSP
    pub fn working_dir<P: AsRef<Path>>(mut self, dir: P) -> Self {
        self.cmd.current_dir(dir);
        self
    }

    // arg sets a single argument to the lsp
    pub fn arg<S: AsRef<OsStr>>(mut self, arg: S) -> Self {
        self.cmd.arg(arg);
        self
    }

    // arg sets a single argument to the lsp
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.cmd.args(args);
        self
    }

    // spawn kicks off the lsp as a forked application and returns
    // a runner
    pub fn spawn(mut self) -> Result<StandardRunner> {
        let child = self.cmd.spawn()?;
        Ok(StandardRunner::new(child))
    }
}

// StandardRunner is used to create a running lsp. This lsp communicates over Stdio and will
// run as a subprocess of the application
pub struct StandardRunner {
    responses: HashMap<u32, Response>,
    _child: Child,
    recv: Receiver<Response>,
    request: Arc<AtomicU32>,
    writer: Arc<Mutex<ChildStdin>>,
}

impl StandardRunner {
    fn new(mut child: Child) -> Self {
        let (mut reader, recv) =
            StandardRunnerReader::new(child.stdout.take().expect("stdout will be there"));
        let writer = Arc::new(Mutex::new(child.stdin.take().expect("stdin will be there")));

        tokio::spawn(async move { reader.start().await });

        StandardRunner {
            responses: HashMap::new(),
            recv,
            request: Arc::new(AtomicU32::new(0)),
            writer,
            _child: child,
        }
    }
}

impl Runner for StandardRunner {
    type Sender = StandardRunnerWriter;
    async fn response(&mut self, r: RequestID) -> Result<Response> {
        loop {
            match self.recv.recv().await {
                Some(resp) => {
                    self.responses.insert(resp.id, resp);
                }
                None => return Err(Error::LSPError(String::from("reciever closed"))),
            }

            match self.responses.remove(&r) {
                Some(response) => return Ok(response),
                None => continue,
            }
        }
    }

    fn sender(&mut self) -> Result<StandardRunnerWriter> {
        Ok(StandardRunnerWriter {
            request: self.request.clone(),
            writer: self.writer.clone(),
        })
    }
}

struct StandardRunnerReader<R: AsyncRead + Unpin> {
    sync: Sender<Response>,
    reader: BufReader<R>,
}

impl<R: AsyncRead + Unpin> StandardRunnerReader<R> {
    fn new(reader: R) -> (Self, Receiver<Response>) {
        let (sync, rec) = channel(100);

        (
            StandardRunnerReader {
                sync,
                reader: BufReader::new(reader),
            },
            rec,
        )
    }

    async fn start(&mut self) -> Result<()> {
        loop {
            // if there is a read failure of some kind we return and close the routine
            let res = self.read_response().await?;
            // if their is no reciever because it was dropped we return and close the routine
            self.sync.send(res).await?;
        }
    }

    async fn read_response(&mut self) -> Result<Response> {
        let mut buf = String::new();
        let mut headers = HashMap::new();
        self.reader.read_line(&mut buf).await?;

        while !buf.trim_end().is_empty() {
            let (key, value) = buf
                .split_once(":")
                .ok_or_else(|| Error::LSPError(format!("Invalid header \"{}\"", &buf)))?;

            headers.insert(key.trim().to_lowercase(), value.trim().to_string());

            buf.clear();
            self.reader.read_line(&mut buf).await?;
        }

        let length: usize = headers
            .get("content-length")
            .ok_or_else(|| Error::LSPError(format!("missing required header Content-Length")))?
            .parse()?;

        let mut body = vec![0; length];
        self.reader.read_exact(&mut body).await?;

        Ok(Response::new(headers, &body)?)
    }
}

pub struct StandardRunnerWriter {
    request: Arc<AtomicU32>,
    writer: Arc<Mutex<ChildStdin>>,
}

impl StandardRunnerWriter {
    fn next_request_id(&mut self) -> RequestID {
        self.request.fetch_add(1, Ordering::SeqCst)
    }
}

impl Requester for StandardRunnerWriter {
    async fn send<S, R>(&mut self, msg: R) -> Result<RequestID>
    where
        S: Serialize,
        R: Into<Request<S>>,
    {
        let mut msg = msg.into();
        let id = self.next_request_id();
        msg.id = id;

        let req_b = serde_json::to_string(&msg)?;

        let headers = format!("Content-Length:{}\r\n\r\n{}", req_b.len(), &req_b);
        let mut writer = self.writer.lock().unwrap();
        writer.write_all(headers.as_bytes()).await?;
        writer.flush().await?;

        Ok(id)
    }
}
