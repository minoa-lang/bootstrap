use std::fmt;

use crate::{tokens::{Span, Token, TokenStream}, util::format::TreeIndentFormatter};


pub struct TokenTreeData {
    start:  u32,
    end:    u32,
    subtrees: Vec<TokenTreeData>
}

impl Default for TokenTreeData {
    fn default() -> Self {
        Self {
            start: 0,
            end: 0,
            subtrees: Vec::new()
        }
    }
}

pub enum TokenOrSubtree<'a> {
    Token(Token),
    Subtree(TokenTree<'a>)
}

#[derive(Clone, Copy)]
pub struct TokenTree<'a> {
    pub(super) stream:    &'a TokenStream,
    pub(super) tree_data: &'a TokenTreeData,
    pub(super) depth:     usize,
}

impl TokenTree<'_> {
    pub fn get_token_or_subtree<'a>(&'a self, idx: u32) -> Option<TokenOrSubtree<'a>> {
        self._get_token_or_subtree(idx, &self.tree_data)
    }
    fn _get_token_or_subtree<'a>(&'a self, idx: u32, tree: &'a TokenTreeData) -> Option<TokenOrSubtree<'a>> {
        if idx as usize >= self.stream.tokens.len() {
            return None;
        }

        for sub_tree in &tree.subtrees {
            if idx < sub_tree.start || idx > sub_tree.end {
                continue;
            }

            return Some(TokenOrSubtree::Subtree(TokenTree::<'a>{ stream: &self.stream, tree_data: sub_tree, depth: self.depth + 1 }));
        }

        return Some(TokenOrSubtree::Token(self.stream.tokens[idx as usize].clone()))
    }

    pub fn get_subtree_for<'a>(&'a self, idx: u32) -> TokenTree<'a> {
        self._get_subtree_for(idx, &self.stream.tree, 0)
    }
    fn _get_subtree_for(&self, idx: u32, tree_data: &TokenTreeData, depth: usize) -> Self {
        for sub_tree in &tree_data.subtrees {
            if idx < sub_tree.start || idx > sub_tree.end {
                continue;
            }

            return self._get_subtree_for(idx, &sub_tree, depth + 1);
        }
        *self
    }

    pub fn get_formatter<'a>(&'a self) -> TokenTreeFormatter<'a> {
        TokenTreeFormatter { tree: self }
    }

    fn get_max_indent(&self) -> usize {
        Self::_get_max_indent(&self.tree_data)
    }
    fn _get_max_indent(tree_data: &TokenTreeData) -> usize {
        let mut max_indent = 0;
        for sub_tree in &tree_data.subtrees {
            let indent = Self::_get_max_indent(sub_tree);
            max_indent = max_indent.max(indent);
        }
        max_indent + 1
    }
}

pub struct TokenTreeFormatter<'a> {
    tree:    &'a TokenTree<'a>,
}

impl TokenTreeFormatter<'_> {
    fn fmt_tree(&self, f: &mut fmt::Formatter<'_>, sub_tree: &TokenTree, idx: &mut usize, indents: &mut TreeIndentFormatter, max_ident: usize, first: bool) -> fmt::Result {
        if !first {
            indents.push_no_name();
        }

        let end = sub_tree.tree_data.end as u32;
        let indent_width = (max_ident - indents.depth() - 1) * 4 + 31;

        while *idx <= end as usize {
            indents.set_final_indent_if(*idx == end as usize);

            match sub_tree.get_token_or_subtree(*idx as u32).unwrap() {
                TokenOrSubtree::Token(token) => {
                    write!(f, "{indents}{:indent_width$}: {token}\n", token.get_kind_str())?;
                    *idx += 1;
                },
                TokenOrSubtree::Subtree(token_tree) => {
                    indents.set_final_indent_if(first && token_tree.tree_data.end == end);
                    self.fmt_tree(f, &token_tree, idx, indents, max_ident, false)?
                },
            }
            indents.signal_no_name_formatted();
        }
        
        if !first {
            indents.pop();
        }

        Ok(())
    }
}

impl fmt::Display for TokenTreeFormatter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.tree.stream.is_empty() {
            return Ok(())
        }

        let mut idx = 0;
        let mut indents = TreeIndentFormatter::new();
        let max_indent = self.tree.get_max_indent();
        self.fmt_tree(f, &self.tree, &mut idx, &mut indents, max_indent, true) 
    }
}

// -------------------------------------------------------------

pub enum TokenTreeBuildError {
    EndOfRootTree,
    Missing(Vec<(Span, String)>),
    Unexpected{
        span: Span,
        expected: &'static str,
        found: &'static str,
    },
}

pub struct TokenTreeBuilder {
    stack: Vec<TokenTreeData>,
}

impl TokenTreeBuilder {
    pub fn new() -> Self {
        Self {
            stack: vec![TokenTreeData::default()],
        }
    }

    pub fn begin_subtree(&mut self, start: u32) {
        let tree = TokenTreeData {
            start,
            end: 0,
            subtrees: Vec::new(),
        };
        self.stack.push(tree);
    }

    // Returns if a subtree was successfully popped
    pub fn end_subtree(&mut self, end: u32, stream: &TokenStream) -> Result<(), TokenTreeBuildError> {
        if self.stack.len() <= 1 {
            return Err(TokenTreeBuildError::EndOfRootTree);
        }
        let start_tok = &stream.tokens[self.stack.first().unwrap().start as usize];
        let end_tok = &stream.tokens[end as usize];

        // TODO: Other posibilities
        if let Token::OpenDelim(open_delim) = start_tok {
            if let Token::CloseDelim(close_delim) = end_tok {
                if open_delim != close_delim {
                    let span = stream.metadata()[end as usize].span;
                    return Err(TokenTreeBuildError::Unexpected {
                        span,
                        expected: open_delim.as_close_str(),
                        found: close_delim.as_close_str()
                    });
                }
            }
        }
        
        let mut subtree = self.stack.pop().unwrap();
        subtree.end = end;
        let last = self.stack.last_mut().unwrap();
        last.subtrees.push(subtree);

        Ok(())
    }

    pub fn finalize(mut self, stream: &TokenStream) -> Result<TokenTreeData, TokenTreeBuildError> {
        if self.stack.len() > 1 {
            let mut missing = Vec::with_capacity(self.stack.len() - 1);
            for open in self.stack.iter().skip(1) {
                let open_tok = &stream.tokens()[open.start as usize];
                let span = stream.metadata()[open.start as usize].span;
                match open_tok {
                    Token::OpenDelim(delim) => missing.push((span, delim.as_close_str().to_string())),
                    _ => missing.push((span, open_tok.to_string())),
                }
            }
            return Err(TokenTreeBuildError::Missing(missing));
        }

        // We always have at least the base tree
        let mut tree = self.stack.pop().unwrap();
        tree.end = (stream.len() - 1) as u32;
        Ok(tree)
    }
}