use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Dirs {
    pub root: PathBuf,
    pub versions: PathBuf,
    pub libraries: PathBuf,
    pub assets: PathBuf,
    pub runtime: PathBuf,
    pub instances: PathBuf,
}

impl Dirs {
    pub fn discover() -> Self {
        let root = std::env::var("ATOMIC_CLIENT_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::data_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("AtomicClient")
            });
        Self::new(root)
    }

    pub fn new(root: PathBuf) -> Self {
        let d = Dirs {
            versions: root.join("versions"),
            libraries: root.join("libraries"),
            assets: root.join("assets"),
            runtime: root.join("runtime"),
            instances: root.join("instances"),
            root,
        };
        d.ensure();
        d
    }

    pub fn ensure(&self) {
        for p in [
            &self.root,
            &self.versions,
            &self.libraries,
            &self.assets,
            &self.assets.join("indexes"),
            &self.assets.join("objects"),
            &self.runtime,
            &self.instances,
        ] {
            let _ = std::fs::create_dir_all(p);
        }
    }

    pub fn version_dir(&self, id: &str) -> PathBuf {
        self.versions.join(id)
    }
    pub fn version_json(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.json"))
    }
    pub fn client_jar(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.jar"))
    }
    pub fn index_file(&self, id: &str) -> PathBuf {
        self.assets.join("indexes").join(format!("{id}.json"))
    }
    pub fn object_path(&self, hash: &str) -> PathBuf {
        let (a, b) = hash.split_at(2.min(hash.len()));
        self.assets.join("objects").join(a).join(b)
    }
    pub fn virtual_assets(&self, id: &str) -> PathBuf {
        self.assets.join("virtual").join(id)
    }
    pub fn instance(&self, name: &str) -> PathBuf {
        self.instances.join(name)
    }
    pub fn natives(&self, version: &str) -> PathBuf {
        self.root.join("natives").join(version)
    }
    pub fn log_config(&self, id: &str) -> PathBuf {
        self.version_dir(id).join("log4j.xml")
    }
    pub fn java_home(&self, component: &str, platform: &str) -> PathBuf {
        self.runtime.join(component).join(platform)
    }
}

pub fn path_from_maven(name: &str) -> Option<String> {
    // group:artifact:version[:classifier][@ext]
    let (coords, ext) = match name.split_once('@') {
        Some((c, e)) => (c, e.to_string()),
        None => (name, "jar".to_string()),
    };
    let parts: Vec<&str> = coords.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let version = parts[2];
    let classifier = parts.get(3).map(|c| format!("-{c}")).unwrap_or_default();
    Some(format!(
        "{group}/{artifact}/{version}/{artifact}-{version}{classifier}.{ext}"
    ))
}

pub fn file_name_of(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}
