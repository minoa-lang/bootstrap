#![allow(unused)]

use std::{collections::{HashMap, HashSet}, path::{Path, PathBuf}, time::*};

use bootstrap_macros::flags;

use crate::log;

pub struct FileLexStats {
    pub time:       Duration,
    pub num_tokens: usize,
}

impl FileLexStats {
    pub fn new() -> Self {
        Self { time: Duration::ZERO, num_tokens: 0 }
    }
}

pub struct LexStats {
    total:      Duration,
    num_tokens: usize,
    per_file:   HashMap<PathBuf, FileLexStats>,
}

impl LexStats {
    pub fn new() -> Self {
        Self {
            total: Duration::ZERO,
            num_tokens: 0,
            per_file: HashMap::new(),
        }
    }

    fn _add_file_stats(&mut self, path: &Path, dur: Duration, num_tokens: usize) {
        match self.per_file.get_mut(path) {
            Some(stats) => {
                stats.time += dur;
                stats.num_tokens += num_tokens;
            },
            None => {
                self.per_file.insert(path.to_path_buf(), FileLexStats {
                    time: dur,
                    num_tokens,
                });
            },
        }
    }

    fn _get_file_stat(&self, path: &Path) -> Option<&FileLexStats> {
        self.per_file.get(path)
    }
}


#[flags]
pub enum CompilerStatFlags {
    Minimal = 0,
    PerFileLex,

    PerFile = PerFileLex,
}

pub struct CompilerStats {
    flags: CompilerStatFlags,
    files: HashSet<PathBuf>,
    lex:   LexStats
}

impl CompilerStats {
    pub fn new(flags: CompilerStatFlags) -> Self {
        Self {
            flags: flags,
            files: HashSet::new(),
            lex:   LexStats::new(),
        }
    }

    pub fn log(&self, flags: CompilerStatFlags) {
        
        log!(Info, "Compiler Stats");
        log!(Info, "==============");

        log!(Info, "Summary:");
        log!(Info, "Lex: {}s, {} tokens (avg {} tokens/sec)", self.lex.total.as_secs_f32(), self.lex.num_tokens, ((self.lex.num_tokens as f32) / self.lex.total.as_secs_f32()).floor());



        log!(Verbose, "Per file:");

        if flags.intersects(CompilerStatFlags::PerFile) {
            for file in &self.files {
                log!(Verbose, " - Path: {}", file.to_str().unwrap());

                if flags.contains(CompilerStatFlags::PerFileLex) {
                    let info = self.get_file_lex_stats(file.as_path()).unwrap();
                    let dur = info.time.as_secs_f32();
                    let avg = ((info.num_tokens as f32) / dur).round();

                    log!(Verbose, "      - Lex: {dur}ms, {} tokens (avg {avg} tokens/sec)", info.num_tokens);
                }
            }
        }
    }

    pub fn log_current(&self) {
        self.log(self.flags);
    }

    pub fn add_file_lex_stats<P: ?Sized + AsRef<Path>>(&mut self, path: &P, dur: Duration, num_tokens: usize) {
        if self.flags.intersects(CompilerStatFlags::PerFile) {
            if !self.files.contains(path.as_ref()) {
                self.files.insert(path.as_ref().to_path_buf());
            }
        }

        if self.flags.contains(CompilerStatFlags::PerFileLex) {
            self.lex._add_file_stats(path.as_ref(), dur, num_tokens);
        }
        self.lex.total += dur;
        self.lex.num_tokens += num_tokens;
    }

    pub fn get_file_lex_stats<P: ?Sized + AsRef<Path>>(&self, path: &P) -> Option<&FileLexStats> {
        self.lex._get_file_stat(path.as_ref())
    }
}