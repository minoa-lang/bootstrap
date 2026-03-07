use std::{fmt, iter::Skip};

use bootstrap_macros::enum_utils;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[enum_utils(as_str)]
enum TreeIndentKind {
    #[string("    ")]
    Empty,
    #[string("│   ")]
    Skip,
    #[string("├── ")]
    Intermediate,
    #[string("└── ")]
    Final,
    #[string("┬── ")]
    NoName,
    #[string("├───")]
    IntermediateFullLen,
    #[string("└───")]
    FinalFullLen,
}

pub struct TreeIndentFormatter {
    indents: Vec<TreeIndentKind>,
}

impl TreeIndentFormatter {
    pub fn new() -> Self {
        Self {
            indents: vec![TreeIndentKind::Intermediate],   
        }
    }

    pub fn depth(&self) -> usize {
        self.indents.len() - 1
    }

    pub fn push(&mut self) {
        self.indents.push(TreeIndentKind::Intermediate);
    }

    pub fn push_no_name(&mut self) {
        self.indents.push(TreeIndentKind::NoName);
    }

    pub fn pop(&mut self) {
        self.indents.pop();
    }

    pub fn set_final_indent(&mut self) {
        if let Some(last) = self.indents.last_mut() {
            *last = TreeIndentKind::Final;
        }
    }

    pub fn set_final_indent_if(&mut self, cond: bool) {
        if cond {
            self.set_final_indent();
        }
    }

    pub fn signal_no_name_formatted(&mut self) {
        let Some(last) = self.indents.last_mut() else { return; };
        if *last == TreeIndentKind::NoName {
            *last = TreeIndentKind::Intermediate;
        }
    }

    fn get_ident_to_draw(&self, idx: usize) -> TreeIndentKind {
        if self.indents.is_empty() {
            return TreeIndentKind::Empty;
        }

        let last_idx = self.indents.len() - 1;
        let last_indent = self.indents[last_idx];

        if idx == last_idx {
            return last_indent;
        }

        if idx + 1 == last_idx && last_indent == TreeIndentKind::NoName {
            match self.indents[idx] {
                TreeIndentKind::Empty        => TreeIndentKind::Empty,
                TreeIndentKind::Skip         => TreeIndentKind::Skip,
                TreeIndentKind::Intermediate => TreeIndentKind::IntermediateFullLen,
                TreeIndentKind::Final        => TreeIndentKind::FinalFullLen,
                TreeIndentKind::NoName       => TreeIndentKind::Skip,
                _ => unreachable!(),
            }
        } else {
            match self.indents[idx] {
                TreeIndentKind::Empty        => TreeIndentKind::Empty,
                TreeIndentKind::Skip         => TreeIndentKind::Skip,
                TreeIndentKind::Intermediate => TreeIndentKind::Skip,
                TreeIndentKind::Final        => TreeIndentKind::Empty,
                TreeIndentKind::NoName       => TreeIndentKind::Skip,
                _ => unreachable!(),
            }
        }
    }
}

impl fmt::Display for TreeIndentFormatter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for idx in 0..self.indents.len() {
            f.write_str(self.get_ident_to_draw(idx).as_str())?;
        }
        Ok(())
    }
}