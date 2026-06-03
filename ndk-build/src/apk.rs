use crate::error::NdkError;
use crate::manifest::AndroidManifest;
use crate::ndk::{Key, Ndk};
use crate::target::Target;
use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::{fs, io};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;
use zip::{CompressionMethod, ZipArchive, ZipWriter};
use zip::write::FileOptions;

/// The options for how to treat debug symbols that are present in any `.so`
/// files that are added to the APK.
///
/// Using [`strip`](https://doc.rust-lang.org/cargo/reference/profiles.html#strip)
/// or [`split-debuginfo`](https://doc.rust-lang.org/cargo/reference/profiles.html#split-debuginfo)
/// in your cargo manifest(s) may cause debug symbols to not be present in a
/// `.so`, which would cause these options to do nothing.
#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StripConfig {
    /// Does not treat debug symbols specially
    Default,
    /// Removes debug symbols from the library before copying it into the APK
    Strip,
    /// Splits the library into into an ELF (`.so`) and DWARF (`.dwarf`). Only the
    /// `.so` is copied into the APK
    Split,
}

impl Default for StripConfig {
    fn default() -> Self {
        Self::Default
    }
}

pub struct ApkConfig {
    pub ndk: Ndk,
    pub build_dir: PathBuf,
    pub apk_name: String,
    pub assets: Option<PathBuf>,
    pub resources: Option<PathBuf>,
    pub java_resources: Option<PathBuf>,
    pub manifest: AndroidManifest,
    pub disable_aapt_compression: bool,
    pub strip: StripConfig,
    pub reverse_port_forward: HashMap<String, String>,
}

impl ApkConfig {
    fn build_tool(&self, tool: &'static str) -> Result<Command, NdkError> {
        let mut cmd = self.ndk.build_tool(tool)?;
        cmd.current_dir(&self.build_dir);
        Ok(cmd)
    }

    fn linked_res(&self) -> PathBuf {
        self.build_dir
            .join(format!("{}_res.zip", self.apk_name))
    }

    fn unaligned_zip(&self) -> PathBuf {
        self.build_dir
            .join(format!("{}_unaligned.zip", self.apk_name))
    }

    fn aligned_zip(&self) -> PathBuf {
        self.build_dir
            .join(format!("{}_aligned.zip", self.apk_name))
    }

    /// Retrieves the path of the APK that will be written when [`UnsignedAab::sign`]
    /// is invoked
    #[inline]
    pub fn apk(&self) -> PathBuf {
        self.build_dir.join(format!("{}.apk", self.apk_name))
    }

    #[inline]
    pub fn aab(&self) -> PathBuf {
        self.build_dir.join(format!("{}.aab", self.apk_name))
    }

    #[inline]
    pub fn apks_zip(&self) -> PathBuf {
        self.build_dir.join(format!("{}.apks", self.apk_name))
    }

    #[inline]
    pub fn apks_connected_zip(&self) -> PathBuf {
        self.build_dir.join(format!("{}_connected.apks", self.apk_name))
    }

    pub fn create_resources(&self) -> Result<LinkedResources<'_>, NdkError> {
        std::fs::create_dir_all(&self.build_dir)?;
        self.manifest.write_to(&self.build_dir)?;

        let target_sdk_version = self
            .manifest
            .sdk
            .target_sdk_version
            .unwrap_or_else(|| self.ndk.default_target_platform());

        let mut resources = None;
        if let Some(res) = &self.resources && res.exists() {
            resources = Option::Some(self.build_dir.join("compiled_resources.zip"));
            let mut aapt = self.build_tool(bin!("aapt2"))?;
            aapt.arg("compile")
                .arg("--dir")
                .arg(format!("{}", res.display()))
                .arg("-o")
                .arg(format!("{}", resources.as_ref().unwrap().display()));
            if !aapt.status()?.success() {
                return Err(NdkError::CmdFailed(Box::new(aapt)));
            }
        }
        let mut aapt = self.build_tool(bin!("aapt2"))?;
        aapt.arg("link")
            .arg("--proto-format")
            .arg("-o")
            .arg(format!("{}", self.linked_res().display()))
            .arg("-I")
            .arg(self.ndk.android_jar(target_sdk_version)?)
            .arg("--manifest")
            .arg("AndroidManifest.xml");

        if self.disable_aapt_compression {
            aapt.arg("-0").arg("");
        }

        if let Some(assets) = &self.assets {
            aapt.arg("-A").arg(assets);
        }

        if let Some(res) = resources {
            aapt.arg(format!("{}", res.display()));
        }

        if !aapt.status()?.success() {
            return Err(NdkError::CmdFailed(Box::new(aapt)));
        }

        Ok(LinkedResources {
            config: self,
            pending_libs: HashSet::default(),
        })
    }
}

pub struct LinkedResources<'a> {
    config: &'a ApkConfig,
    pending_libs: HashSet<String>,
}

impl<'a> LinkedResources<'a> {
    pub fn config(&self) -> &ApkConfig {
        self.config
    }

    pub fn add_lib(&mut self, path: &Path, target: Target) -> Result<(), NdkError> {
        if !path.exists() {
            return Err(NdkError::PathNotFound(path.into()));
        }
        let abi = target.android_abi();
        let lib_path = Path::new("lib").join(abi).join(path.file_name().unwrap());
        let out = self.config.build_dir.join(&lib_path);
        std::fs::create_dir_all(out.parent().unwrap())?;

        match self.config.strip {
            StripConfig::Default => {
                std::fs::copy(path, out)?;
            }
            StripConfig::Strip | StripConfig::Split => {
                let obj_copy = self.config.ndk.toolchain_bin("objcopy", target)?;

                {
                    let mut cmd = Command::new(&obj_copy);
                    cmd.arg("--strip-debug");
                    cmd.arg(path);
                    cmd.arg(&out);

                    if !cmd.status()?.success() {
                        return Err(NdkError::CmdFailed(Box::new(cmd)));
                    }
                }

                if self.config.strip == StripConfig::Split {
                    let dwarf_path = out.with_extension("dwarf");

                    {
                        let mut cmd = Command::new(&obj_copy);
                        cmd.arg("--only-keep-debug");
                        cmd.arg(path);
                        cmd.arg(&dwarf_path);

                        if !cmd.status()?.success() {
                            return Err(NdkError::CmdFailed(Box::new(cmd)));
                        }
                    }

                    let mut cmd = Command::new(obj_copy);
                    cmd.arg(format!("--add-gnu-debuglink={}", dwarf_path.display()));
                    cmd.arg(out);

                    if !cmd.status()?.success() {
                        return Err(NdkError::CmdFailed(Box::new(cmd)));
                    }
                }
            }
        }

        // Pass UNIX path separators to `aapt` on non-UNIX systems, ensuring the resulting separator
        // is compatible with the target device instead of the host platform.
        // Otherwise, it results in a runtime error when loading the NativeActivity `.so` library.
        let lib_path_unix = lib_path.to_str().unwrap().replace('\\', "/");

        self.pending_libs.insert(lib_path_unix);

        Ok(())
    }

    pub fn add_runtime_libs(
        &mut self,
        path: &Path,
        target: Target,
        search_paths: &[&Path],
    ) -> Result<(), NdkError> {
        let abi_dir = path.join(target.android_abi());
        for entry in fs::read_dir(&abi_dir).map_err(|e| NdkError::IoPathError(abi_dir, e))? {
            let entry = entry?;
            let path = entry.path();
            if path.extension() == Some(OsStr::new("so")) {
                self.add_lib_recursively(&path, target, search_paths)?;
            }
        }
        Ok(())
    }

    pub fn add_pending_libs_and_align(self) -> Result<UnsignedAab<'a>, NdkError> {
        let workspace = self.config.build_dir.join("extracted_res_workspace");
        let base = self.config.unaligned_zip();
        if !workspace.exists() {
            std::fs::create_dir_all(&workspace)?;
        }
        {
            let zip_reader = BufReader::new(File::open(self.config.linked_res())?);
            let mut archive = ZipArchive::new(zip_reader)?;
            archive.extract(&workspace)?;
        }

        let res_dir = workspace.join("res");
        let dex_dir = workspace.join("dex");
        let lib_dir = workspace.join("lib");
        let manifest_dir = workspace.join("manifest");
        let original_lib_dir = self.config.build_dir.join("lib");
        if !res_dir.exists() {
            std::fs::create_dir_all(&res_dir)?;
        }
        if !dex_dir.exists() {
            std::fs::create_dir_all(&dex_dir)?;
        }

        if let Some(resources) = &self.config.java_resources {
            let mut files = Vec::new();
            if resources.exists() {
                eprintln!("No resources found in {}", resources.display());
            }
            for entry in WalkDir::new(resources).into_iter().filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_file() {
                    let extension = path.extension();
                    if let Some(extension) = extension {
                        let extension = extension.to_ascii_lowercase();
                        let str = extension.to_string_lossy().to_string();
                        match str.as_str() {
                            "dex" | "class" | "zip" | "jar" | "apk" => {
                                files.push(path.to_owned());
                            }
                            _ => {
                                eprintln!("Unsupported file type: {}", path.display());
                            }
                        }
                    }
                }
            }
            if files.is_empty() {
                eprintln!("No resources found in {}", resources.display());
            }
            let mut d8 = self.config.build_tool(bin!("d8"))?;
            d8
                .arg("--output")
                .arg(format!("{}", dex_dir.display()))
                .arg("--android-platform-build")
                .arg("--min-api")
                .arg(format!("{}", self.config.manifest.sdk.min_sdk_version.unwrap_or(self.config.manifest.sdk.target_sdk_version.unwrap_or(self.config.ndk.default_target_platform()))));
            if self.config.manifest.application.debuggable.unwrap_or(false) {
                d8.arg("--debug");
            } else {
                d8.arg("--release");
            }
            for file in files {
                d8.arg(file);
            }
            if !d8.status()?.success() {
                return Err(NdkError::CmdFailed(Box::new(d8)));
            }
        }

        if lib_dir.exists() {
            if lib_dir.is_dir() {
                std::fs::remove_dir_all(&lib_dir)?;
            } else {
                std::fs::remove_file(&lib_dir)?;
            }
        }
        for entry in WalkDir::new(&original_lib_dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            let name = path.strip_prefix(Path::new(&original_lib_dir)).unwrap();
            let new_path = lib_dir.join(name);
            if path.is_dir() {
                std::fs::create_dir_all(&new_path)?;
            } else {
                std::fs::copy(path, new_path)?;
            }
        }

        let manifest = manifest_dir.join("AndroidManifest.xml");
        let old_manifest = workspace.join("AndroidManifest.xml");

        if !old_manifest.exists() {
            return Err(NdkError::PathNotFound(old_manifest));
        }
        if !manifest_dir.exists() {
            std::fs::create_dir_all(&manifest_dir)?;
        }
        std::fs::copy(&old_manifest, manifest)?;
        std::fs::remove_file(old_manifest)?;

        let deflated_options = FileOptions::DEFAULT
            .compression_method(CompressionMethod::Deflated)
            .compression_level(Some(9));
        let stored_options = FileOptions::DEFAULT
            .compression_method(CompressionMethod::Stored);

        let mut zip_file = ZipWriter::new(File::create(base)?);

        for entry in WalkDir::new(&workspace).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();

            // Strip the "workspace" prefix to get the relative internal zip path
            let name = path.strip_prefix(Path::new(&workspace)).unwrap();

            // Skip the root workspace directory entry itself
            if name.as_os_str().is_empty() {
                continue;
            }

            // Convert path separators to forward slashes format (required by ZIP spec)
            let name_str = name.to_string_lossy().replace('\\', "/");

            if path.is_dir() {
                // ZIP directories must end with a trailing forward slash
                zip_file.add_directory(format!("{}/", name_str), deflated_options)?;
            } else if path.is_file() {
                if name_str.ends_with(".DS_Store") {
                    continue;
                }
                // Check if the file lives inside the 'lib/' folder
                let options = if name_str.starts_with("lib/") {
                    stored_options
                } else {
                    deflated_options
                };

                // Begin writing the file entry
                zip_file.start_file(name_str, options)?;

                // Stream the file contents into the zip archive
                let mut f = File::open(path)?;
                io::copy(&mut f, &mut zip_file)?;
            }
        }

        zip_file.finish()?;

        let mut zipalign = self.config.build_tool(bin!("zipalign"))?;
        zipalign
            .arg("-f")
            .arg("-v")
            .args(["-P", "16"])
            .arg("4")
            .arg(self.config.unaligned_zip())
            .arg(self.config.aligned_zip());

        if !zipalign.status()?.success() {
            return Err(NdkError::CmdFailed(Box::new(zipalign)));
        }

        let mut bundle_tools = self.config.ndk.bundle_tool()?;

        bundle_tools
            .arg("build-bundle")
            .arg(format!("--modules={}", self.config.aligned_zip().display()))
            .arg(format!("--output={}", self.config.aab().display()))
            .arg("--overwrite");
        if !bundle_tools.status()?.success() {
            return Err(NdkError::CmdFailed(Box::new(bundle_tools)));
        }

        Ok(UnsignedAab(self.config))
    }
}

pub struct UnsignedAab<'a>(&'a ApkConfig);

pub struct SignedAab {
    path: PathBuf,
    apks: PathBuf,
    apks_connected: PathBuf,
    package_name: String,
    ndk: Ndk,
    reverse_port_forward: HashMap<String, String>,
    key: Key
}

impl SignedAab {
    pub fn from_config(config: &ApkConfig, key: Key) -> Self {
        let ndk = config.ndk.clone();
        Self {
            path: config.aab(),
            apks: config.apks_zip(),
            apks_connected: config.apks_connected_zip(),
            package_name: config.manifest.package.clone(),
            ndk,
            reverse_port_forward: config.reverse_port_forward.clone(),
            key,
        }
    }

    pub fn reverse_port_forwarding(&self, device_serial: Option<&str>) -> Result<(), NdkError> {
        for (from, to) in &self.reverse_port_forward {
            println!("Reverse port forwarding from {from} to {to}");
            let mut adb = self.ndk.adb(device_serial)?;

            adb.arg("reverse").arg(from).arg(to);

            if !adb.status()?.success() {
                return Err(NdkError::CmdFailed(Box::new(adb)));
            }
        }

        Ok(())
    }

    pub fn build_apks(&self) -> Result<(), NdkError> {
        let aapt2_bin = self.ndk.aapt2_path()?;
        let mut bundle_tool = self.ndk.bundle_tool()?;
        bundle_tool
            .arg("build-apks")
            .arg(format!("--bundle={}", self.path.display()))
            .arg(format!("--output={}", self.apks.display()))
            .arg("--overwrite")
            .arg(format!("--ks={}", self.key.path.display()))
            .arg(format!("--ks-key-alias={}", self.key.alias))
            .arg(format!("--ks-pass=pass:{}", self.key.password))
            .arg(format!("--aapt2={}", aapt2_bin.display()))
            .arg("--enable-sparse-encoding");
        if !bundle_tool.status()?.success() {
            return Err(NdkError::CmdFailed(Box::new(bundle_tool)));
        }
        Ok(())
    }

    pub fn install(&self, device_serial: Option<&str>) -> Result<(), NdkError> {
        let adb_bin = self.ndk.adb_path()?;
        let aapt2_bin = self.ndk.aapt2_path()?;
        let mut bundle_tool = self.ndk.bundle_tool()?;
        bundle_tool
            .arg("build-apks")
            .arg(format!("--bundle={}", self.path.display()))
            .arg(format!("--output={}", self.apks_connected.display()))
            .arg("--overwrite")
            .arg(format!("--ks={}", self.key.path.display()))
            .arg(format!("--ks-key-alias={}", self.key.alias))
            .arg(format!("--ks-pass=pass:{}", self.key.password))
            .arg(format!("--aapt2={}", aapt2_bin.display()))
            .arg(format!("--adb={}", adb_bin.display()))
            .arg("--enable-sparse-encoding")
            .arg("--local-testing");
        if let Some(device_serial) = device_serial {
            bundle_tool.arg(format!("--device-id={}", device_serial));
        } else {
            bundle_tool.arg("--connected-device");
        }

        if !bundle_tool.status()?.success() {
            return Err(NdkError::CmdFailed(Box::new(bundle_tool)));
        }

        let mut bundle_tool = self.ndk.bundle_tool()?;
        bundle_tool
            .arg("install-apks")
            .arg(format!("--apks={}", self.apks_connected.display()))
            .arg(format!("--adb={}", adb_bin.display()))
            .arg("--allow-test-only")
            .arg("--grant-runtime-permissions")
            .arg("");

        if let Some(device_serial) = device_serial {
            bundle_tool.arg(format!("--device-id={}", device_serial));
        }

        if !bundle_tool.status()?.success() {
            return Err(NdkError::CmdFailed(Box::new(bundle_tool)));
        }

        Ok(())
    }

    pub fn start(&self, device_serial: Option<&str>) -> Result<(), NdkError> {
        let mut adb = self.ndk.adb(device_serial)?;
        adb.arg("shell")
            .arg("am")
            .arg("start")
            .arg("-a")
            .arg("android.intent.action.MAIN")
            .arg("-n")
            .arg(format!("{}/android.app.NativeActivity", self.package_name));

        if !adb.status()?.success() {
            return Err(NdkError::CmdFailed(Box::new(adb)));
        }

        Ok(())
    }

    pub fn uidof(&self, device_serial: Option<&str>) -> Result<u32, NdkError> {
        let mut adb = self.ndk.adb(device_serial)?;
        adb.arg("shell")
            .arg("pm")
            .arg("list")
            .arg("package")
            .arg("-U")
            .arg(&self.package_name);
        let output = adb.output()?;

        if !output.status.success() {
            return Err(NdkError::CmdFailed(Box::new(adb)));
        }

        let output = std::str::from_utf8(&output.stdout).unwrap();
        let (_package, uid) = output
            .lines()
            .filter_map(|line| line.split_once(' '))
            // `pm list package` uses the id as a substring filter; make sure
            // we select the right package in case it returns multiple matches:
            .find(|(package, _uid)| package.strip_prefix("package:") == Some(&self.package_name))
            .ok_or(NdkError::PackageNotInOutput {
                package: self.package_name.clone(),
                output: output.to_owned(),
            })?;
        let uid = uid
            .strip_prefix("uid:")
            .ok_or(NdkError::UidNotInOutput(output.to_owned()))?;
        uid.parse()
            .map_err(|e| NdkError::NotAUid(e, uid.to_owned()))
    }

    /*pub fn create_apks(&self) -> Result<Vec<UnsignedApk<'a>>, NdkError> {
        let mut bundle_tool = self.0.ndk.bundle_tool()?;
        // build-apks --bundle=app-release.aab --output=app.apks
        bundle_tool
            .arg("build-apks")
            .arg(format!("--bundle={}", self.0.aab().display()))
            .arg(format!("--output={}", self.0.apks_zip().display()));
        if !bundle_tool.status()?.success() {
            return Err(NdkError::CmdFailed(Box::new(bundle_tool)));
        }
        // Apk::from_config(self.0)
        Ok(vec![])
    }*/
}

impl<'a> UnsignedAab<'a> {
    pub fn sign(self, key: Key) -> Result<SignedAab, NdkError> {
        let mut jarsigner = self.0.ndk.jarsigner()?;
        jarsigner
            .arg("-keystore")
            .arg(&key.path)
            .arg("-storepass")
            .arg(format!("{}", &key.password))
            .arg(self.0.aab())
            .arg(&key.alias);
        if !jarsigner.status()?.success() {
            return Err(NdkError::CmdFailed(Box::new(jarsigner)));
        }
        Ok(SignedAab::from_config(self.0, key))
    }

    pub fn build_apks(&self) -> Result<(), NdkError> {
        let aapt2_bin = self.0.ndk.aapt2_path()?;
        let mut bundle_tool = self.0.ndk.bundle_tool()?;
        bundle_tool
            .arg("build-apks")
            .arg(format!("--bundle={}", self.0.aab().display()))
            .arg(format!("--output={}", self.0.apks_zip().display()))
            .arg("--overwrite")
            .arg(format!("--aapt2={}", aapt2_bin.display()))
            .arg("--enable-sparse-encoding");
        if !bundle_tool.status()?.success() {
            return Err(NdkError::CmdFailed(Box::new(bundle_tool)));
        }
        Ok(())
    }
}
