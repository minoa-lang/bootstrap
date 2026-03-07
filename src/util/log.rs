use std::{
    cell::OnceCell, fmt::Arguments, fs, io::{self, BufWriter, Stderr, StderrLock, Stdout, StdoutLock, Write as _}, path::Path, sync::OnceLock, time::SystemTime
};

use parking_lot::{RwLock, RwLockWriteGuard};

use crate::util::{self, get_local_datetime};

#[macro_export]
macro_rules! log {
    ($level:ident, $msg:expr) => {
        $crate::util::log::get_logger().log_fmt($crate::util::log::Level::$level, format_args!($msg))
    };
    ($level:ident, $msg:expr $(, $val:expr)+) => {
        $crate::util::log::get_logger().log_fmt($crate::util::log::Level::$level, format_args!($msg $(, $val)+))
    };
}

#[macro_export]
macro_rules! log_str {
    ($level:ident, $msg:expr) => {
        $crate::util::log::get_logger().log($crate::util::log::Level::$level, $msg)
    };
}


pub fn init_global_logger<P: ?Sized + AsRef<Path>>(to_stdout: bool, log_file: Option<&P>, level: Level, use_ansi_colors: bool) -> io::Result<()> {
    let logger = Logger::new(to_stdout, log_file, level, use_ansi_colors)?;
    LOGGER.get_or_init(|| logger);
    Ok(())
}

pub fn get_logger() -> &'static Logger {
    LOGGER.get_or_init(|| panic!("LOGGER should have been initialized at this point"))
}

pub static LOGGER: OnceLock<Logger> = OnceLock::new();


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
                let file = util::create_file_and_dirs(log_file)?;

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

        let mut lock = self.lock(to_stderr);

        lock.log_header(level, self.use_ansi_colors)?;
        lock.log(text)?;
        lock.log(if self.use_ansi_colors { "\x1B[39;49m\n" } else { "\n" })
    }

    pub fn log_fmt(&self, level: Level, args: Arguments) -> io::Result<()> {
        let to_stderr = level.should_write_to_stderr();
        if !to_stderr && level > self.level {
            return Ok(());
        }

        let mut lock = self.lock(to_stderr);

        lock.log_header(level, self.use_ansi_colors)?;
        lock.log_fmt( args)?;
        // reset ANSI color if needed
        lock.log(if self.use_ansi_colors { "\x1B[39;49m\n" } else { "\n" })
    }

    pub fn logln_undecorated(&self, text: &str) -> io::Result<()> {
        let mut lock = self.lock(false);
        lock.log(text)?;
        lock.log("\n")
    }

    pub fn log_undecorated(&self, text: &str) -> io::Result<()> {
        self.lock(false).log(text)
    }

    fn lock<'a>(&'a self, to_stderr: bool) -> LockedLog<'a> {
        let should_use_stderr = to_stderr && self.stdout.is_none();
        LockedLog {
            stdout: self.stdout.as_ref().map(|stdout| stdout.lock()),
            stderr: should_use_stderr.then(|| self.stderr.lock()),
            file_io: self.writer.as_ref().map(|file_io| file_io.write()),
        }
    }

    pub fn flush(&self) -> io::Result<()> {
        if let Some(writer) = &self.writer {
            writer.write().flush()
        } else {
            Ok(())
        }
    }
}

// Used to avoid having to lock the outputs multiple times per line
struct LockedLog<'a> {
    stdout: Option<StdoutLock<'a>>,
    stderr: Option<StderrLock<'a>>,
    file_io: Option<RwLockWriteGuard<'a, BufWriter<fs::File>>>
}

impl LockedLog<'_> {
    fn log_fmt(&mut self, args: Arguments) -> io::Result<()> {
        if let Some(stdout) = &mut self.stdout {
            stdout.write_fmt(args)?;
        }
        if let Some(stderr) = &mut self.stderr {
            stderr.write_fmt(args)?;
        }
        if let Some(file_io) = &mut self.file_io {
            file_io.write_fmt(args)?;
        }
        Ok(())
    }

    fn log(&mut self, s: &str) -> io::Result<()> {
        if let Some(stdout) = &mut self.stdout {
            stdout.write(s.as_bytes())?;
        }
        if let Some(stderr) = &mut self.stderr {
            stderr.write(s.as_bytes())?;
        }
        if let Some(file_io) = &mut self.file_io {
            file_io.write(s.as_bytes())?;
        }
        Ok(())
    }

    fn log_header(&mut self, level: Level, use_ansi_colors: bool) -> io::Result<()> {
        if use_ansi_colors {
            self.log(level.to_ansi_color())?;
        }

        let timestamp = get_local_datetime();
        self.log_fmt(format_args!("[{}]", timestamp))?;
        self.log_fmt(format_args!("[{:7}] ", level.as_upper_str()))
    }
}