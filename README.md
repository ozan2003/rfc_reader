# RFC Reader
<!-- cool badges -->
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
![Rust](https://img.shields.io/badge/language-Rust-orange?logo=rust)
[![Stars](https://img.shields.io/github/stars/ozan2003/rfc_reader)](https://github.com/ozan2003/rfc_reader/stargazers)
[![Last Commit](https://img.shields.io/github/last-commit/ozan2003/rfc_reader)](https://github.com/ozan2003/rfc_reader/commits/master)
[![Code Size](https://img.shields.io/github/languages/code-size/ozan2003/rfc_reader)](https://github.com/ozan2003/rfc_reader)
[![dependency status](https://deps.rs/repo/github/ozan2003/rfc_reader/status.svg?path=.)](https://deps.rs/repo/github/ozan2003/rfc_reader?path=.)
![Built With Ratatui](https://img.shields.io/badge/Built_With_Ratatui-000?logo=ratatui&logoColor=fff)
<!--[![Lines of Code](https://tokei.rs/b1/github/ozan2003/rfc_reader?style=flat)](https://github.com/ozan2003/rfc_reader?=style=flat)-->

A tool to read IETF RFCs (Request for Comments) with a TUI, allowing you to fetch, cache, and browse them.

## Features

- View documents directly in the terminal
- Automatic caching of the documents for offline reading
- Text search functionality within document
- Table of contents navigation

> [!NOTE]
> Table of contents section might not always be accurate, as there's no standard way to extract it from RFCs. It works best with RFCs that have a well-defined TOC.

- Keyboard controls

## Screenshots

[![rfc-reader-normal.png](https://i.postimg.cc/VvwfYKFm/rfc-reader-normal.png)](https://postimg.cc/njdb2Y5P)

[![rfc-reader-toc.png](https://i.postimg.cc/k5kMHJ6V/rfc-reader-toc.png)](https://postimg.cc/DWPKJKcF)

[![rfc-reader-search.png](https://i.postimg.cc/nzBVbyB3/rfc-reader-search.png)](https://postimg.cc/tZRGFmr6)

## Usage

```bash
rfc_reader [OPTIONS] [RFC_NUMBER]
```

### Examples

```bash
# Read a specific RFC
rfc_reader 2616

# Read a specific RFC in offline mode (only works if previously cached)
rfc_reader --offline 2616

# Clear the RFC cache
rfc_reader --clear-cache
```

### Options

- `--offline`, `-o`: Run in offline mode (only load cached RFCs)
- `--clear-cache`: Clear the RFC cache

Refer to `rfc_reader --help` for more options.

## Controls

Refer to the [wiki](https://github.com/ozan2003/rfc_reader/wiki/Keybindings) for keybindings.

## Minimum Supported Rust Version (MSRV)

Rust 1.88.0 or newer.

## Cache Location

RFCs are cached locally to improve performance and enable offline reading.

Linux:

```bash
/home/{YOUR_USERNAME}/.config/rfc_reader
```

MacOS:

```bash
/Users/{YOUR_USERNAME}/Library/Application Support/rfc_reader
```

Windows:

```bash
C:\Users\{YOUR_USERNAME}\AppData\Roaming\rfc_reader\config
```

## Contributing

I don't know very well about contribution/PR stuff. Contact me or create a issue if for any issues or suggestions.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
