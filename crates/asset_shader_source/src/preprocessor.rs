use std::collections::HashSet;

pub struct Preprocessor {
    pub lines: Vec<String>,
    pub defines: HashSet<String>,
    pub imports: HashSet<String>,
}

impl Preprocessor {
    pub fn new(src: &str) -> Self {
        let lines = src.lines().map(str::to_string).collect();
        Self {
            lines,
            defines: Default::default(),
            imports: Default::default(),
        }
    }

    pub fn with_defines(mut self, defines: impl IntoIterator<Item = String>) -> Self {
        self.defines = defines.into_iter().collect();
        self
    }

    pub fn define(&mut self, def: impl Into<String>) {
        self.defines.insert(def.into());
    }

    pub fn is_defined(&self, def: &str) -> bool {
        self.defines.contains(def)
    }

    pub fn preprocess(&mut self) -> Result<(), String> {
        let mut if_depth: i32 = 0;

        let mut ignore_line = false;
        let mut depth_of_ignore_line = if_depth;

        let mut i: usize = 0;
        while i < self.lines.len() {
            let line = std::mem::take(&mut self.lines[i]);

            match parse_directive(&line) {
                Some(directive) => {
                    self.lines.remove(i);

                    match directive {
                        Directive::IfDef(define) => {
                            if_depth += 1;
                            if !self.is_defined(define) {
                                ignore_line = true;
                                depth_of_ignore_line = if_depth;
                            }
                        }
                        Directive::IfNDef(define) => {
                            if_depth += 1;
                            if self.is_defined(define) {
                                ignore_line = true;
                                depth_of_ignore_line = if_depth;
                            }
                        }
                        Directive::EndIf => {
                            if ignore_line && depth_of_ignore_line == if_depth {
                                ignore_line = false;
                                depth_of_ignore_line = 0;
                            }
                            if_depth -= 1;
                        }
                        Directive::Define(define) => {
                            self.define(define);
                        }
                        Directive::Import(import) => {
                            todo!("import {}", import);
                        }
                    }
                }
                None => {
                    if ignore_line {
                        self.lines.remove(i);
                    } else {
                        self.lines[i] = line;
                        i += 1;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn source(&self) -> String {
        self.lines.join("\n")
    }
}

fn parse_directive(line: &str) -> Option<Directive> {
    let line = line.trim();
    if let Some(define) = line.strip_prefix("#ifdef ") {
        Some(Directive::IfDef(define.trim()))
    } else if let Some(define) = line.strip_prefix("#ifndef ") {
        Some(Directive::IfNDef(define.trim()))
    } else if let Some(_) = line.strip_prefix("#endif") {
        Some(Directive::EndIf)
    } else if let Some(define) = line.strip_prefix("#define ") {
        Some(Directive::Define(define.trim()))
    } else if let Some(import) = line.strip_prefix("#import ") {
        Some(Directive::Import(import.trim()))
    } else {
        None
    }
}

enum Directive<'a> {
    IfDef(&'a str),
    IfNDef(&'a str),
    EndIf,
    Define(&'a str),
    Import(&'a str),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stuff() {
        // TODO
    }
}
