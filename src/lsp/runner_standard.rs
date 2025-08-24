use std::{
    collections::HashMap,
    ffi::OsStr,
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
};

use serde::Serialize;

use super::{Error, Request, RequestID, Response, Result, Runner, Sender};

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
    reader: BufReader<ChildStdout>,
    request: Arc<AtomicU32>,
    items_read: u32,
    writer: Arc<Mutex<ChildStdin>>,
}

impl StandardRunner {
    fn new(mut child: Child) -> Self {
        let reader = BufReader::new(child.stdout.take().expect("stdout will be there"));
        let writer = Arc::new(Mutex::new(child.stdin.take().expect("stdin will be there")));

        StandardRunner {
            responses: HashMap::new(),
            request: Arc::new(AtomicU32::new(0)),
            items_read: 0,
            reader,
            writer,
            _child: child,
        }
    }

    fn stash(&mut self) -> Result<()> {
        // get the number of requests we are expecting to read
        // TODO: fire and forget requests, like didOpen, break this
        let delta = self.request.load(Ordering::SeqCst) - self.items_read;
        for _ in 0..delta {
            let resp = self.read_response()?;
            self.responses.insert(resp.id, resp);
            self.items_read += 1;
        }

        Ok(())
    }

    fn read_response(&mut self) -> Result<Response> {
        let mut buf = String::new();
        let mut headers = HashMap::new();
        self.reader.read_line(&mut buf)?;

        while !buf.trim_end().is_empty() {
            let (key, value) = buf
                .split_once(":")
                .ok_or_else(|| Error::LSPError(format!("Invalid header \"{}\"", &buf)))?;

            headers.insert(key.trim().to_lowercase(), value.trim().to_string());

            buf.clear();
            self.reader.read_line(&mut buf)?;
        }

        let length: usize = headers
            .get("content-length")
            .ok_or_else(|| Error::LSPError(format!("missing required header Content-Length")))?
            .parse()?;

        let mut body = vec![0; length];
        self.reader.read_exact(&mut body)?;

        Ok(Response::new(headers, &body)?)
    }
}

impl Runner for StandardRunner {
    type Sender = StandardRunnerWriter;
    fn try_response(&mut self, r: RequestID) -> Result<Response> {
        self.stash()?;
        self.responses.remove(&r.into()).ok_or(Error::NotReady)
    }

    fn sender(&mut self) -> Result<StandardRunnerWriter> {
        Ok(StandardRunnerWriter {
            request: self.request.clone(),
            writer: self.writer.clone(),
        })
    }
}

pub struct StandardRunnerReader {}

pub struct StandardRunnerWriter {
    request: Arc<AtomicU32>,
    writer: Arc<Mutex<ChildStdin>>,
}

impl StandardRunnerWriter {
    fn next_request_id(&mut self) -> RequestID {
        self.request.fetch_add(1, Ordering::SeqCst)
    }
}

impl Sender for StandardRunnerWriter {
    fn send<S, R>(&mut self, msg: R) -> Result<RequestID>
    where
        S: Serialize,
        R: Into<Request<S>>,
    {
        let mut msg = msg.into();
        let id = self.next_request_id();
        msg.id = id;

        let req_b = serde_json::to_string(&msg)?;

        write!(
            self.writer.lock().unwrap(),
            "Content-Length:{}\r\n\r\n{}",
            req_b.len(),
            &req_b
        )?;

        Ok(id)
    }
}
