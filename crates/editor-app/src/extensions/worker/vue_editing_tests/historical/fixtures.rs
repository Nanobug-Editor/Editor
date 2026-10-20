//! Three temporary configured projects reuse only the explicitly selected locked dependency trees.
use super::*;
use serde_json::json;
use std::{fs, process::Command};

/// A row names a fixed, already declared dependency combination; no Vue3 class support is invented.
#[derive(Clone, Copy)]
pub(super) struct Sample {
    pub version: &'static str,
    pub kind: &'static str,
    lang: &'static str,
}

/// These eight rows share three configured graphs while retaining distinct unopened parent targets.
pub(super) const SAMPLES: [Sample; 8] = [
    Sample {
        version: "vue26",
        kind: "class",
        lang: "ts",
    },
    Sample {
        version: "vue27",
        kind: "class",
        lang: "ts",
    },
    Sample {
        version: "vue26",
        kind: "jsx",
        lang: "jsx",
    },
    Sample {
        version: "vue26",
        kind: "tsx",
        lang: "tsx",
    },
    Sample {
        version: "vue27",
        kind: "jsx",
        lang: "jsx",
    },
    Sample {
        version: "vue27",
        kind: "tsx",
        lang: "tsx",
    },
    Sample {
        version: "vue3",
        kind: "jsx",
        lang: "jsx",
    },
    Sample {
        version: "vue3",
        kind: "tsx",
        lang: "tsx",
    },
];

/// Originals are immutable observations, not writable document state or a replacement Undo stack.
pub(super) struct Projects {
    directory: tempfile::TempDir,
    originals: Vec<(PathBuf, Vec<u8>)>,
    fixed_projects: Vec<PathBuf>,
}

impl Projects {
    /// Keep every dependency junction within the selected workspace authority, and isolate generated sources.
    pub fn prepare() -> Self {
        let workspace = super::super::super::vue_test_support::input("NANOBUG_VUE_FIXTURES")
            .canonicalize()
            .unwrap();
        let target = workspace.join("target");
        fs::create_dir_all(&target).unwrap();
        let directory = tempfile::Builder::new()
            .prefix("s6-native-historical-")
            .tempdir_in(target)
            .unwrap();
        let mut originals = Vec::new();
        let mut fixed_projects = Vec::new();
        for version in ["vue26", "vue27", "vue3"] {
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
                "fixture dependency must stay within the selected workspace"
            );
            junction(&root.join("node_modules"), &dependency);
            let mut compiler = json!({"target":"ES2020","module":"ESNext","moduleResolution":"Node","strict":true,
                "skipLibCheck":true,"allowJs":true,"checkJs":true,"experimentalDecorators":true,
                "useDefineForClassFields":false,"jsx":"preserve"});
            if version == "vue3" {
                compiler["jsxImportSource"] = json!("vue");
            }
            if version == "vue26" {
                compiler["jsxFactory"] = json!("VueTsxSupport");
            }
            let target = match version {
                "vue26" => 2.6,
                "vue27" => 2.7,
                _ => 3.5,
            };
            fs::write(root.join("tsconfig.json"), serde_json::to_vec(&json!({"compilerOptions":compiler,
                "include":["**/*.vue","**/*.ts"],"vueCompilerOptions":{"target":target,"strictTemplates":true}})).unwrap()).unwrap();
            fs::write(root.join("helpers.ts"), "export function formatHistoricalTitle(value: string): string { return value.toUpperCase(); }\r\n").unwrap();
            for name in ["package.json", "tsconfig.json", "helpers.ts"] {
                let path = root.join(name);
                originals.push((path.clone(), fs::read(path).unwrap()));
            }
            fixed_projects.push(fixed);
        }
        let mut projects = Self {
            directory,
            originals,
            fixed_projects,
        };
        for sample in SAMPLES {
            let root = projects.directory(sample);
            fs::create_dir_all(&root).unwrap();
            fs::write(root.join("Child.vue"), child(sample)).unwrap();
            fs::write(root.join("App.vue"), parent(sample)).unwrap();
            for name in ["Child.vue", "App.vue"] {
                let path = root.join(name);
                projects
                    .originals
                    .push((path.clone(), fs::read(path).unwrap()));
            }
        }
        projects
    }

    /// Each row has its own closed parent; all forms of one Vue version share the same configured graph.
    pub fn directory(&self, sample: Sample) -> PathBuf {
        self.directory.path().join(sample.version).join(sample.kind)
    }

    /// Persistent fixtures and their dependency helper directories must survive acceptance byte-for-byte.
    pub fn assert_unchanged(&self) {
        for (path, bytes) in &self.originals {
            assert_eq!(
                fs::read(path).unwrap(),
                *bytes,
                "historical acceptance changed {}",
                path.display()
            );
        }
        for project in &self.fixed_projects {
            assert!(
                !project.join("node_modules/.vue-global-types").exists(),
                "historical analysis generated helpers in the fixed dependency fixture"
            );
        }
    }
}

/// Match the semantic forms in the real tsserver suite, including the locked Vue2 JSX declaration.
fn child(sample: Sample) -> String {
    let body = if sample.kind == "class" {
        "import {Vue,Component,Prop} from 'vue-property-decorator';\n@Component\nexport default class Child extends Vue { @Prop({type:String,required:true}) readonly title!: string; formatTitle(){return this.title.toUpperCase();} }".to_owned()
    } else if sample.version == "vue3" {
        "import {defineComponent} from 'vue';\nexport default defineComponent({props:{title:{type:String,required:true}},setup(props){return ()=> <span>{props.title}</span>}});".to_owned()
    } else {
        format!(
            "{}import Vue from 'vue';\nexport default Vue.extend({{props:{{title:{{type:String,required:true}}}},render(h){{return <span>{{this.title}}</span>}}}});",
            if sample.version == "vue26" {
                "import 'vue-tsx-support/enable-check';\n"
            } else {
                ""
            }
        )
    };
    format!("<script lang=\"{}\">\n{body}\nconst completionSample = null;\n</script>\n<template><p>{{{{ title }}}}</p></template>\n", sample.lang).replace('\n', "\r\n")
}

/// A plain imported component use ensures the historical property's proposal includes another real file.
fn parent(sample: Sample) -> String {
    let body = if sample.version == "vue3" {
        "import {defineComponent} from 'vue';\nimport Child from './Child.vue';\nexport default defineComponent({components:{Child}});"
    } else {
        "import Vue from 'vue';\nimport Child from './Child.vue';\nexport default Vue.extend({components:{Child}});"
    };
    format!(
        "<script lang=\"ts\">\n{body}\n</script>\n<template><Child title=\"hello\" /></template>\n"
    )
    .replace('\n', "\r\n")
}

/// Non-interactive Windows junction creation passes literal paths through a child-only environment.
fn junction(link: &Path, target: &Path) {
    use std::os::windows::process::CommandExt as _;
    let ordinary = |path: &Path| {
        path.to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_owned()
    };
    let output = Command::new("powershell").creation_flags(0x0800_0000)
        .args(["-NoProfile", "-NonInteractive", "-Command",
            "New-Item -ItemType Junction -Path $env:NANOBUG_S6_HISTORY_LINK -Value $env:NANOBUG_S6_HISTORY_TARGET -ErrorAction Stop | Out-Null"])
        .env("NANOBUG_S6_HISTORY_LINK", ordinary(link)).env("NANOBUG_S6_HISTORY_TARGET", ordinary(target)).output().unwrap();
    assert!(
        output.status.success(),
        "historical dependency junction failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(link.canonicalize().unwrap(), target);
}
