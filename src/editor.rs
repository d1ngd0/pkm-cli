use crate::Result;
use std::{
    env,
    path::Path,
    process::{Command, ExitStatus, Stdio},
};

pub struct Editor {
    command: Command,
}

impl Editor {
    pub fn new<P>(editor: &str, root: P) -> Self
    where
        P: AsRef<Path>,
    {
        let mut command = Command::new(editor);

        command
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .current_dir(root);

        Editor { command }
    }

    pub fn new_from_env<P>(env: &str, root: P) -> Self
    where
        P: AsRef<Path>,
    {
        let editor = env::var(env).unwrap_or_else(|_| "vim".to_string());
        Self::new(&editor, root)
    }

    pub fn file<P>(mut self, arg: P) -> Self
    where
        P: AsRef<Path>,
    {
        self.command.arg(arg.as_ref().as_os_str());
        self
    }

    pub fn exec(mut self) -> Result<ExitStatus> {
        let status = self.command.status()?;
        Ok(status)
    }
}
