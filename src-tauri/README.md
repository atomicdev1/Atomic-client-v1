# Atomic Client — Rust Engine

The core launcher engine written in Rust. Handles all Minecraft download, installation, Java management, and launch logic.

## Architecture

```
src-tauri/src/engine/
├── mod.rs              # Module declarations
├── paths.rs            # Data directory paths and helpers
├── rules.rs            # Mojang version rule evaluation (OS/arch/version/features)
├── manifest.rs         # Version manifest parsing and fetching
├── download.rs         # Parallel downloader with SHA-1 verification
├── java.rs             # Java runtime downloader and manager
├── fabric.rs           # Fabric/Quilt mod loader support
├── args.rs             # Launch argument construction with ${} substitution
├── launch.rs           # Install + launch orchestrator
├── auth.rs             # Microsoft device-code authentication
├── modrinth.rs         # Modrinth API integration
├── settings.rs         # Settings persistence (JSON)
├── performance.rs      # Performance profiles (default/balanced/performance)
└── version_catalogue.rs # Minecraft version catalogue (filtering, ordering, install status)
```

## Data Flow

```
1. User selects version in UI
   ↓
2. list_versions command fetches version_manifest_v2.json from Mojang
   ↓
3. User clicks Install/Launch
   ↓
4. launch_game command:
   a. Downloads version JSON from piston-meta.mojang.com
   b. Downloads client jar from piston-data.mojang.com
   c. Downloads libraries from libraries.minecraft.net
   d. Extracts native libraries to instances/<name>/natives/
   e. Downloads assets from resources.download.minecraft.net
   f. Downloads Java runtime from launchermeta.mojang.com
   g. Builds classpath and launch arguments
   h. Spawns java.exe/java process
   i. Streams stdout/stderr to log file and UI
```

## Data Directory Layout

```
<data_dir>/
├── versions/<id>/<id>.jar       # Minecraft client jar
├── versions/<id>/<id>.json      # Version metadata
├── libraries/<path>/<file.jar>  # Minecraft libraries
├── assets/indexes/<id>.json     # Asset index
├── assets/objects/<hash>        # Game assets (textures, sounds, etc.)
├── runtime/<component>/<arch>/  # Java runtimes
├── instances/<name>/            # Game instances
│   ├── mods/                    # Mod files
│   ├── logs/                    # Log files
│   ├── saves/                   # World saves
│   └── options.txt              # Game settings
├── natives/<version>/           # Extracted native libraries
├── settings.json                # User settings
└── auth.json                    # Microsoft auth tokens
```

## Key Design Decisions

### Download Safety
- All downloads verified with SHA-1 checksums from Mojang metadata
- Files written to `.part` temp file, then atomically renamed
- Duplicate URLs are deduplicated (downloaded once)
- Existing files with matching size are skipped

### Native Extraction Safety
- All ZIP entry names are sanitized before extraction
- `../` traversal, absolute paths, and drive letters are rejected
- Only forward-slash paths are accepted internally
- Files are extracted to per-JAR subdirectories to avoid DLL conflicts

### Java Runtime
- Mojang's java-runtime manifest is used to find the correct Java version
- Each Minecraft version specifies its required Java major version (8, 17, 21, 25, etc.)
- Runtimes are downloaded per-component and stored separately
- User can override with custom Java path in settings

### Authentication
- Microsoft device-code flow (OAuth 2.0)
- Tokens stored locally in `auth.json` (not sent anywhere except Microsoft APIs)
- Offline accounts supported without Microsoft account
- No passwords stored by Atomic Client

## Building

### CLI Only (Windows, macOS, Linux)
```bash
cargo build --release
```

### Desktop App (Tauri) - Windows requires MSVC
```bash
cargo build --release --features tauri
```

## Testing

```bash
cargo test
```

Tests cover:
- Version manifest parsing
- Version rule evaluation (OS/arch/version/features)
- Launch argument substitution
- Java version selection
- Settings save/load and malformed JSON recovery
- Memory allocation bounds
- Performance profile application
- Settings restoration
- SHA-1 download integrity
- Path handling

## Security

- All network requests use HTTPS (rustls-tls)
- No secrets or credentials in source code
- Authentication tokens stored locally only
- ZIP entry names validated to prevent path traversal
- File paths validated before writing