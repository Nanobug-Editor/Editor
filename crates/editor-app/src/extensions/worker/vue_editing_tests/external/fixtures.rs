//! Disposable external-script projects retain locked dependencies and immutable saved-source evidence.
use super::*;
use std::fs;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::os::windows::process::CommandExt as _;
use std::process::{Child, Command, Stdio};

/// One legal version/export combination with separate saved and unsaved original source preimages.
pub(super) struct Case {
    /// Fixed framework dependency fixture, rather than a synthesized future target.
    pub version: &'static str,
    /// External block language is TS for legacy and JS for modern Vue.
    pub extension: &'static str,
    /// Canonical original SFC, with no generated projection in the expected target set.
    pub component: PathBuf,
    /// Canonical associated source whose primary provider remains TypeScript.
    pub script: PathBuf,
    /// Saved preimage used solely to detect unintended disk writes.
    component_saved: String,
    /// External saved preimage deliberately lacks the unsaved numeric field.
    script_saved: String,
    /// Immutable native SFC preimage containing Unicode before the relevant expressions.
    pub component_dirty: String,
    /// Immutable external preimage proving the association reads the dirty native session.
    pub script_dirty: String,
}

impl Case {
    /// Language proposals never silently save either participating original source.
    pub fn assert_saved(&self) {
        assert_eq!(
            fs::read_to_string(&self.component).unwrap(),
            self.component_saved
        );
        assert_eq!(fs::read_to_string(&self.script).unwrap(), self.script_saved);
    }
}

/// The directory outlives checked shutdown; fixed projects and generated metadata remain untouched.
pub(super) struct Projects {
    /// Own temporary sources and junctions until every managed process has stopped.
    _directory: tempfile::TempDir,
    /// Retain fixed fixture and project metadata bytes without creating writable editor state.
    originals: Vec<(PathBuf, Vec<u8>)>,
    /// Exactly the two documented external-source acceptance combinations.
    pub cases: Vec<Case>,
}

impl Projects {
    /// Keep both physical source roots and resolved dependency targets within the granted Vue workspace.
    pub fn prepare() -> Self {
        let workspace = super::super::super::vue_test_support::input("NANOBUG_VUE_FIXTURES")
            .canonicalize()
            .unwrap();
        let target = workspace.join("target");
        fs::create_dir_all(&target).unwrap();
        let directory = tempfile::Builder::new()
            .prefix("s6-native-external-")
            .tempdir_in(target)
            .unwrap();
        let mut originals = Vec::new();
        let mut cases = Vec::new();
        for (version, extension) in [("vue26", "ts"), ("vue3", "js")] {
            let fixed = workspace.join(format!("tests/projects/{version}"));
            for name in ["package.json", "tsconfig.json", "App.vue", "Child.vue"] {
                let path = fixed.join(name);
                originals.push((path.clone(), fs::read(path).unwrap()));
            }
            let root = directory.path().join(version);
            fs::create_dir_all(&root).unwrap();
            fs::copy(fixed.join("package.json"), root.join("package.json")).unwrap();
            let dependency = fixed.join("node_modules").canonicalize().unwrap();
            assert!(
                dependency.starts_with(&workspace),
                "fixed dependencies escaped workspace"
            );
            junction(&root.join("node_modules"), &dependency);
            let mut config: serde_json::Value =
                serde_json::from_slice(&fs::read(fixed.join("tsconfig.json")).unwrap()).unwrap();
            config["compilerOptions"]["allowJs"] = serde_json::json!(true);
            config["compilerOptions"]["checkJs"] = serde_json::json!(true);
            config["include"] = serde_json::json!(["*.vue", "*.ts", "*.js"]);
            fs::write(
                root.join("tsconfig.json"),
                serde_json::to_vec(&config).unwrap(),
            )
            .unwrap();
            for name in ["package.json", "tsconfig.json"] {
                let path = root.join(name);
                originals.push((path.clone(), fs::read(path).unwrap()));
            }
            let (import, call) = if version == "vue3" {
                ("import {defineComponent as define} from 'vue';", "define")
            } else {
                ("import Vue from 'vue';", "Vue.extend")
            };
            let script_saved = format!(
                "{import}\r\nexport default {call}({{data(){{return{{message:'saved'}}}}}});\r\n"
            );
            let script_dirty = format!(
                "{import}\r\n/* 中文😀e\u{301} */ export default {call}({{data(){{return{{message:'未保存😀',extra:1}}}}}});\r\n"
            );
            let prefix = format!(
                "<script src=\"./external.{extension}\" lang=\"{extension}\"></script>\r\n"
            );
            let component_saved =
                format!("{prefix}<template><div>{{{{ message }}}}</div></template>\r\n");
            let component_dirty = format!(
                "{prefix}<template><div>中文😀e\u{301} {{{{ message }}}} {{{{ extra.toUpperCase() }}}}</div></template>\r\n"
            );
            let component = root.join("External.vue");
            let script = root.join(format!("external.{extension}"));
            fs::write(&component, &component_saved).unwrap();
            fs::write(&script, &script_saved).unwrap();
            cases.push(Case {
                version,
                extension,
                component: component.canonicalize().unwrap(),
                script: script.canonicalize().unwrap(),
                component_saved,
                script_saved,
                component_dirty,
                script_dirty,
            });
        }
        Self {
            _directory: directory,
            originals,
            cases,
        }
    }

    /// Compare original bytes instead of inferring safety from a still-dirty tab indicator.
    pub fn assert_unchanged(&self) {
        for (path, bytes) in &self.originals {
            assert_eq!(
                fs::read(path).unwrap(),
                *bytes,
                "external acceptance wrote {}",
                path.display()
            );
        }
        for case in &self.cases {
            case.assert_saved();
        }
    }
}

/// Reap the exact fixture helper on success, timeout and panic; it never owns the editor's tools.
struct Helper(Child);
impl Drop for Helper {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// A fixed child-only script creates one junction; bounded private-file stderr cannot block a pipe.
fn junction(link: &Path, target: &Path) {
    // Only adapt the already-authorized local fixture representation for PowerShell New-Item.
    // The actual target is canonicalized and checked before calling this helper, and checked again below.
    let ordinary = |path: &Path| {
        path.to_string_lossy()
            .strip_prefix(r"\\?\")
            .map(str::to_owned)
            .unwrap_or_else(|| path.to_string_lossy().into_owned())
    };
    let program = PathBuf::from(std::env::var_os("SystemRoot").expect("Windows directory"))
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let mut errors = tempfile::tempfile().unwrap();
    let mut helper = Helper(Command::new(program).creation_flags(0x0800_0000)
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command",
            "New-Item -ItemType Junction -Path $env:NANOBUG_S6_EXTERNAL_LINK -Value $env:NANOBUG_S6_EXTERNAL_TARGET -ErrorAction Stop | Out-Null"])
        .env("NANOBUG_S6_EXTERNAL_LINK", ordinary(link))
        .env("NANOBUG_S6_EXTERNAL_TARGET", ordinary(target))
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::from(errors.try_clone().unwrap()))
        .spawn().unwrap());
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        assert!(
            errors.metadata().unwrap().len() <= 16 * 1024,
            "fixture helper stderr exceeded bound"
        );
        if let Some(status) = helper.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "external dependency junction helper timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    errors.seek(SeekFrom::Start(0)).unwrap();
    let mut message = Vec::new();
    errors
        .take(16 * 1024 + 1)
        .read_to_end(&mut message)
        .unwrap();
    assert!(
        message.len() <= 16 * 1024,
        "fixture helper stderr exceeded bound"
    );
    assert!(
        status.success(),
        "external dependency junction failed: {}",
        String::from_utf8_lossy(&message)
    );
    assert_eq!(link.canonicalize().unwrap(), target);
}
