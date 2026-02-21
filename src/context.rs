use parking_lot::Mutex;

use crate::{err_warn::ErrorWarnState, stats::{CompilerStatFlags, CompilerStats}};


pub struct Context {
    pub stats:    Mutex<CompilerStats>,
    pub err_warn: Mutex<ErrorWarnState>,
}

impl Context {
    pub fn new(stat_flags: CompilerStatFlags) -> Self {
        Self {
            stats: Mutex::new(CompilerStats::new(stat_flags)),
            err_warn: Mutex::new(ErrorWarnState::new()),
        }
    }
}