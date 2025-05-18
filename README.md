# RFC Reader

A CLI tool to read RFCs (Request for Comments) in the terminal. This tool allows you to fetch, cache, and browse RFC documents with a TUI.

## Features

- View RFCs directly in your terminal with a clean, navigable interface
- Automatic caching of RFCs for offline reading
- Full text search functionality within documents
- Table of contents navigation
- Keyboard-driven interface with intuitive controls

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

## Cache Location

RFCs are cached locally to improve performance and enable offline reading.
This is done via the `directories` crate.

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
C:\Users\{YOUR_USERNAME}\AppData\Roaming\rfc_reader
```

## Contributing

Contributions are welcome, feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the LICENSE file for details.
