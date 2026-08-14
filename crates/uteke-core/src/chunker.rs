//! AST-aware code chunking — splits source files by semantic boundaries.
//!
//! Uses regex-based pattern matching (no tree-sitter dependency) to detect
//! function, class, and struct definitions. Supports Rust, Go, Python,
//! TypeScript/JavaScript, and Dart.
//!
//! Also provides markdown/prose chunking (#405) — splits by headings
//! while respecting a token window.

/// Floor-clamp an index to the nearest valid UTF-8 char boundary at or before `idx`.
///
/// Polyfill for `str::floor_char_boundary` (stabilized in Rust 1.91.0).
/// We support MSRV 1.85.0, so we can't use the std method yet.
fn floor_char_boundary(s: &str, mut idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    // Walk backward until we land on a char boundary.
    while !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

/// A code chunk representing a semantic unit (function, class, etc).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CodeChunk {
    /// The source code of this chunk.
    pub content: String,
    /// Language detected from file extension or content.
    pub language: String,
    /// Symbol type: function, struct, class, impl, interface, etc.
    pub symbol_type: String,
    /// Symbol name (function/class/struct name).
    pub symbol_name: String,
}

/// A text chunk from markdown/prose splitting (#405).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TextChunk {
    /// Section heading (empty string if no heading).
    pub heading: String,
    /// The text content of this chunk.
    pub content: String,
    /// Heading level (1-6, 0 = no heading).
    pub level: u8,
    /// Character offset from start of original text.
    pub char_start: usize,
    /// Character offset end (exclusive).
    pub char_end: usize,
}

/// Chunk markdown using embedder's max_seq_len (#407).
///
/// Derives max_chars from the embedder's token limit using the
/// heuristic: ~4 chars per token. This ensures chunks fit within
/// the embedding model's sequence window.
///
/// For ONNX (256 tokens): max_chars = 256 * 4 = 1024
/// For OpenAI (8191 tokens): max_chars = 8191 * 4 = 32764
pub fn chunk_markdown_embed_aware<E: crate::embed::Embedder>(
    text: &str,
    embedder: &E,
) -> Vec<TextChunk> {
    const CHARS_PER_TOKEN: usize = 4;
    let max_chars = embedder.max_seq_len().saturating_mul(CHARS_PER_TOKEN);
    // Guard against zero or implausibly small seq_len.
    let max_chars = if max_chars < 100 { 1024 } else { max_chars };
    chunk_markdown(text, max_chars)
}

/// Chunk markdown or prose text by headings (#405).
///
/// Splits by `#`, `##`, ... headings. When a section exceeds `max_chars`,
/// falls back to paragraph-level splitting. Code blocks (``` fences) are
/// never split mid-block.
///
/// `max_chars` should be derived from `embedder.max_seq_len()` — roughly
/// 4 chars per token. For ONNX (256 tokens): ~1024 chars. For OpenAI
/// (8191 tokens): ~32K chars.
pub fn chunk_markdown(text: &str, max_chars: usize) -> Vec<TextChunk> {
    if text.trim().is_empty() {
        return vec![];
    }
    let max_chars = if max_chars == 0 { 1024 } else { max_chars };

    // Split into sections by heading lines.
    let sections = split_by_headings(text);

    let mut chunks = Vec::new();
    let mut char_offset = 0usize;

    for section in &sections {
        let section_len = section.content.len();

        if section_len <= max_chars {
            // Section fits in one chunk.
            chunks.push(TextChunk {
                heading: section.heading.clone(),
                content: section.content.clone(),
                level: section.level,
                char_start: char_offset,
                char_end: char_offset + section_len,
            });
        } else {
            // Section too large — split by paragraphs.
            // NOTE: the heading line is already part of section.content
            // (split_by_headings seeds each section with its own heading
            // line), so sub_chunks[0] already begins with the heading.
            // No prefix is needed — prepending one would duplicate it.
            let sub_chunks = split_by_paragraphs(&section.content, max_chars);
            for (i, sub) in sub_chunks.iter().enumerate() {
                chunks.push(TextChunk {
                    heading: if i == 0 {
                        section.heading.clone()
                    } else {
                        format!("{} (part {})", section.heading, i + 1)
                    },
                    content: sub.clone(),
                    level: section.level,
                    char_start: char_offset,
                    char_end: char_offset + sub.len(),
                });
            }
        }

        char_offset += section_len + 1; // +1 for the separator consumed during split
    }

    chunks
}

/// Internal: a section bounded by headings.
struct MdSection {
    heading: String,
    content: String,
    level: u8,
}

/// Split markdown into sections by heading lines.
/// Each section includes its heading line in the content.
fn split_by_headings(text: &str) -> Vec<MdSection> {
    let lines: Vec<&str> = text.lines().collect();
    let mut sections = Vec::new();
    let mut current_heading = String::new();
    let mut current_level: u8 = 0;
    let mut current_lines: Vec<&str> = Vec::new();
    let mut in_code_block = false;

    for line in &lines {
        // Track code block state — don't treat # inside code blocks as headings.
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            current_lines.push(line);
            continue;
        }

        if !in_code_block {
            // Check if this line is a heading (1-6 # marks).
            if let Some((level, title)) = parse_heading(line) {
                // Flush previous section.
                if !current_lines.is_empty() {
                    sections.push(MdSection {
                        heading: current_heading.clone(),
                        content: current_lines.join("\n"),
                        level: current_level,
                    });
                }
                current_heading = title;
                current_level = level;
                current_lines = vec![line];
                continue;
            }
        }

        current_lines.push(line);
    }

    // Flush final section.
    if !current_lines.is_empty() {
        sections.push(MdSection {
            heading: current_heading,
            content: current_lines.join("\n"),
            level: current_level,
        });
    }

    sections
}

/// Parse a markdown heading line (e.g., "## Title" → (2, "Title")).
fn parse_heading(line: &str) -> Option<(u8, String)> {
    let trimmed = line.trim_start();
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &trimmed[hashes..];
    // Must have at least one space after #s (not a tag like #hashtag).
    if !rest.starts_with(' ') && !rest.is_empty() {
        return None;
    }
    let title = rest.trim_start().trim_end();
    Some((hashes as u8, title.to_string()))
}

/// Split text by paragraphs, respecting code block boundaries.
/// Accumulates paragraphs until `max_chars` is reached.
fn split_by_paragraphs(text: &str, max_chars: usize) -> Vec<String> {
    let paragraphs: Vec<&str> = text.split("\n\n").collect();
    let mut chunks = Vec::new();
    let mut current = String::new();

    for para in &paragraphs {
        if current.len() + para.len() + 2 > max_chars && !current.is_empty() {
            // Current chunk is full — flush it.
            chunks.push(std::mem::take(&mut current).trim_end().to_string());
        }

        if para.len() > max_chars {
            // Single paragraph exceeds limit — hard split by lines.
            if !current.is_empty() {
                chunks.push(std::mem::take(&mut current).trim_end().to_string());
            }
            for line_chunk in split_long_text(para, max_chars) {
                chunks.push(line_chunk);
            }
        } else {
            if !current.is_empty() {
                current.push_str("\n\n");
            }
            current.push_str(para);
        }
    }

    if !current.is_empty() {
        chunks.push(current.trim_end().to_string());
    }

    if chunks.is_empty() {
        chunks.push(text.to_string());
    }

    chunks
}

/// Hard-split very long text by character boundary.
/// Tries to break at sentence boundaries (.), then words (space).
///
/// Uses `floor_char_boundary` to avoid splitting multi-byte UTF-8 sequences.
fn split_long_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut start = 0;

    while start < text.len() {
        let end = (start + max_chars).min(text.len());
        if end == text.len() {
            chunks.push(text[start..].to_string());
            break;
        }

        // Clamp end to a valid UTF-8 char boundary to avoid panic on
        // multi-byte characters (CJK, emoji, etc.).
        let end = floor_char_boundary(text, end);

        // Try to find a sentence boundary (.) or word boundary (space).
        let slice = &text[start..end];
        let break_at = slice
            .rfind(". ")
            .or_else(|| slice.rfind('\n'))
            .or_else(|| slice.rfind(' '))
            .map(|pos| start + pos + 1);

        let chunk_end = break_at.unwrap_or(end);
        let chunk_end = floor_char_boundary(text, chunk_end);
        // Guard against zero-length chunks that would cause an infinite loop.
        // Advance at least one full character: `start + 1` may still be a
        // non-boundary inside a multi-byte char (CJK/emoji), which would
        // floor back to `start` and loop forever.
        let chunk_end = if chunk_end <= start {
            let mut next = start + 1;
            while next < text.len() && !text.is_char_boundary(next) {
                next += 1;
            }
            next
        } else {
            chunk_end
        };
        let chunk_text = text[start..chunk_end].trim();
        if !chunk_text.is_empty() {
            chunks.push(chunk_text.to_string());
        }
        start = chunk_end;
    }

    chunks
}

/// Detect language from file extension.
pub fn detect_language(filename: &str) -> &str {
    let ext = filename.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => "rust",
        "go" => "go",
        "py" => "python",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "dart" => "dart",
        "java" | "kt" => "java",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" => "cpp",
        "rb" => "ruby",
        "swift" => "swift",
        "lua" => "lua",
        "svelte" => "svelte",
        _ => "text",
    }
}

/// Chunk source code by semantic boundaries.
///
/// Detects function, struct, class, impl, and interface definitions.
/// Falls back to line-based splitting if no patterns match.
pub fn chunk_code(content: &str, language: &str) -> Vec<CodeChunk> {
    match language {
        "rust" => chunk_rust(content),
        "go" => chunk_go(content),
        "python" => chunk_python(content),
        "typescript" | "javascript" => chunk_typescript(content),
        "dart" => chunk_dart(content),
        _ => vec![CodeChunk {
            content: content.to_string(),
            language: language.to_string(),
            symbol_type: "file".to_string(),
            symbol_name: "full".to_string(),
        }],
    }
}

/// Extract import/use statements from source code.
pub fn extract_imports(content: &str, language: &str) -> Vec<String> {
    let mut imports = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        let is_import = match language {
            "rust" => trimmed.starts_with("use "),
            "go" => trimmed.starts_with("import "),
            "python" => trimmed.starts_with("import ") || trimmed.starts_with("from "),
            "typescript" | "javascript" | "dart" => {
                trimmed.starts_with("import ")
                    || trimmed.starts_with("const ") && trimmed.contains("require(")
            }
            _ => false,
        };
        if is_import && !trimmed.is_empty() {
            imports.push(trimmed.to_string());
        }
    }
    imports
}

// ── Language-specific chunkers ──────────────────────────────────────────

fn chunk_rust(content: &str) -> Vec<CodeChunk> {
    let patterns = [
        ("fn ", "function"),
        ("struct ", "struct"),
        ("enum ", "enum"),
        ("trait ", "trait"),
        ("impl ", "impl"),
        ("macro_rules!", "macro"),
    ];

    chunk_by_patterns(content, "rust", &patterns, '{', '}')
}

fn chunk_go(content: &str) -> Vec<CodeChunk> {
    let patterns = [
        ("func ", "function"),
        ("type ", "struct"),
        ("interface{}", "interface"),
    ];

    chunk_by_patterns(content, "go", &patterns, '{', '}')
}

fn chunk_python(content: &str) -> Vec<CodeChunk> {
    let mut chunks = Vec::new();
    let mut current_name = String::new();
    let mut current_type = String::new();
    let mut current_lines: Vec<&str> = Vec::new();
    let mut in_block = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Detect new definitions
        let (new_name, new_type) = if trimmed.starts_with("def ") {
            let name = trimmed
                .trim_start_matches("def ")
                .split('(')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            (name, "function".to_string())
        } else if trimmed.starts_with("class ") {
            let name = trimmed
                .trim_start_matches("class ")
                .split('(')
                .next()
                .unwrap_or("")
                .split(':')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            (name, "class".to_string())
        } else {
            (String::new(), String::new())
        };

        if !new_name.is_empty() {
            // Save previous block
            if in_block && !current_lines.is_empty() {
                chunks.push(CodeChunk {
                    content: current_lines.join("\n"),
                    language: "python".to_string(),
                    symbol_type: current_type,
                    symbol_name: current_name,
                });
            }
            current_name = new_name;
            current_type = new_type;
            current_lines = vec![line];
            in_block = true;
        } else if in_block {
            current_lines.push(line);
        }
    }

    // Don't forget the last block
    if in_block && !current_lines.is_empty() {
        chunks.push(CodeChunk {
            content: current_lines.join("\n"),
            language: "python".to_string(),
            symbol_type: current_type,
            symbol_name: current_name,
        });
    }

    // If no chunks found, return whole file
    if chunks.is_empty() {
        chunks.push(CodeChunk {
            content: content.to_string(),
            language: "python".to_string(),
            symbol_type: "file".to_string(),
            symbol_name: "full".to_string(),
        });
    }

    chunks
}

fn chunk_typescript(content: &str) -> Vec<CodeChunk> {
    let patterns = [
        ("function ", "function"),
        ("class ", "class"),
        ("interface ", "interface"),
        ("const ", "const"), // arrow functions, consts
        ("export default ", "export"),
        ("export ", "export"),
    ];

    let mut chunks = chunk_by_patterns(content, "typescript", &patterns, '{', '}');

    // Filter: only keep const/export chunks that contain function-like syntax
    chunks.retain(|c| {
        if c.symbol_type == "const" || c.symbol_type == "export" {
            c.content.contains("=>") || c.content.contains("function")
        } else {
            true
        }
    });

    if chunks.is_empty() {
        chunks.push(CodeChunk {
            content: content.to_string(),
            language: "typescript".to_string(),
            symbol_type: "file".to_string(),
            symbol_name: "full".to_string(),
        });
    }

    chunks
}

fn chunk_dart(content: &str) -> Vec<CodeChunk> {
    let patterns = [
        ("void ", "function"),
        ("Future<", "function"),
        ("Stream<", "function"),
        ("class ", "class"),
        ("enum ", "enum"),
        ("Widget ", "widget"),
    ];

    chunk_by_patterns(content, "dart", &patterns, '{', '}')
}

/// Generic pattern-based chunker for brace-delimited languages.
fn chunk_by_patterns(
    content: &str,
    language: &str,
    patterns: &[(&str, &str)],
    open: char,
    close: char,
) -> Vec<CodeChunk> {
    let mut chunks = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        for (keyword, sym_type) in patterns {
            if trimmed.starts_with(keyword)
                || (keyword.starts_with("export") && trimmed.contains(keyword))
            {
                // Extract symbol name
                let after_kw = trimmed.trim_start_matches(*keyword);
                let name = after_kw
                    .split(|c: char| {
                        c.is_whitespace() || c == '(' || c == '<' || c == '{' || c == ':'
                    })
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();

                if name.is_empty() {
                    continue;
                }

                // Find the body by matching braces
                let body = match extract_block(&lines, i, open, close) {
                    Some(b) => b,
                    None => continue,
                };

                chunks.push(CodeChunk {
                    content: body,
                    language: language.to_string(),
                    symbol_type: sym_type.to_string(),
                    symbol_name: name,
                });
                break; // Don't match same line twice
            }
        }
    }

    if chunks.is_empty() {
        chunks.push(CodeChunk {
            content: content.to_string(),
            language: language.to_string(),
            symbol_type: "file".to_string(),
            symbol_name: "full".to_string(),
        });
    }

    chunks
}

/// Extract a brace-delimited block starting from `start_line`.
/// Returns the full text from the definition line to the closing brace.
fn extract_block(lines: &[&str], start: usize, open: char, close: char) -> Option<String> {
    let mut depth = 0i32;
    let mut found_open = false;
    let mut block_lines: Vec<&str> = Vec::new();

    for &line in &lines[start..] {
        block_lines.push(line);

        for ch in line.chars() {
            if ch == open {
                depth += 1;
                found_open = true;
            } else if ch == close {
                depth -= 1;
            }
        }

        if found_open && depth <= 0 {
            return Some(block_lines.join("\n"));
        }
    }

    // If no braces found but we have content, return a few lines
    if !block_lines.is_empty() && !found_open {
        let end = (start + 5).min(lines.len());
        return Some(lines[start..end].join("\n"));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language("main.rs"), "rust");
        assert_eq!(detect_language("main.go"), "go");
        assert_eq!(detect_language("app.py"), "python");
        assert_eq!(detect_language("App.tsx"), "typescript");
        assert_eq!(detect_language("main.dart"), "dart");
        assert_eq!(detect_language("README.md"), "text");
    }

    #[test]
    fn test_detect_language_all_extensions() {
        // Kill all match-arm deletion mutants in detect_language (lines 313-321).
        // Each extension must return the correct language, not "text".
        assert_eq!(detect_language("App.ts"), "typescript");
        assert_eq!(detect_language("app.js"), "javascript");
        assert_eq!(detect_language("App.jsx"), "javascript");
        assert_eq!(detect_language("app.mjs"), "javascript");
        assert_eq!(detect_language("app.cjs"), "javascript");
        assert_eq!(detect_language("Main.java"), "java");
        assert_eq!(detect_language("App.kt"), "java");
        assert_eq!(detect_language("main.c"), "c");
        assert_eq!(detect_language("header.h"), "c");
        assert_eq!(detect_language("src.cpp"), "cpp");
        assert_eq!(detect_language("src.cc"), "cpp");
        assert_eq!(detect_language("src.cxx"), "cpp");
        assert_eq!(detect_language("header.hpp"), "cpp");
        assert_eq!(detect_language("app.rb"), "ruby");
        assert_eq!(detect_language("App.swift"), "swift");
        assert_eq!(detect_language("script.lua"), "lua");
        assert_eq!(detect_language("Component.svelte"), "svelte");
    }

    #[test]
    fn test_extract_imports_all_languages() {
        // Kill mutant: line 348 (extract_imports → vec!["xyzzy"] / vec![String::new()]).
        // Also kills match-arm deletion mutants.
        // Rust
        let rust_code = "use std::io;\nuse serde::Serialize;\nfn main() {}";
        let imports = extract_imports(rust_code, "rust");
        assert_eq!(imports.len(), 2);
        assert!(imports[0].contains("std::io"));
        assert!(imports[1].contains("serde"));
        // Python
        let py_code = "import os\nfrom pathlib import Path\nprint('hi')";
        let py_imports = extract_imports(py_code, "python");
        assert_eq!(py_imports.len(), 2);
        // Go
        let go_code = "import \"fmt\"\nfunc main() {}";
        let go_imports = extract_imports(go_code, "go");
        assert_eq!(go_imports.len(), 1);
        // TypeScript/JavaScript
        let ts_code = "import { foo } from 'bar';\nconst baz = require('qux');";
        let ts_imports = extract_imports(ts_code, "typescript");
        assert_eq!(ts_imports.len(), 2);
        // Dart
        let dart_code = "import 'package:flutter/material.dart';\nvoid main() {}";
        let dart_imports = extract_imports(dart_code, "dart");
        assert_eq!(dart_imports.len(), 1);
    }

    #[test]
    fn test_chunk_code_dart() {
        // Kill mutant: line 336 (delete match arm "dart" in chunk_code).
        // If arm deleted, Dart code falls through to generic → single chunk.
        let code =
            "void main() {\n  print('hello');\n}\n\nclass Foo {\n  int bar() { return 42; }\n}";
        let chunks = chunk_code(code, "dart");
        // chunk_dart should find at least the function and class.
        assert!(
            chunks.len() >= 2,
            "expected Dart code to be chunked into multiple parts, got {}",
            chunks.len()
        );
    }

    #[test]
    fn test_chunk_python_multiple_defs() {
        // Kill mutants: lines 431, 449, 449 in chunk_python.
        // Tests def + class detection with multiple blocks.
        let code = "def foo():\n    return 1\n\ndef bar(x):\n    return x + 1\n\nclass Baz:\n    def method(self):\n        return True";
        let chunks = chunk_code(code, "python");
        // Should find: foo, bar, Baz (at minimum).
        assert!(
            chunks.len() >= 3,
            "expected at least 3 Python chunks, got {}: {:?}",
            chunks.len(),
            chunks.iter().map(|c| &c.symbol_name).collect::<Vec<_>>()
        );
        // Verify names.
        let names: Vec<&str> = chunks.iter().map(|c| c.symbol_name.as_str()).collect();
        assert!(names.contains(&"foo"));
        assert!(names.contains(&"bar"));
        assert!(names.contains(&"Baz"));
    }

    #[test]
    fn test_chunk_python_nested_class_method() {
        // Kill mutants: line 449 (replace && with ||, delete !) in last-block flush.
        let code = "class Calculator:\n    def add(self, a, b):\n        return a + b";
        let chunks = chunk_code(code, "python");
        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].symbol_name, "Calculator");
        assert_eq!(chunks[0].symbol_type, "class");
    }

    #[test]
    fn test_chunk_typescript_functions_and_classes() {
        // Kill mutants: lines 485-486 in chunk_typescript (replace || with &&).
        // The retain filter uses || to check for function-like syntax.
        let code = "function greet(): string {\n  return 'hello';\n}\n\nclass Foo {\n  bar(): void {}\n}\n\nconst add = (a: number, b: number): number => a + b;\n\nconst PI = 3.14;";
        let chunks = chunk_code(code, "typescript");
        // Should find: greet (function), Foo (class), add (const with =>).
        // PI should be filtered out (no => or function).
        let names: Vec<&str> = chunks.iter().map(|c| c.symbol_name.as_str()).collect();
        assert!(names.contains(&"greet"), "expected 'greet' in {:?}", names);
        assert!(names.contains(&"Foo"), "expected 'Foo' in {:?}", names);
        assert!(
            names.contains(&"add"),
            "expected 'add' (arrow fn) in {:?}",
            names
        );
        // PI should be filtered out (no function syntax).
        assert!(
            !names.contains(&"PI"),
            "PI should be filtered out, got {:?}",
            names
        );
    }

    #[test]
    fn test_chunk_go_funcs_and_types() {
        // Kill mutants in chunk_go path.
        let code = "package main\n\nimport \"fmt\"\n\nfunc greet() {\n    fmt.Println(\"hello\")\n}\n\ntype Config struct {\n    Port int\n}";
        let chunks = chunk_code(code, "go");
        assert!(
            chunks.len() >= 2,
            "expected at least 2 Go chunks, got {}",
            chunks.len()
        );
        let names: Vec<&str> = chunks.iter().map(|c| c.symbol_name.as_str()).collect();
        assert!(names.contains(&"greet"));
        assert!(names.contains(&"Config"));
    }

    #[test]
    fn test_extract_block_balanced_braces() {
        // Kill mutants in extract_block (lines 582-609).
        // Tests depth tracking with nested braces.
        let lines: Vec<&str> = vec![
            "function outer() {",
            "  let x = 1;",
            "  function inner() {",
            "    let y = 2;",
            "  }",
            "  let z = x + y;",
            "}",
            "function other() {",
            "  return;",
            "}",
        ];
        // Extract from line 0 (function outer).
        let block = extract_block(&lines, 0, '{', '}').expect("should find block");
        // Should include lines 0-6 (outer function with nested inner).
        assert!(block.contains("function outer"));
        assert!(block.contains("inner"));
        assert!(block.contains("let z"));
        // Should NOT include other().
        assert!(!block.contains("function other"));
        // Verify depth tracking: nested braces must balance.
        let open_count = block.matches('{').count();
        let close_count = block.matches('}').count();
        assert_eq!(open_count, close_count, "braces should be balanced");
    }

    #[test]
    fn test_extract_block_no_braces_fallback() {
        // Kill mutants: lines 598-606 in extract_block.
        // When no braces found, should return first 5 lines as fallback.
        let lines: Vec<&str> = vec![
            "def foo():",
            "    x = 1",
            "    y = 2",
            "    z = 3",
            "    return x",
            "extra = 99",
        ];
        let block = extract_block(&lines, 0, '{', '}').expect("should return fallback");
        // Python-style: no braces found, fallback to 5 lines.
        assert!(block.contains("foo"));
        assert!(block.contains("x = 1"));
        // Should include up to 5 lines (start + 5 = 5).
        assert!(!block.contains("extra = 99"));
    }

    #[test]
    fn test_extract_block_empty_lines() {
        // Kill mutant: line 604 (delete !) and line 604:8 (delete !).
        // Empty block_lines or found_open=true should return None.
        let lines: Vec<&str> = vec![];
        let result = extract_block(&lines, 0, '{', '}');
        assert!(result.is_none(), "empty lines should return None");
    }
    #[test]
    fn test_chunk_by_patterns_export_keyword() {
        // Kill mutants: line 533 (replace && with ||), lines 539 in chunk_by_patterns.
        // Tests export keyword detection (contains check, not starts_with).
        let code = "export function foo() {\n  return 1;\n}\n\nexport default function bar() {\n  return 2;\n}";
        let chunks = chunk_code(code, "typescript");
        // Export patterns match and produce chunks (names may vary based on
        // keyword extraction, but chunks should be produced and filtered).
        assert!(!chunks.is_empty(), "export functions should produce chunks");
        // Verify the content includes the function bodies.
        let has_foo = chunks.iter().any(|c| c.content.contains("return 1"));
        let has_bar = chunks.iter().any(|c| c.content.contains("return 2"));
        assert!(has_foo, "should find foo's body");
        assert!(has_bar, "should find bar's body");
    }

    #[test]
    fn test_chunk_rust_functions() {
        let code = r#"
fn hello() {
    println!("hello");
}

fn world(x: i32) -> i32 {
    x + 1
}
"#;
        let chunks = chunk_code(code, "rust");
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].symbol_type, "function");
        assert_eq!(chunks[0].symbol_name, "hello");
        assert_eq!(chunks[1].symbol_name, "world");
    }

    #[test]
    fn test_chunk_rust_struct() {
        let code = r#"
struct Config {
    name: String,
    port: u16,
}
"#;
        let chunks = chunk_code(code, "rust");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].symbol_type, "struct");
        assert_eq!(chunks[0].symbol_name, "Config");
    }

    #[test]
    fn test_chunk_python_functions() {
        let code = r#"
def greet(name):
    return f"Hello {name}"

class Calculator:
    def add(self, a, b):
        return a + b
"#;
        let chunks = chunk_code(code, "python");
        // def greet, class Calculator (with add method inside)
        assert!(chunks.len() >= 2);
        assert_eq!(chunks[0].symbol_name, "greet");
    }

    #[test]
    fn test_chunk_go_functions() {
        let code = r#"
package main

func main() {
    fmt.Println("hello")
}

func add(a, b int) int {
    return a + b
}
"#;
        let chunks = chunk_code(code, "go");
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn test_chunk_typescript() {
        let code = r#"
function greet(name: string): string {
    return `Hello ${name}`;
}

interface User {
    id: number;
    name: string;
}
"#;
        let chunks = chunk_code(code, "typescript");
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn test_extract_imports_rust() {
        let code = r#"
use serde::{Serialize, Deserialize};

fn main() {}
"#;
        let imports = extract_imports(code, "rust");
        assert!(
            !imports.is_empty(),
            "expected at least 1 import, got {}: {:?}",
            imports.len(),
            imports
        );
    }

    #[test]
    fn test_chunk_fallback_text() {
        let code = "some random text\nno patterns here";
        let chunks = chunk_code(code, "text");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].symbol_type, "file");
    }

    #[test]
    fn test_chunk_dart() {
        let code = r#"
class MyApp extends StatelessWidget {
  Widget build(BuildContext context) {
    return Container();
  }
}
"#;
        let chunks = chunk_code(code, "dart");
        assert!(!chunks.is_empty());
    }

    // ── Markdown chunker tests (#405) ──────────────────────────────

    #[test]
    fn test_md_simple_headings() {
        let md = "# Title 1\n\nSome content here.\n\n## Subsection\n\nMore content.";
        let chunks = chunk_markdown(md, 1024);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].heading, "Title 1");
        assert_eq!(chunks[1].heading, "Subsection");
        assert_eq!(chunks[1].level, 2);
    }

    #[test]
    fn test_md_respects_code_blocks() {
        let md = "# Code\n\n```rust\n## Not a heading\nfn main() {}\n```\n\nAfter code.";
        let chunks = chunk_markdown(md, 1024);
        // The ## inside code block should NOT create a new section.
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.contains("## Not a heading"));
    }

    #[test]
    fn test_md_large_section_splits() {
        let para = "This is a paragraph. ".repeat(100);
        let md = format!("# Big Section\n\n{para}");
        let chunks = chunk_markdown(&md, 200);
        assert!(chunks.len() > 1);
        // First chunk should have the heading.
        assert_eq!(chunks[0].heading, "Big Section");
        // Subsequent chunks should have "part N" suffix.
        assert!(chunks[1].heading.contains("part 2"));
    }

    #[test]
    fn test_md_no_headings() {
        let text = "Just some prose.\n\nNo headings here.\n\nMore text.";
        let chunks = chunk_markdown(text, 1024);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].heading, "");
    }

    #[test]
    fn test_md_empty_input() {
        assert!(chunk_markdown("", 1024).is_empty());
        assert!(chunk_markdown("   \n\n  ", 1024).is_empty());
    }

    #[test]
    fn test_md_nested_levels() {
        let md = "# H1\n\nText A\n\n### H3\n\nText B\n\n## H2\n\nText C";
        let chunks = chunk_markdown(md, 1024);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].heading, "H1");
        assert_eq!(chunks[1].heading, "H3");
        assert_eq!(chunks[1].level, 3);
        assert_eq!(chunks[2].heading, "H2");
        assert_eq!(chunks[2].level, 2);
    }

    #[test]
    fn test_md_char_offsets() {
        let md = "# A\n\nText A\n\n# B\n\nText B";
        let chunks = chunk_markdown(md, 1024);
        assert_eq!(chunks.len(), 2);
        // Offsets should be within text bounds.
        assert_eq!(chunks[0].char_start, 0);
        assert!(chunks[0].char_end > 0);
        assert!(chunks[1].char_start >= chunks[0].char_end);
    }

    #[test]
    fn test_md_hashtag_not_heading() {
        // #hashtag (no space) should not be treated as heading.
        let md = "This has #hashtag and #another\n\nText.";
        let chunks = chunk_markdown(md, 1024);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].heading, "");
    }

    // ---- Mutation-killing tests for chunker ----

    #[test]
    fn test_md_multi_section_char_offsets_exact() {
        // Kill mutant: line 138 (+= → *=) char_offset accumulation.
        // "# A\n\nText A\n\n# B\n\nText B" → section 1 content "# A\n\nText A"
        // (10 bytes) → section 2 must start at offset 11 (10 + 1 separator).
        let md = "# A\n\nText A\n\n# B\n\nText B";
        let chunks = chunk_markdown(md, 1024);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].char_start, 0);
        assert_eq!(chunks[0].char_end, 12, "section 1 is 12 bytes");
        assert_eq!(
            chunks[1].char_start, 13,
            "section 2 starts after section 1 + separator"
        );
        assert_eq!(chunks[1].char_end, 24);
    }

    #[test]
    fn test_md_subchunk_char_end_exact() {
        // Kill mutant: line 133 (+ → *) in sub-chunk path.
        // char_end = char_offset + sub.len() → mutant: char_offset * sub.len()
        // For sub-chunks, char_offset is the section start, sub.len() > 0.
        // Mutant: char_offset * sub.len() could be 0 or huge → wrong.
        let para = "Word ".repeat(100); // 500 chars
        let md = format!("# Heading\n\n{para}");
        let chunks = chunk_markdown(&md, 100);
        assert!(chunks.len() > 1);
        // Every chunk must have char_end >= char_start.
        for (i, chunk) in chunks.iter().enumerate() {
            assert!(
                chunk.char_end >= chunk.char_start,
                "chunk {i}: char_end ({}) should be >= char_start ({})",
                chunk.char_end,
                chunk.char_start
            );
            // char_end must be > 0 for all non-empty chunks (kills * at line 133
            // when char_offset=0: 0 * sub.len() = 0).
            assert!(
                chunk.char_end > 0,
                "chunk {i}: char_end ({}) must be > 0",
                chunk.char_end
            );
        }
    }

    #[test]
    fn test_md_subchunk_no_heading_duplication() {
        // Regression: section.content already contains the heading line
        // (split_by_headings seeds current_lines with the heading itself).
        // Prepending heading_prefix again duplicates the heading — embedding
        // input would see the title twice.
        let para = "X".repeat(300);
        let md = format!("# My Heading\n\n{para}");
        let chunks = chunk_markdown(&md, 100);
        assert!(chunks.len() >= 3);
        let occurrences = chunks[0].content.matches("My Heading").count();
        assert_eq!(
            occurrences, 1,
            "heading must appear exactly once in first sub-chunk, got {occurrences}: {:?}",
            chunks[0].content
        );
    }

    #[test]
    fn test_md_subchunk_heading_prefix_first_chunk_only() {
        // Kill mutants: line 119 (&& → ||, == → !=, delete !).
        // Need section large enough to split into sub_chunks WITH heading.
        // Original: heading_prefix only on i==0 AND heading non-empty.
        // Mutant && → ||: heading prefix on ALL sub_chunks (wrong).
        // Mutant == → !=: heading prefix on i!=0 (wrong).
        // Mutant delete !: heading prefix when heading_prefix IS empty (wrong).
        let para = "X".repeat(300);
        let md = format!("# My Heading\n\n{para}");
        let chunks = chunk_markdown(&md, 100);
        assert!(chunks.len() >= 3);
        // First sub-chunk CONTENT must start with heading prefix.
        // (Kills == → != and delete ! mutants: they strip the prefix.)
        assert!(
            chunks[0].content.starts_with("# My Heading"),
            "first chunk content must start with heading prefix, got: {:?}",
            &chunks[0].content[..40.min(chunks[0].content.len())]
        );
        // Second sub-chunk CONTENT must NOT repeat the heading.
        // (Kills && → || mutant: it prefixes ALL sub-chunks.)
        assert!(
            !chunks[1].content.contains("My Heading"),
            "second chunk content must not repeat heading, got: {:?}",
            &chunks[1].content[..40.min(chunks[1].content.len())]
        );
    }

    #[test]
    fn test_parse_heading_level_6_boundary() {
        // Kill mutants: line 206 (> → ==, > → >=) in parse_heading.
        // Original: hashes > 6 → None. h6 is valid (6 > 6 = false → accepted).
        // Mutant >=: 6 >= 6 = true → None → h6 rejected (wrong).
        // Mutant ==: 6 == 6 = true → None → h6 rejected (wrong).
        let md = "###### Level Six Heading\n\nBody text.";
        let chunks = chunk_markdown(md, 1024);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].heading, "Level Six Heading");
        assert_eq!(chunks[0].level, 6);
    }

    #[test]
    fn test_parse_heading_level_7_rejected() {
        // Verify headings with > 6 hashes are rejected (not a mutant test,
        // but ensures the boundary is correct from both sides).
        let md = "####### Seven Hashes\n\nBody.";
        let chunks = chunk_markdown(md, 1024);
        // Should NOT be parsed as heading.
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].heading, "");
    }

    #[test]
    fn test_md_hashtag_no_space_rejected() {
        // Kill mutant: line 211 (delete !) in parse_heading.
        // #hashtag without space should NOT be heading.
        // Mutant delete !: !rest.starts_with(' ') → rest.starts_with(' ')
        //   → #tag would check if rest starts with space → false → AND
        //   !rest.is_empty() → rest.is_empty() → false → heading accepted (wrong).
        let md = "#tag\n\nBody text here.";
        let chunks = chunk_markdown(md, 1024);
        // Should be treated as plain text, not a heading.
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].heading, "");
    }

    #[test]
    fn test_split_by_paragraphs_exact_boundary() {
        // Kill mutants on line 226:
        //   > → ==, > → >=, > → <, && → ||, + → -, + → *, delete !.
        // Also kills line 231 (> → >=), 233 (delete !), 240 (delete !), 247 (delete !).
        // Construct text where paragraph boundary exactly hits max_chars.
        // Para1 = 10 chars, para2 = 10 chars. max_chars = 24.
        // current.len() + para.len() + 2 = 0+10+2=12 > 24? No → accumulate.
        // Then current.len()=10, 10+10+2=22 > 24? No → accumulate.
        // current.len()=20+2=22 (with \n\n). Hmm, need more precise.
        let text = "AAAAAAAAAA\n\nBBBBBBBBBB";
        // Each para = 10 chars. max_chars = 12.
        // Iteration 1: current="" → 0+10+2=12 > 12? No (not strictly greater).
        //   current.is_empty() → skip flush. para.len()=10 > 12? No.
        //   current = "AAAAAAAAAA"
        // Iteration 2: current.len()=10 → 10+10+2=22 > 12? Yes → flush.
        //   But mutant > → >=: 22 >= 12? Yes → same result.
        //   Mutant > → ==: 22 == 12? No → does NOT flush → accumulates wrong!
        //   Mutant > → <: 22 < 12? No → does NOT flush → accumulates wrong!
        let chunks = chunk_markdown(text, 12);
        // With max_chars=12: first para fits, second triggers flush.
        // Expect 2 chunks.
        assert!(
            chunks.len() >= 2,
            "expected at least 2 chunks for 2 paragraphs with max_chars=12"
        );
    }

    #[test]
    fn test_split_by_paragraphs_oversized_single() {
        // Kill mutant: line 231 (> → >=) in split_by_paragraphs.
        // para.len() > max_chars → hard split.
        // Mutant >=: para.len() == max_chars also triggers hard split.
        // Need para exactly == max_chars to differentiate.
        let text = "A".repeat(50);
        // max_chars = 50. para.len() = 50.
        // Original: 50 > 50 = false → accumulated normally.
        // Mutant >=: 50 >= 50 = true → hard split triggered (wrong path).
        let md = format!("# Heading\n\n{text}");
        let chunks = chunk_markdown(&md, 50);
        // With max_chars=50 and para=50: section heading (9 chars) + para (50).
        // Should split but the para itself should NOT be hard-split
        // (50 > 50 is false in original).
        // The exact chunk count depends on heading overhead, but we verify
        // no chunk is empty.
        for chunk in &chunks {
            assert!(!chunk.content.is_empty(), "no chunk should be empty");
        }
    }

    #[test]
    fn test_split_long_text_exact_multiple() {
        // Kill mutant: line 266 (< → <=) in split_long_text.
        // while start < text.len() → mutant: while start <= text.len().
        // If text.len() is exact multiple of max_chars, the mutant will
        // iterate one extra time when start == text.len(), slicing text[len..]
        // which is empty → produces an extra empty chunk.
        let text = "A".repeat(20);
        let chunks = chunk_markdown(&text, 10);
        // 20 chars / 10 = exactly 2 chunks.
        // Mutant <= would produce 3 chunks (extra empty one).
        // But chunk_markdown wraps, so verify no empty content chunks.
        for chunk in &chunks {
            assert!(!chunk.content.is_empty(), "chunk should not be empty");
        }
    }

    #[test]
    fn test_split_long_text_multibyte_tiny_limit() {
        // Regression: original guard used (start+1).min(len) then floored —
        // inside a 3-byte CJK char that floors back to `start` → infinite
        // loop on inputs like "日本語" with max_chars < 3.
        let text = "日本語テスト";
        let chunks = chunk_markdown(text, 2);
        assert!(
            !chunks.is_empty(),
            "multibyte tiny-limit split must terminate"
        );
        // Reassembly must preserve all characters (no char loss/corruption).
        let reassembled = chunks
            .iter()
            .map(|c| c.content.as_str())
            .collect::<String>();
        assert_eq!(reassembled.chars().count(), 6, "no chars may be lost");
        // Kill mutant: while start < text.len() → <=. The mutant re-enters
        // the loop once more at start == len and pushes an EXTRA EMPTY chunk.
        assert_eq!(
            chunks.len(),
            6,
            "exactly one chunk per CJK char, got {chunks:?}"
        );
    }

    #[test]
    fn test_split_long_text_sentence_split_exact() {
        // Kill mutants at break_at: |pos| start + pos + 1 (+ → *, + → -).
        // Direct call (private fn is visible to this module).
        // text = "AA. BB CC" (9 bytes), max=6 → slice "AA. BB", rfind(". ")=2
        // → break_at = 0+2+1 = 3 → chunk "AA."; remainder "BB CC".
        let chunks = split_long_text("AA. BB CC", 6);
        assert_eq!(chunks.len(), 2, "got {chunks:?}");
        assert_eq!(chunks[0], "AA.", "+→* gives 'AA', +→- gives 'A'");
        assert_eq!(
            chunks[1], " BB CC",
            "space after period stays in next chunk"
        );
    }

    #[test]
    fn test_extract_imports_ts_const_require() {
        // Kill mutant: line 351 (&& → ||) — starts_with("const ") alone
        // would count ANY const as an import. Only const + require() counts.
        let code = "const PI = 3.14;\nconst fs = require(\"fs\");\nlet x = 1;";
        let imports = extract_imports(code, "typescript");
        assert_eq!(imports.len(), 1, "got {imports:?}");
        assert_eq!(imports[0], "const fs = require(\"fs\");");
    }

    #[test]
    fn test_chunk_python_two_defs_flush() {
        // Kill mutants: line 428 (&& → ||) save-previous-block and
        // line 443 (delete !) last-block flush.
        // Mutant &&→||: on FIRST def (in_block=false, current_lines empty)
        // false||true → pushes an EMPTY chunk before the real ones.
        // Mutant delete ! at last flush: drops the last block → fallback.
        let code = "def a():\n    return 1\n\ndef b():\n    return 2";
        let chunks = chunk_python(code);
        assert_eq!(chunks.len(), 2, "got {chunks:?}");
        assert_eq!(chunks[0].symbol_name, "a");
        assert_eq!(chunks[1].symbol_name, "b");
        assert_eq!(chunks[0].content, "def a():\n    return 1\n");
    }

    #[test]
    fn test_chunk_python_no_defs_fallback() {
        // Kill mutant: line 443 (&& → ||) last-block flush.
        // With no defs at all: in_block=false, current_lines empty →
        // original falls through to the whole-file fallback chunk.
        // Mutant (false||true) pushes an EMPTY chunk and skips the
        // fallback → symbol_name "" instead of "full".
        let code = "# just a comment\nx = 1\n";
        let chunks = chunk_python(code);
        assert_eq!(chunks.len(), 1, "got {chunks:?}");
        assert_eq!(chunks[0].symbol_type, "file");
        assert_eq!(chunks[0].symbol_name, "full");
        assert_eq!(chunks[0].content, code);
    }

    #[test]
    fn test_chunk_rust_comment_with_keyword_no_match() {
        // Kill mutant: line 546 inner (&& → ||) — mid-line keyword matches.
        // A comment CONTAINING "fn " (no braces in the comment!) must NOT
        // chunk; only lines STARTING with the keyword may.
        let code = "// calls fn helper below\nfn real() {\n    1\n}";
        let chunks = chunk_code(code, "rust");
        assert_eq!(chunks.len(), 1, "got {chunks:?}");
        assert_eq!(chunks[0].symbol_name, "real");
        assert_eq!(chunks[0].symbol_type, "function");
    }

    #[test]
    fn test_chunk_ts_default_export_contains() {
        // Kill mutant: line 545 outer (|| → &&) — `export default` does not
        // start with "export default " via starts_with... wait, it does.
        // The inner contains() arm exists for lines like `};` — keep an
        // exact-behavior test for the export default pattern.
        let code = "export default function foo() {\n    return 1;\n}";
        let chunks = chunk_code(code, "typescript");
        assert_eq!(chunks.len(), 1, "got {chunks:?}");
        assert_eq!(chunks[0].symbol_type, "export");
        assert!(chunks[0].content.contains("function foo"));
    }

    #[test]
    fn test_split_by_paragraphs_exact_len_leading_space() {
        // Kill mutant: line 220 (> → >=). A paragraph of EXACTLY max_chars
        // with leading whitespace: original accumulates it (final flush does
        // trim_end only, keeping leading spaces); mutant hard-splits via
        // split_long_text which fully trims → loses the leading spaces.
        let chunks = split_by_paragraphs("AA\n\n  BBBB", 6);
        assert_eq!(chunks.len(), 2, "got {chunks:?}");
        assert_eq!(chunks[0], "AA");
        assert_eq!(chunks[1], "  BBBB", "leading spaces must survive");
    }

    #[test]
    fn test_split_by_paragraphs_exact_len_trailing_space() {
        // Kill mutant: line 220 (> → >=). "AAAA  " has len 6 == max_chars.
        // Original: accumulate path → final flush trim_end → "AAAA".
        // Mutant (>=): hard-split path → split_long_text early-exit (line
        // 257: end == text.len()) pushes the slice UNTRIMMED → "AAAA  ".
        // The trailing spaces are observable → mutant killed.
        let chunks = split_by_paragraphs("AAAA  ", 6);
        assert_eq!(chunks.len(), 1, "got {chunks:?}");
        assert_eq!(chunks[0], "AAAA", "trailing spaces must be trimmed");
    }

    #[test]
    fn test_chunk_rust_generic_struct_name() {
        // Kill mutant: line 533 first-ish || → && ('<' no longer splits).
        // "struct Foo<T> {": original name "Foo" (split at '<');
        // mutant keeps going to the space → "Foo<T>".
        let code = "struct Foo<T> {\n    x: T,\n}";
        let chunks = chunk_code(code, "rust");
        assert_eq!(chunks.len(), 1, "got {chunks:?}");
        assert_eq!(chunks[0].symbol_name, "Foo");
        assert_eq!(chunks[0].symbol_type, "struct");
    }

    #[test]
    fn test_chunk_ts_class_no_space_brace() {
        // Kill mutant: line 533 later || → && ('{' and ':' no longer split).
        // "class Foo{": original name "Foo" (split at '{');
        // mutant finds no delimiter → "Foo{".
        let code = "class Foo{\n    a: number;\n}";
        let chunks = chunk_code(code, "typescript");
        assert_eq!(chunks.len(), 1, "got {chunks:?}");
        assert_eq!(chunks[0].symbol_name, "Foo");
        assert_eq!(chunks[0].symbol_type, "class");
    }

    #[test]
    fn test_split_by_paragraphs_plus_two_boundary() {
        // Kill mutant: line 226 second + → - (len + para - 2 > max).
        // len+para = 9: original 9+2=11 > 10 → flush; mutant 9-2=7 ≤ 10 → merge.
        let chunks = split_by_paragraphs("AAAA\n\nBBBBB", 10);
        assert_eq!(
            chunks.len(),
            2,
            "original flushes at 11 > 10, got {chunks:?}"
        );
        assert_eq!(chunks[0], "AAAA");
        assert_eq!(chunks[1], "BBBBB");
    }

    #[test]
    fn test_split_long_text_sentence_boundary_offsets() {
        // Kill mutants: lines 283 (+ → *, + → -), 290 (+ → *) in split_long_text.
        // line 283: .map(|pos| start + pos + 1)
        //   mutant + → *: start + pos * 1 = start + pos (no +1 → off by one)
        //   mutant + → -: start + pos - 1 (off by one backward)
        // line 290: (start + 1).min(text.len())
        //   mutant + → *: start * 1 = start → floor_char_boundary(text, start)
        //   → chunk_end <= start → infinite loop guard → no progress
        // Need text with sentence boundary (". ") to trigger break_at path.
        let text = "First sentence. Second part. Third segment.";
        // max_chars=20: slice = "First sentence. Sec"
        // rfind(". ") = position of ". " before "Second" → break_at = start + pos + 1
        let chunks = chunk_markdown(text, 20);
        // Should split at sentence boundary, not mid-word.
        assert!(chunks.len() >= 2, "expected split at sentence boundary");
        // Verify no chunk is empty (kills + → * and + → - at line 283).
        for chunk in &chunks {
            assert!(!chunk.content.is_empty());
        }
    }

    #[test]
    fn test_split_long_text_no_word_boundary_progress() {
        // Kill mutant: line 290 (+ → *) in split_long_text.
        // This line is the guard: if chunk_end <= start, force start+1.
        // Mutant + → *: (start * 1) = start → floor_char_boundary(text, start)
        //   → if start is valid boundary, returns start → chunk_end = start
        //   → chunk_text = text[start..start] = "" → skip → start = chunk_end = start
        //   → INFINITE LOOP → test times out, caught as timeout not missed.
        // So this mutant is actually unviable (timeout), not missed.
        // We test with text that has no spaces to force the guard path.
        let text = "A".repeat(30);
        let chunks = chunk_markdown(&text, 10);
        // Should produce 3 chunks without infinite loop.
        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert!(!chunk.content.is_empty());
        }
    }

    #[test]
    fn test_split_by_paragraphs_flush_and_accumulate() {
        // Kill mutants: lines 226 (all), 231 (> → >=), 233/240/247 (delete !).
        // Constructed so chunk count is EXACT — any operator mutation changes it.
        // Paragraphs: "AAAA"(4) "\n\n" "BBBB"(4) "\n\n" "CCCC"(4)
        // max_chars = 10:
        //  - para A: current="" → 0+4+2=6 > 10? NO → accumulate. current="AAAA"
        //  - para B: 4+4+2=10 > 10? NO → accumulate. current="AAAA\n\nBBBB" (10)
        // - para C: 10+4+2=16 > 10? YES → flush "AAAA\n\nBBBB", accumulate C
        // Final flush → 2 chunks total. Mutant > → >= at 226: A: 6>=10 no;
        //   B: 10>=10 YES → flush early → 3 chunks → caught.
        let text = "AAAA\n\nBBBB\n\nCCCC";
        let chunks = split_by_paragraphs(text, 10);
        assert_eq!(chunks.len(), 2, "expected exactly 2 chunks, got {chunks:?}");
        assert_eq!(chunks[0], "AAAA\n\nBBBB");
        assert_eq!(chunks[1], "CCCC");
    }

    #[test]
    fn test_split_by_paragraphs_accumulate_separator() {
        // Kill mutant: line 240 (delete !) — "\n\n" separator only added
        // when current is non-empty. delete ! → always prepend "\n\n" →
        // first paragraph starts with leading "\n\n" → exact content check.
        let chunks = split_by_paragraphs("AA\n\nBB", 100);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "AA\n\nBB", "no leading separator allowed");
    }

    // Shared mock embedder for embed-aware chunking tests.
    struct MockEmbedder {
        seq_len: usize,
    }
    impl crate::embed::Embedder for MockEmbedder {
        fn embed(&self, _text: &str) -> Result<Vec<f32>, crate::Error> {
            Ok(vec![0.0; 8])
        }
        fn dims(&self) -> usize {
            8
        }
        fn max_seq_len(&self) -> usize {
            self.seq_len
        }
        fn name(&self) -> &str {
            "mock"
        }
    }

    #[test]
    fn test_embed_aware_chunking() {
        // 256 tokens * 4 chars/token = 1024 chars.
        let embedder = MockEmbedder { seq_len: 256 };
        let long_text = "A".repeat(2000);
        let chunks = chunk_markdown_embed_aware(&long_text, &embedder);
        // Should split into chunks of ~1024 chars.
        assert!(
            chunks.len() > 1,
            "expected multiple chunks for 2000 chars with 1024 limit"
        );
        assert!(chunks[0].content.len() <= 1024);
    }

    #[test]
    fn test_chunk_markdown_zero_max_chars_fallback() {
        // Kill mutant: replace == with != on line 85.
        // Original: max_chars == 0 → fallback to 1024.
        // Mutant !=: max_chars != 0 → 1024 is used when max_chars != 0,
        //   and original max_chars (0) is used when == 0 → empty/tiny chunks.
        // With max_chars=0 and mutant, chunks would have 0-char limit → many tiny chunks.
        let text = "A".repeat(50);
        let chunks = chunk_markdown(&text, 0);
        // Original: 0 → fallback to 1024 → 1 chunk of 50 chars.
        assert_eq!(chunks.len(), 1, "max_chars=0 should fallback to 1024");
        assert_eq!(chunks[0].content.len(), 50);
    }

    #[test]
    fn test_chunk_markdown_embed_aware_small_seq_len_fallback() {
        // Kill mutants on line 68: replace < with ==, >, <= in guard.
        // seq_len=10 → max_chars=40 → 40 < 100 → fallback to 1024.
        // Mutants:
        //   == : 40 == 100 = false → uses 40 (wrong: tiny chunks)
        //   >  : 40 > 100 = false → uses 40 (wrong)
        //   <= : at boundary 100, different result (see next test)
        let embedder = MockEmbedder { seq_len: 10 };
        let long_text = "A".repeat(500);
        let chunks = chunk_markdown_embed_aware(&long_text, &embedder);
        // With fallback to 1024, 500 chars fits in 1 chunk.
        assert_eq!(
            chunks.len(),
            1,
            "seq_len=10 should fallback to 1024 chars, fitting 500 in 1 chunk"
        );
    }

    #[test]
    fn test_chunk_markdown_embed_aware_boundary_100() {
        // Kill mutant: replace < with <= on line 68.
        // seq_len=25 → max_chars=100 → original: 100 < 100 = false → use 100.
        // Mutant <= : 100 <= 100 = true → fallback to 1024 (wrong: larger chunks).
        let embedder = MockEmbedder { seq_len: 25 };
        let long_text = "A".repeat(250);
        let chunks = chunk_markdown_embed_aware(&long_text, &embedder);
        // With max_chars=100, 250 chars → 3 chunks (100+100+50).
        assert!(
            chunks.len() >= 2,
            "seq_len=25 → max_chars=100, should split 250 chars into multiple chunks"
        );
        assert!(
            chunks[0].content.len() <= 100,
            "first chunk should be ≤100 chars with seq_len=25"
        );
    }

    #[test]
    fn test_split_long_text_multibyte_utf8() {
        // Text with multi-byte UTF-8 characters (emoji, CJK).
        // Each emoji is 4 bytes; CJK chars are 3 bytes.
        let text = "🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉中文测试中文字符";
        let chunks = split_long_text(text, 10);
        // Should not panic and should produce valid UTF-8 strings.
        for chunk in &chunks {
            assert!(
                std::str::from_utf8(chunk.as_bytes()).is_ok(),
                "chunk is not valid UTF-8"
            );
        }
        // Reconstructed text should cover the full input (possibly with minor whitespace).
        let combined: String = chunks.concat();
        assert!(combined.contains("🎉"));
        assert!(combined.contains("中文"));
    }

    #[test]
    fn test_floor_char_boundary() {
        // "héllo" — 'é' is 2 bytes (0xC3 0xA9).
        let s = "héllo";
        assert_eq!(floor_char_boundary(s, 0), 0);
        assert_eq!(floor_char_boundary(s, 1), 1); // 'h' boundary
        assert_eq!(floor_char_boundary(s, 2), 1); // middle of 'é' → floor to 1
        assert_eq!(floor_char_boundary(s, 3), 3); // 'l' boundary
        assert_eq!(floor_char_boundary(s, usize::MAX), s.len());
    }
}
