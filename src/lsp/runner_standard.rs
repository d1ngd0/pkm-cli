use std::{
    ffi::{OsStr, OsString},
    path::Path,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU32, Ordering},
};

use super::{RequestID, Result, Sender};

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
        Ok(StandardRunner {
            request: AtomicU32::new(0),
            child,
        })
    }
}

// StandardRunner is used to create a running lsp. This lsp communicates over Stdio and will
// run as a subprocess of the application
pub struct StandardRunner {
    request: AtomicU32,
    child: Child,
}

impl StandardRunner {
    fn next_request_id(&mut self) -> RequestID {
        self.request.fetch_add(1, Ordering::SeqCst)
    }
}

impl Sender for StandardRunner {
    fn send<R: Into<super::Request>>(&mut self, msg: R) -> Result<RequestID> {
        let mut msg = msg.into();
        let id = self.next_request_id();
        msg.id = id;
        Ok(id)
    }
}
