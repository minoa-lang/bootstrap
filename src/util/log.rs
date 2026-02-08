use std::{
    fmt::Arguments,
    fs,
    io::{self, BufWriter, Stderr, Stdout, Write},
    path::Path, time::SystemTime
};

use parking_lot::RwLock;

use crate::util::get_local_datetime;


#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, PartialOrd, Eq)]
pub enum Level {
    Severe,
    Error,
    Warning,
    Info,
    Verbose,
    Debug,
}

impl Level {
    fn as_str(self) -> &'static str {
        match self {
            Level::Severe  => "Severe",
            Level::Error   => "Error",
            Level::Warning => "Warning",
            Level::Info    => "Info",
            Level::Verbose => "Verbose",
            Level::Debug   => "Debug",
        }
    }

    fn as_upper_str(self) -> &'static str {
        match self {
            Level::Severe  => "SEVERE",
            Level::Error   => "ERROR",
            Level::Warning => "WARNING",
            Level::Info    => "INFO",
            Level::Verbose => "VERBOSE",
            Level::Debug   => "DEBUG",
        }
    }

    fn to_ansi_color(self) -> &'static str {
        match self {
            Level::Severe  => "\x1B[97;41m",
            Level::Error   => "\x1B[31;49m",
            Level::Warning => "\x1B[33;49m",
            Level::Info    => "\x1B[37;49m",
            Level::Verbose => "\x1B[90;49m",
            Level::Debug   => "\x1B[34;49m",
        }
    }

    fn should_write_to_stderr(self) -> bool {
        matches!(self, Level::Severe | Level::Error)
    }
}


pub struct Logger {
    stdout:          Option<Stdout>,
    stderr:          Stderr,
    writer:          Option<RwLock<BufWriter<fs::File>>>,
    level:           Level,
    use_ansi_colors: bool,
}

#[allow(unused)]
impl Logger {
    pub fn new<P: ?Sized + AsRef<Path>>(to_stdout: bool, log_file: Option<&P>, level: Level, use_ansi_colors: bool) -> io::Result<Self> {
        Self::_new(to_stdout, log_file.map(|path| path.as_ref()), level, use_ansi_colors)
    }

    pub fn _new(to_stdout: bool, log_file: Option<&Path>, level: Level, use_ansi_colors: bool) -> io::Result<Self> {
        let stdout = if to_stdout {
            Some(io::stdout())
        } else {
            None
        };

        let writer = match log_file {
            Some(log_file) => {
                if let Some(dir) = log_file.parent() {
                    fs::create_dir_all(dir)?;
                }

                let file = fs::File::create(log_file)?;

                let absolute_path = std::path::absolute(log_file).unwrap();
                println!("Created logfile at: {}", absolute_path.as_path().to_str().unwrap());

                let buf_writer = BufWriter::new(file);
                let rwlock = RwLock::new(buf_writer);
                Some(rwlock)
            },
            None => None,
        };

        let level = if level == Level::Severe {
            Level::Error
        } else {
            level
        };

        let stderr = io::stderr();

        Ok(Self {
            stdout,
            stderr, 
            writer,
            level,
            use_ansi_colors,
        })
    }

    pub fn max_level(&self) -> Level {
        self.level
    }

    /// All messages are logged with newlines
    pub fn log(&self, level: Level, text: &str) -> io::Result<()> {
        let to_stderr = level.should_write_to_stderr();
        if !to_stderr && level > self.level {
            return Ok(());
        }

        if self.use_ansi_colors {
            self._log(to_stderr, level.to_ansi_color())?;
        }

        let timestamp = get_local_datetime();
        self._log_fmt(to_stderr, format_args!("[{}]", timestamp));

        self._log_fmt(to_stderr, format_args!("[{:7}] ", level.as_upper_str()))?;
        
        // reset ANSI color
        if self.use_ansi_colors {
            self._log(to_stderr, text)?;
            self._logln(to_stderr, "\x1B[39;49m")
        } else {
            self._logln(to_stderr, text)
        }
    }



    pub fn logln_undecorated(&mut self, text: &str) -> io::Result<()> {
        self._logln(false, text)
    }

    pub fn log_undecorated(&mut self, text: &str) -> io::Result<()> {
        self._log(false, text)
    }


    fn _logln(&self, to_stderr: bool, text: &str) -> io::Result<()> {
        if let Some(stdout) = &self.stdout {
            let mut stdout = stdout.lock();
            stdout.write(text.as_bytes())?;
            stdout.write(&[b'\n'])?;
        } else if to_stderr {
            let mut stderr = self.stderr.lock();
            stderr.write(text.as_bytes())?;
            stderr.write(&[b'\n'])?;
        }
        if let Some(writer) = &self.writer {
            let mut writer = writer.write();
            writer.write(text.as_bytes())?;
            writer.write(&[b'\n'])?;
        }
        Ok(())
    }

    fn _log(&self, to_stderr: bool, text: &str) -> io::Result<()> {
        if let Some(stdout) = &self.stdout {
            let mut stdout = stdout.lock();
            stdout.write(text.as_bytes())?;
        } else if to_stderr {
            let mut stderr = self.stderr.lock();
            stderr.write(text.as_bytes())?;
        }
        if let Some(writer) = &self.writer {
            let mut writer = writer.write();
            writer.write(text.as_bytes())?;
        }
        Ok(())
    }

    fn _log_fmt(&self, to_stderr: bool, args: Arguments) -> io::Result<()> {
        if let Some(stdout) = &self.stdout {
            let mut stdout = stdout.lock();
            stdout.write_fmt(args)?;
        } else if to_stderr {
            let mut stderr = self.stderr.lock();
            stderr.write_fmt(args)?;
        }
        if let Some(writer) = &self.writer {
            let mut writer = writer.write();
            writer.write_fmt(args)?;
        }
        Ok(())
    }
}