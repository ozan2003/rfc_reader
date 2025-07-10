# RFC Reader

A tool to read IETF RFCs (Request for Comments) with a TUI, allowing you to fetch, cache, and browse them.

## Features

- View documents directly in the terminal
- Automatic caching of the documents for offline reading
- Text search functionality within document
- Table of contents navigation
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
