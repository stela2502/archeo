[![Crates.io](https://img.shields.io/crates/v/archeo.svg)](https://crates.io/crates/archeo)
[![Docs.rs](https://docs.rs/archeo/badge.svg)](https://docs.rs/archeo)
[![Build Status](https://github.com/stela2502/archeo/actions/workflows/rust.yml/badge.svg)](https://github.com/stela2502/archeo/actions/workflows/rust.yml)
[![License](https://img.shields.io/crates/l/archeo.svg)](LICENSE)

# 🏺 Archeo

**Reconstruct what the hell this project is**

Archeo is a command-line tool that analyzes a codebase or data folder
and produces a structured, AI-generated summary of what the project
likely does.

It is designed for: - forgotten projects
    -    inherited repositories\
    -   messy research folders\
    -    large pipelines with unclear structure

Archeo scans files, optionally inspects their contents, and uses a local
AI model (via Ollama) to generate a readable report.

------------------------------------------------------------------------

## 🚀 Features

-   📂 **Project scanning**
    Finds relevant files with configurable filters (extensions, size,
    hidden files, etc.)

-   🧠 **AI-based project understanding**
    Infers:

    -   project purpose
    -   main components
    -   workflow
    -   domain

-   📄 **Content-aware analysis (optional)**
    Reads files and summarizes their meaning before synthesizing a
    project-level overview

-   🧩 **Configurable behavior**
    Control parsing, sampling, and AI prompts per file type

-   🔒 **Local AI support**
    Works with local Ollama models --- no cloud required

------------------------------------------------------------------------

## 📦 Installation

### From crates.io (soon)

``` bash
cargo install archeo
```

### From source

``` bash
git clone https://github.com/stela2502/archeo
cd archeo
cargo build --release
```

------------------------------------------------------------------------

## 🧪 Usage

``` bash
archeo --path <PATH> [OPTIONS]
```

### Example

``` bash
archeo --path . --content-analysis --model deepseek-coder
```

This will: 1. Scan the folder 2. Analyze files 3. Run AI interpretation
4. Write a report (`AI_digest.md` by default)

------------------------------------------------------------------------

## ⚙️ Key Options

  Option                   Description
  ------------------------ ------------------------------
  `--path`                 Path to analyze
  `--output`               Output markdown file
  `--model`                Ollama model
  `--content-analysis`     Enable file content analysis
  `--content-mode`         Per-extension parsing rules
  `--content-primer`       Custom prompts per file type
  `--content-extensions`   Restrict analyzed extensions
  `--include-hidden`       Include hidden files

------------------------------------------------------------------------

## 🧠 Content Analysis Modes

Control how files are processed:

-   `full` → read full file
-   `sampled` → partial content (tables, large files)
-   `skip` → ignore

Example:

``` bash
--content-mode py=full --content-mode csv=sampled
```

------------------------------------------------------------------------

## 📊 Output

Archeo generates a markdown report containing:

    -  project overview
    -  detected structure
    -  file list
    -  optional per-file AI summaries
    -  final synthesized explanation

------------------------------------------------------------------------

## 🧬 Example Use Cases

    -  Understanding old bioinformatics pipelines
    -  Exploring unfamiliar GitHub repositories
    -  Reverse-engineering data science projects
    -  Cleaning up legacy codebases

------------------------------------------------------------------------

## 🛠️ Requirements

    -  Rust (for building)\
    -  Ollama (running locally)

------------------------------------------------------------------------

## 🤝 Contributing

Contributions are welcome.\
Open issues or PRs at:

https://github.com/stela2502/archeo

------------------------------------------------------------------------

## 📜 License

MIT License
