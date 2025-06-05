use std::collections::HashMap;
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

use async_trait::async_trait;
use cainome::parser::tokens::{Composite, Function};

use crate::error::BindgenResult;
use crate::{DojoContract, DojoData};

pub mod recs;
pub mod typescript;
pub mod typescript_v2;
pub mod unity;
pub mod unrealengine;
pub mod golang;

#[derive(Debug)]
pub enum BuiltinPlugins {
    Typescript,
    Unity,
    UnrealEngine,
    TypeScriptV2,
    Recs,
    Golang,
}

impl fmt::Display for BuiltinPlugins {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuiltinPlugins::Typescript => write!(f, "typescript"),
            BuiltinPlugins::Unity => write!(f, "unity"),
            BuiltinPlugins::UnrealEngine => write!(f, "unrealengine"),
            BuiltinPlugins::TypeScriptV2 => write!(f, "typescript_v2"),
            BuiltinPlugins::Recs => write!(f, "recs"),
            BuiltinPlugins::Golang => write!(f, "golang"),
        }
    }
}

#[derive(Debug)]
pub struct Buffer(Vec<String>);
impl Buffer {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn has(&self, s: &str) -> bool {
        self.0.iter().any(|b| b.contains(s))
    }

    pub fn push(&mut self, s: String) {
        self.0.push(s.clone());
    }

    /// Inserts string after the first occurrence of the separator.
    ///
    /// * `s` - The string to insert.
    /// * `pos` - The string inside inner vec to search position for.
    /// * `sep` - The separator to search for.
    /// * `idx` - The index of the separator to insert after.
    pub fn insert_after(&mut self, s: String, pos: &str, sep: &str, idx: usize) {
        let pos = self.pos(pos).unwrap();
        if let Some(st) = self.0.get_mut(pos) {
            let indices = st.match_indices(sep).map(|(i, _)| i).collect::<Vec<usize>>();
            let append_after = indices[indices.len() - idx] + 1;
            st.insert_str(append_after, &s);
        }
    }

    /// Inserts string at the specified position.
    ///
    /// * `s` - The string to insert.
    /// * `pos` - The position to insert the string at.
    /// * `idx` - The index of the string to insert at.
    pub fn insert_at(&mut self, s: String, pos: usize, idx: usize) {
        if let Some(st) = self.0.get_mut(idx) {
            st.insert_str(pos + 1, &s);
        }
    }

    /// Finds position of the given string in the inner vec.
    ///
    /// * `pos` - The string to search for.
    pub fn pos(&self, pos: &str) -> Option<usize> {
        self.0.iter().position(|b| b.contains(pos))
    }

    pub fn join(&mut self, sep: &str) -> String {
        self.0.join(sep)
    }

    /// At given index, finds the first occurrence of the needle string after the search string.
    ///
    /// * `needle` - The string to search for.
    /// * `search` - The string to search after.
    /// * `idx` - The index to search at.
    pub fn get_first_after(&self, needle: &str, search: &str, idx: usize) -> Option<usize> {
        if let Some(st) = self.0.get(idx) {
            let indices = st.match_indices(needle).map(|(i, _)| i).collect::<Vec<usize>>();
            if indices.is_empty() {
                return None;
            }

            let start = indices[indices.len() - 1] + 1;
            let search_indices = st.match_indices(search).map(|(i, _)| i).collect::<Vec<usize>>();
            return search_indices.iter().filter(|&&i| i > start).min().copied();
        }
        None
    }

    /// At given index, finds the first occurrence of the needle string before the position in
    /// string
    ///
    /// * `search` - The token to search for.
    /// * `pos` - Starting position of the search.
    /// * `idx` - The index to search at.
    pub fn get_first_before_pos(&self, search: &str, pos: usize, idx: usize) -> Option<usize> {
        if let Some(st) = self.0.get(idx) {
            let indices = st.match_indices(search).map(|(i, _)| i).collect::<Vec<usize>>();
            if indices.is_empty() {
                return None;
            }

            return indices.iter().filter(|&&i| i < pos).max().copied();
        }
        None
    }
}

impl Deref for Buffer {
    type Target = Vec<String>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for Buffer {
    fn deref_mut(&mut self) -> &mut Vec<String> {
        &mut self.0
    }
}

#[async_trait]
pub trait BuiltinPlugin: Sync {
    /// Generates code by executing the plugin.
    ///
    /// # Arguments
    ///
    /// * `data` - Dojo data gathered from the compiled project.
    async fn generate_code(&self, data: &DojoData) -> BindgenResult<HashMap<PathBuf, Vec<u8>>>;
}

pub trait BindgenWriter: Sync {
    /// Writes the generated code to the specified path.
    ///
    /// # Arguments
    ///
    /// * `code` - The generated code.
    fn write(&self, path: &str, data: &DojoData) -> BindgenResult<(PathBuf, Vec<u8>)>;
    fn get_path(&self) -> &str;
}

pub trait BindgenModelGenerator: Sync {
    /// Generates code by executing the plugin.
    /// The generated code is written to the specified path.
    /// This will write file sequentially (for now) so we need one generator per part of the file.
    /// (header, type definitions, interfaces, functions and so on)
    /// TODO: add &mut ref to what's currently generated to place specific code at specific places.
    ///
    /// # Arguments
    fn generate(&self, token: &Composite, buffer: &mut Buffer) -> BindgenResult<String>;
}

pub trait BindgenContractGenerator: Sync {
    fn generate(
        &self,
        contract: &DojoContract,
        token: &Function,
        buffer: &mut Buffer,
    ) -> BindgenResult<String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_get_first_after() {
        let mut buff = Buffer::new();
        buff.push("import { DojoProvider } from \"@dojoengine/core\";".to_owned());
        buff.push("return { actions: { changeTheme, increaseGlobalCounter, } };".to_owned());
        let pos = buff.get_first_after("actions: {", "}", 1);

        assert_eq!(pos, Some(56));
    }

    #[test]
    fn test_buffer_get_first_before() {
        let mut buff = Buffer::new();
        buff.push("import { DojoProvider } from \"@dojoengine/core\";".to_owned());
        buff.push("return { actions: { changeTheme, increaseGlobalCounter, } };".to_owned());
        let pos = buff.get_first_before_pos(",", 56, 1);

        assert_eq!(pos, Some(54));
    }
}
