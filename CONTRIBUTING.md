# Contributing to Atomic Client

Thank you for wanting to help! Here's how to contribute without breaking things.

## Golden Rules

1. **Never push directly to `main`** — Always create your own branch
2. **Always test before pushing** — Run `cargo build --release` and `cargo test`
3. **Don't touch files you didn't create** without asking first
4. **Never commit secrets** — No passwords, tokens, or private keys

## How to Contribute

### 1. Fork and Clone

```bash
git clone https://github.com/atomicdev1/Atomic-client-v1.git
cd Atomic-client-v1
```

### 2. Create Your Own Branch

Never work on `main`. Create a branch with your name or feature:

```bash
git checkout -b yourname-feature-name
# Example:
git checkout -b john-add-settings
```

### 3. Make Your Changes

Edit the files you need. Common files:

- **Rust code**: `src-tauri/src/engine/`
- **Tauri commands**: `src-tauri/src/commands/`
- **React UI**: `src/`
- **Config**: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`

### 4. Test Your Changes

**Always run these before pushing:**

```bash
cd src-tauri
cargo build --release    # Must compile without errors
cargo test               # All tests must pass
cargo run --release -- cli versions  # CLI must work
cargo run --release -- cli launch 1.20.1 --loader vanilla --name test  # Must launch Minecraft
```

### 5. Commit and Push Your Branch

```bash
git add .
git commit -m "your description of changes"
git push origin your-branch-name
```

### 6. Open a Pull Request

1. Go to your GitHub repo page
2. Click **Compare & pull request**
3. Describe what you changed
4. Wait for the repo owner to review and merge

---

## What NOT to Do

| ❌ Don't Do This | ✅ Do This Instead |
|------------------|-------------------|
| Push directly to `main` | Push to your own branch, then PR |
| Commit `auth.json` or secrets | They're in `.gitignore` — never commit them |
| Delete someone else's code | Ask first, or create your own branch |
| Change the Rust toolchain | Ask the project owner first |
| Commit `node_modules/` or `target/` | They're in `.gitignore` |

---

## File Ownership Guide

| Area | Files | Who Should Edit |
|------|-------|-----------------|
| Minecraft engine (download, launch, Java) | `src-tauri/src/engine/` | Core developer |
| Tauri commands | `src-tauri/src/commands/` | Core developer |
| React UI | `src/` | Frontend developer |
| Config | `src-tauri/Cargo.toml`, `tauri.conf.json` | Core developer |

---

## Communication

If you're working with a friend on this project:

- **Discuss before editing** the same file
- **Use separate branches** for different features
- **Review each other's code** before merging
- **Test together** before pushing to `main`

---

## Getting Help

If something breaks:

1. Check `cargo build --release` for errors
2. Check `cargo test` for test failures
3. Check the launcher log in the UI or at `instances/<name>/logs/atomic-launch.log`
4. Ask the project owner before making major changes

---

Thank you for contributing! 🚀