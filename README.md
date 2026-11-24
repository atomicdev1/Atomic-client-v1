# Atomic Client

A fast, native Minecraft Java Edition launcher built with Rust (core engine) + Tauri 2 (desktop shell) + React + TypeScript.

Atomic Client downloads Minecraft versions, manages Java runtimes, handles mods, and launches the game — all from a clean desktop interface.

## What This Is

A Minecraft Java launcher that:
- Downloads official Minecraft versions directly from Mojang's servers
- Automatically installs the correct Java runtime for each version
- Downloads and verifies all game libraries, assets, and native files
- Supports offline accounts (no Microsoft account required)
- Supports Fabric and Quilt mod loaders
- Searches and installs mods from Modrinth
- Saves settings and instance data locally

## What This Is NOT

- ❌ Not a Minecraft crack or pirated game client — you need a valid Minecraft account
- ❌ Not a cheat client or hacked client
- ❌ Not a virus or malware — all code is open source and auditable
- ❌ Not connected to your personal data unless you explicitly provide it

## 🎮 What Is a Minecraft Launcher?

### Simple Explanation

Minecraft Java Edition is not just one file — it's a complex program made of hundreds of smaller pieces:

- **The game code** (the actual Minecraft program)
- **Libraries** (helper code for graphics, sound, networking, etc.)
- **Assets** (textures, sounds, music, models, language files)
- **Java** (Minecraft is written in Java, so it needs Java installed to run)
- **Native files** (platform-specific code for graphics, input, etc.)

**A launcher is a helper program that does all the boring work for you:**

| Without a Launcher | With Atomic Client |
|-------------------|-------------------|
| You manually download each file from Mojang's website | One click downloads everything automatically |
| You manually install the correct Java version | Java is downloaded and installed automatically |
| You manually figure out which files go where | Files are placed in the correct folders automatically |
| You manually write the complex launch command | One click starts the game with all correct settings |
| If something breaks, you figure out why | Clear error messages tell you what went wrong |

### How Atomic Client Works (Step by Step)

```
1. You open Atomic Client
   ↓
2. Atomic asks Mojang: "What Minecraft versions exist?"
   ↓
3. You pick a version (e.g., 1.20.1)
   ↓
4. Atomic downloads:
   - The Minecraft game file (client.jar)
   - All helper libraries (~60 files)
   - Game assets (textures, sounds - ~485 MB for new versions)
   - The correct Java version for that Minecraft version
   ↓
5. Atomic verifies every file is correct (checksums)
   ↓
6. Atomic builds the launch command with all settings
   ↓
7. Atomic starts Java → Minecraft opens
   ↓
8. You play!
```

### Why Use Atomic Client?

| Feature | Benefit |
|---------|---------|
| **One-click install** | No manual file management |
| **Automatic Java** | No Java knowledge required |
| **Version switching** | Easily switch between 1.8.9, 1.20.1, 26.3, etc. |
| **Offline play** | Play without Microsoft account |
| **Mod support** | Install Fabric/Quilt mods easily |
| **Progress tracking** | See exactly what's downloading |
| **Error messages** | Clear messages if something goes wrong |

---

## Project Structure

```
atomic-client/
├── index.html              # HTML shell
├── package.json            # Frontend dependencies (React, Vite, Tauri CLI)
├── tsconfig.json           # TypeScript configuration
├── vite.config.ts          # Vite build configuration
├── .gitignore              # Files excluded from Git (secrets, Minecraft data, etc.)
├── src/                    # React frontend
│   ├── main.tsx            # React entry point
│   ├── App.tsx             # Main UI (versions, launch, settings, mods)
│   ├── api.ts              # Tauri command wrappers (frontend → Rust)
│   └── styles.css          # Styling
└── src-tauri/              # Rust backend
    ├── Cargo.toml          # Rust dependencies
    ├── build.rs            # Build script
    ├── tauri.conf.json     # Tauri window/config settings
    ├── src/
    │   ├── main.rs         # Entry point (CLI or desktop)
    │   ├── cli.rs          # Command-line interface
    │   ├── lib.rs          # Tauri desktop application
    │   ├── commands/       # Tauri commands exposed to frontend
    │   └── engine/         # Core Minecraft launcher engine
        ├── mod.rs
        ├── paths.rs        # Data directory layout
        ├── rules.rs        # Mojang version rule evaluation
        ├── manifest.rs     # Version manifest parsing
        ├── download.rs     # Parallel downloader with SHA-1 verification
        ├── java.rs         # Java runtime downloader
        ├── fabric.rs       # Fabric/Quilt loader support
        ├── args.rs         # Launch argument construction
        ├── launch.rs       # Install + launch orchestrator
        ├── auth.rs         # Microsoft device-code authentication
        ├── modrinth.rs     # Modrinth API integration
        ├── settings.rs     # Settings persistence
        ├── performance.rs  # Performance profiles
        └── version_catalogue.rs  # Minecraft version catalogue
```

---

## Quick Start

### Prerequisites

- **Windows**: No additional software needed for CLI
- **Node.js**: Required for frontend development
- **Rust**: Required for building the launcher

### Build the CLI (Command Line Interface)

```bash
cd src-tauri
cargo build --release
```

The compiled binary will be at `src-tauri/target/release/atomic-client.exe`

### Run from CLI

List available Minecraft versions:
```bash
cargo run --release -- cli versions
```

Install a version (downloads all files):
```bash
cargo run --release -- cli install 1.20.1 --loader vanilla --name Player
```

Launch Minecraft:
```bash
cargo run --release -- cli launch 1.20.1 --loader vanilla --name Player
```

CLI flags:
- `--loader vanilla|fabric|quilt` — Mod loader (default: vanilla)
- `--instance NAME` — Instance name (default: default)
- `--memory MB` — Memory allocation in MB (default: 4096)
- `--name PLAYER` — Offline username (default: Player)
- `--demo` — Demo mode
- `--dry-run` — Download only, don't launch

---

## Desktop Application (Tauri)

The desktop GUI is built with Tauri 2 + React. To build:

```bash
cd src-tauri
cargo build --release --features tauri
```

**Note**: On Windows, Tauri requires MSVC Build Tools. The GNU Rust toolchain (MinGW) works for the CLI but not for the Tauri desktop build.

### Tauri Commands

The frontend communicates with Rust through these Tauri commands:

| Command | Description |
|---------|-------------|
| `get_settings` | Get current settings |
| `save_settings` | Save settings |
| `list_versions` | List Minecraft versions |
| `list_versions_filtered` | List versions with filter + order |
| `get_version_info` | Get details for a specific version |
| `launch_game` | Install and launch Minecraft |
| `begin_login` | Start Microsoft device-code login |
| `poll_login` | Check login status |
| `current_account` | Get current account info |
| `logout` | Sign out |
| `search_mods` | Search Modrinth mods |
| `install_mod` | Install a mod |
| `data_dir` | Get data directory path |

---

## Data Directory

Atomic Client stores all data in your OS data directory:

- **Windows**: `%APPDATA%\AtomicClient\`
- **macOS**: `~/Library/Application Support/AtomicClient/`
- **Linux**: `~/.local/share/AtomicClient/`

Inside the data directory:
```
AtomicClient/
├── versions/          # Downloaded Minecraft version jars
├── libraries/         # Minecraft libraries
├── assets/            # Game assets (textures, sounds, models)
├── runtime/           # Java runtimes
├── instances/         # Game instances (saves, mods, logs)
├── natives/           # Extracted native libraries
├── settings.json      # User settings
└── auth.json          # Authentication tokens (Microsoft)
```

---

## Security

- All downloads are verified with SHA-1 checksums
- Downloads use temporary files (`.part`) and atomic rename
- Authentication tokens are stored locally in `auth.json` (not sent anywhere except Microsoft's official APIs)
- Native library extraction validates file paths to prevent directory traversal
- No personal data is sent to any server except Mojang and Microsoft (for official authentication)

---

## Contributing

1. Fork the repository
2. Create a feature branch: `git checkout -b my-feature`
3. Make your changes
4. Test: `cargo build --release` and `cargo test`
5. Submit a Pull Request

### Code Style
- Rust: Follow standard Rust conventions
- TypeScript/React: Follow existing code patterns in `src/`
- All new features should include unit tests

---

## License

This project is provided as-is. Minecraft is a trademark of Mojang Studios. Atomic Client is not affiliated with Mojang, Microsoft, or any other company.

---

## Support

If you encounter issues:
1. Check the launcher log in the UI or at `instances/<instance>/logs/atomic-launch.log`
2. Ensure you have a stable internet connection for downloads
3. Verify you have sufficient disk space (Minecraft versions can be 100+ MB each)
4. Check that your Java runtime is compatible with the selected Minecraft version