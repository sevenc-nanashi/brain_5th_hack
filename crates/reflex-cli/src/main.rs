use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::{
    env,
    ffi::OsString,
    fs,
    hash::{DefaultHasher, Hash, Hasher},
    path::{Component, Path, PathBuf},
    process::Command,
};
use usage_rs::{Cli, Subcommands};

#[derive(Cli)]
#[usage(
    bin = "reflex",
    version,
    about = "Build and deploy Reflex applications",
    unknown_flags = "error"
)]
struct Cli {
    #[usage(long, global)]
    manifest_path: Option<PathBuf>,
    #[usage(long, global)]
    target_dir: Option<PathBuf>,
    #[usage(long, global)]
    objcopy: Option<OsString>,
    #[usage(subcommand)]
    command: Action,
}

#[derive(Subcommands)]
enum Action {
    /// Build AppMain.bin and assemble the SD card application directory.
    Build { package: String },
    /// Copy to PATH/APP/<app-name>, preserving other applications.
    Deploy {
        package: String,
        path: PathBuf,
        /// Build before deploying.
        #[usage(long)]
        build: bool,
    },
}

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
    workspace_root: PathBuf,
    target_directory: PathBuf,
}
#[derive(Deserialize)]
struct Package {
    name: String,
    manifest_path: PathBuf,
    metadata: Option<PackageMetadata>,
}
impl Package {
    fn app(&self) -> Option<&App> {
        self.metadata.as_ref()?.reflex.as_ref()
    }
}
#[derive(Deserialize)]
struct PackageMetadata {
    reflex: Option<App>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct App {
    name: String,
    output: String,
    linker: Option<PathBuf>,
    #[serde(default)]
    assets: Vec<PathBuf>,
    #[serde(default)]
    link_args: Vec<String>,
    inputs: Option<Inputs>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inputs {
    directory: String,
    env: String,
    #[serde(default)]
    assets: Vec<PathBuf>,
}

fn run(command: &mut Command) -> Result<()> {
    eprintln!("{command:?}");
    ensure!(
        command
            .status()
            .with_context(|| format!("start {command:?}"))?
            .success(),
        "command failed: {command:?}"
    );
    Ok(())
}
fn cargo() -> Command {
    Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
}
fn component(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty() && !value.contains(['/', '\\', ':']) && value != "." && value != "..",
        "expected a directory name: {value:?}"
    );
    Ok(())
}
fn relative(path: &Path) -> Result<()> {
    ensure!(
        path.components().all(|c| matches!(c, Component::Normal(_))),
        "expected a relative asset path: {}",
        path.display()
    );
    Ok(())
}

struct Build<'a> {
    package: &'a Package,
    app: &'a App,
    target: PathBuf,
    objcopy: OsString,
}
impl Build<'_> {
    fn output(&self) -> PathBuf {
        self.target
            .join(&self.app.output)
            .join("APP")
            .join(&self.app.name)
    }
    fn root(&self) -> &Path {
        self.package.manifest_path.parent().unwrap()
    }
    fn command(&self, subcommand: &str) -> Command {
        let mut cmd = cargo();
        cmd.current_dir(self.root())
            .arg(subcommand)
            .arg("--manifest-path")
            .arg(&self.package.manifest_path)
            .arg("--package")
            .arg(&self.package.name)
            .arg("--target-dir")
            .arg(&self.target);
        if let Some(inputs) = &self.app.inputs {
            cmd.env(&inputs.env, self.target.join(&inputs.directory));
        }
        cmd
    }
    fn build(&self) -> Result<PathBuf> {
        let linker = match &self.app.linker {
            Some(path) => self.root().join(path),
            None => {
                let directory = self.target.join(&self.app.output);
                fs::create_dir_all(&directory)?;
                let path = directory.join("link.x");
                fs::write(&path, include_bytes!("../link.x"))?;
                path
            }
        };
        // Cargo does not track changes to files passed through link-arg.
        let mut hash = DefaultHasher::new();
        fs::read(&linker)
            .with_context(|| format!("read linker script: {}", linker.display()))?
            .hash(&mut hash);
        let mut cmd = self.command("rustc");
        cmd.arg("--bin").arg(&self.package.name).args([
            "--features",
            "device",
            "--release",
            "--target",
            "armv7a-none-eabi",
            "--",
            "-C",
            "panic=abort",
            "-C",
            "target-cpu=cortex-a7",
            "-C",
            "relocation-model=static",
        ]);
        cmd.arg("-C")
            .arg(format!("link-arg=-T{}", linker.display()));
        cmd.arg("-C")
            .arg(format!("metadata={:016x}", hash.finish()));
        for arg in &self.app.link_args {
            cmd.arg("-C").arg(format!("link-arg={arg}"));
        }
        run(&mut cmd)?;
        let output = self.output();
        fs::create_dir_all(&output)?;
        run(Command::new(&self.objcopy)
            .arg("-O")
            .arg("binary")
            .arg(
                self.target
                    .join("armv7a-none-eabi/release")
                    .join(&self.package.name),
            )
            .arg(output.join("AppMain.bin")))?;
        fs::write(output.join("index.din"), [])?;
        for asset in &self.app.assets {
            fs::copy(
                self.root().join(asset),
                output.join(asset.file_name().context("asset lacks file name")?),
            )?;
        }
        if let Some(inputs) = &self.app.inputs {
            for asset in &inputs.assets {
                relative(asset)?;
                fs::copy(
                    self.target.join(&inputs.directory).join(asset),
                    output.join(asset.file_name().context("asset lacks file name")?),
                )?;
            }
        }
        println!("{}", output.display());
        Ok(output)
    }
}

fn windows_path(path: &Path) -> bool {
    let s = path.as_os_str().to_string_lossy();
    let b = s.as_bytes();
    (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'/' | b'\\'))
        || s.starts_with("\\\\")
}
fn wsl() -> bool {
    cfg!(target_os = "linux")
        && (env::var_os("WSL_DISTRO_NAME").is_some()
            || fs::read_to_string("/proc/sys/kernel/osrelease")
                .is_ok_and(|s| s.to_ascii_lowercase().contains("microsoft")))
}
fn ps_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
fn deploy(source: &Path, destination: &Path, name: &str) -> Result<()> {
    if !cfg!(windows) && windows_path(destination) {
        ensure!(wsl(), "Windows destinations require WSL or native Windows");
        let converted = Command::new("wslpath")
            .arg("-w")
            .arg(source.canonicalize()?)
            .output()
            .context("run wslpath -w")?;
        ensure!(
            converted.status.success(),
            "wslpath failed: {}",
            String::from_utf8_lossy(&converted.stderr)
        );
        let source = String::from_utf8(converted.stdout)?;
        let source = source.trim_end_matches(['\r', '\n']);
        ensure!(!source.is_empty(), "wslpath returned an empty path");
        let destination = destination.to_str().context("Windows path must be UTF-8")?;
        let script = format!(
            r"
            $ErrorActionPreference = 'Stop';
            $src = {};
            $root = {};
            if (!(Test-Path -LiteralPath $root -PathType Container)) {{
                throw 'Destination directory does not exist'
            }}
            $dest = Join-Path (Join-Path $root 'APP') {};
            [System.IO.Directory]::CreateDirectory($dest) | Out-Null;
            Get-ChildItem -LiteralPath $src -Force | ForEach-Object {{
                Copy-Item -LiteralPath $_.FullName -Destination $dest -Recurse -Force
            }}
            ",
            ps_string(source),
            ps_string(destination),
            ps_string(name)
        );
        run(Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command"])
            .arg(script))?;
    } else {
        ensure!(
            destination.is_dir(),
            "destination directory does not exist: {}",
            destination.display()
        );
        copy_directory(source, &destination.join("APP").join(name))?;
    }
    println!("Deployed {} to {}", name, destination.display());
    Ok(())
}
fn copy_directory(source: &Path, destination: &Path) -> Result<()> {
    ensure!(
        !destination.is_symlink(),
        "refusing symlink destination: {}",
        destination.display()
    );
    fs::create_dir_all(destination)?;
    ensure!(
        !destination
            .canonicalize()?
            .starts_with(source.canonicalize()?),
        "deployment destination must not be the source or inside it"
    );
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let output = destination.join(entry.file_name());
        ensure!(
            !output.is_symlink(),
            "refusing symlink destination: {}",
            output.display()
        );
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_directory(&entry.path(), &output)?;
        } else if ty.is_file() {
            fs::copy(entry.path(), output)?;
        } else {
            bail!("unsupported deployment file: {}", entry.path().display());
        }
    }
    Ok(())
}
fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut cmd = cargo();
    cmd.args(["metadata", "--no-deps", "--format-version", "1"]);
    if let Some(path) = &cli.manifest_path {
        cmd.arg("--manifest-path").arg(path);
    }
    let result = cmd.output().context("run cargo metadata")?;
    ensure!(
        result.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let metadata: Metadata = serde_json::from_slice(&result.stdout)?;
    let name = match &cli.command {
        Action::Build { package } | Action::Deploy { package, .. } => package,
    };
    let package = metadata
        .packages
        .iter()
        .find(|p| p.name == *name || p.app().is_some_and(|a| a.output == *name))
        .context("unknown Reflex package (use its Cargo package name or output name)")?;
    let app = package
        .app()
        .context("package lacks [package.metadata.reflex]")?;
    component(&app.name)?;
    component(&app.output)?;
    if let Some(inputs) = &app.inputs {
        component(&inputs.directory)?;
    }
    let target = cli
        .target_dir
        .or_else(|| env::var_os("BUILD_DIR").map(PathBuf::from))
        .unwrap_or(metadata.target_directory);
    let target = if target.is_absolute() {
        target
    } else {
        metadata.workspace_root.join(target)
    };
    let build = Build {
        package,
        app,
        target,
        objcopy: cli
            .objcopy
            .or_else(|| env::var_os("OBJCOPY"))
            .unwrap_or_else(|| "llvm-objcopy-20".into()),
    };
    match cli.command {
        Action::Build { .. } => {
            build.build()?;
        }
        Action::Deploy {
            path,
            build: rebuild,
            ..
        } => {
            if !cfg!(windows) && windows_path(&path) {
                ensure!(wsl(), "Windows destinations require WSL or native Windows");
            } else {
                ensure!(
                    path.is_dir(),
                    "destination directory does not exist: {}",
                    path.display()
                );
            }
            let source = if rebuild {
                build.build()?
            } else {
                build.output()
            };
            ensure!(
                source.join("AppMain.bin").is_file() && source.join("index.din").is_file(),
                "application output missing in {}; use deploy --build to build first",
                source.display()
            );
            deploy(&source, &path, &app.name)?;
        }
    }
    Ok(())
}
