//! Real importer and Java packages compose through the same sealed native binding as the UI.
use super::*;
use plugin_runtime::{
    DemandReason, LanguageRole,
    plugin_protocol::{api, language::DocumentContext},
};

/// A module receives its own source closure and JARs, without merging an unrelated sibling project.
#[test]
#[ignore = "Build Maven and Java with --plugin-package; set NANOBUG_MAVEN_PACKAGE and NANOBUG_JAVA_PACKAGE"]
fn maven_module_prepares_the_real_java_provider_through_public_collaboration() {
    exercise_project(Case::Binding);
}

/// The approved imported plan drives real JDT diagnostics and definitions over its native transport.
#[test]
#[ignore = "Build Maven and Java; set NANOBUG_MAVEN_PACKAGE and NANOBUG_JAVA_PACKAGE"]
fn imported_maven_sources_and_jars_support_real_java_analysis_and_definitions() {
    exercise_project(Case::Semantics);
}

/// Refreshing build metadata withdraws old native bindings even when document/configuration IDs stay equal.
#[test]
#[ignore = "Build Maven and Java; set NANOBUG_MAVEN_PACKAGE and NANOBUG_JAVA_PACKAGE"]
fn refreshing_maven_metadata_revokes_the_previous_java_context() {
    exercise_project(Case::Refresh);
}

/// An excluded compile dependency cannot reappear through another module's analysis classpath.
#[test]
#[ignore = "Build Maven and Java; set NANOBUG_MAVEN_PACKAGE and NANOBUG_JAVA_PACKAGE"]
fn maven_exclusions_remain_effective_in_the_real_java_context() {
    exercise_project(Case::Exclusion);
}

/// Renaming a library member must find dependants, even when preparation starts from the library.
#[test]
#[ignore = "Build Maven and Java; set NANOBUG_MAVEN_PACKAGE and NANOBUG_JAVA_PACKAGE"]
fn maven_library_rename_includes_unsaved_dependent_modules() {
    exercise_project(Case::Rename);
}

/// Source syntax and bytecode target must remain separate in different reactor projects.
#[test]
#[ignore = "Build Maven and Java; set NANOBUG_MAVEN_PACKAGE and NANOBUG_JAVA_PACKAGE"]
fn maven_modules_keep_distinct_source_and_target_compiler_levels() {
    exercise_project(Case::Compiler);
}

/// Each actual-package assertion shares setup, while its observable behavior stays explicit.
#[derive(Clone, Copy)]
enum Case {
    Binding,
    Semantics,
    Refresh,
    Exclusion,
    Rename,
    Compiler,
}

/// Shared actual-package setup keeps native semantics and lifecycle assertions on the same public path.
fn exercise_project(case: Case) {
    let semantics = matches!(case, Case::Semantics);
    let refresh = matches!(case, Case::Refresh);
    let exclude = matches!(case, Case::Exclusion);
    let rename = matches!(case, Case::Rename);
    let compiler = matches!(case, Case::Compiler);
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    for path in [
        "app/src/main/java",
        "lib/src/main/java",
        "other/src/main/java",
    ] {
        std::fs::create_dir_all(workspace.path().join(path)).unwrap();
    }
    std::fs::write(workspace.path().join("pom.xml"),r#"<project xmlns="http://maven.apache.org/POM/4.0.0"><modelVersion>4.0.0</modelVersion><groupId>sample</groupId><artifactId>parent</artifactId><version>1</version><packaging>pom</packaging><properties><maven.compiler.release>25</maven.compiler.release></properties><modules><module>lib</module><module>app</module><module>other</module></modules></project>"#).unwrap();
    for module in ["lib", "app", "other"] {
        let mut dependencies = if module == "app" {
            "<dependencies><dependency><groupId>sample</groupId><artifactId>lib</artifactId><version>1</version></dependency></dependencies>"
        } else {
            ""
        }.to_owned();
        if module == "app" && semantics {
            dependencies=dependencies.replace("</dependencies>","<dependency><groupId>fixture</groupId><artifactId>api</artifactId><version>1</version></dependency></dependencies>");
        }
        if exclude {
            if module == "app" {
                dependencies=dependencies.replace("<artifactId>lib</artifactId><version>1</version>","<artifactId>lib</artifactId><version>1</version><exclusions><exclusion><groupId>fixture</groupId><artifactId>api</artifactId></exclusion></exclusions>");
            }
            if module == "lib" {
                dependencies="<dependencies><dependency><groupId>fixture</groupId><artifactId>api</artifactId><version>1</version></dependency></dependencies>".into();
            }
        }
        std::fs::write(workspace.path().join(format!("{module}/pom.xml")),format!(r#"<project xmlns="http://maven.apache.org/POM/4.0.0"><modelVersion>4.0.0</modelVersion><parent><groupId>sample</groupId><artifactId>parent</artifactId><version>1</version></parent><artifactId>{module}</artifactId>{dependencies}</project>"#)).unwrap();
    }
    std::fs::write(
        workspace.path().join("app/src/main/java/Main.java"),
        "public class Main { Library library; }",
    )
    .unwrap();
    if compiler {
        let pom = workspace.path().join("lib/pom.xml");
        let content = std::fs::read_to_string(&pom).unwrap().replace("</project>","<build><plugins><plugin><artifactId>maven-compiler-plugin</artifactId><configuration><release combine.self=\"override\"/><source>8</source><target>25</target></configuration></plugin></plugins></build><properties><maven.compiler.release></maven.compiler.release></properties></project>");
        std::fs::write(pom, content).unwrap();
    }
    std::fs::write(
        workspace.path().join("lib/src/main/java/Library.java"),
        "public class Library { }",
    )
    .unwrap();
    if semantics || exclude {
        super::repository::seed(&profile.path().join("m2"));
    }
    let mut manager = Manager::open(
        profile.path().into(),
        Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager.runtime_observations().start_capture();
    for variable in ["NANOBUG_MAVEN_PACKAGE", "NANOBUG_JAVA_PACKAGE"] {
        eprintln!("Installing {variable}");
        let package =
            Package::read(std::path::Path::new(&std::env::var_os(variable).unwrap())).unwrap();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
    }
    manager
        .update_setting(
            "maven",
            Scope::Project,
            "maven.repository",
            Some(json!(profile.path().join("m2").display().to_string())),
        )
        .unwrap();
    demand_support::ready(
        &mut manager,
        "maven",
        DemandReason::Command("import".into()),
    );
    eprintln!("Maven is active; importing reactor");
    maven_command(
        &mut manager,
        "import",
        json!({"pom":"pom.xml","profiles":[],"offline":true}),
    )
    .unwrap();
    let availability = manager.availability("maven").unwrap();
    let document = DocumentContext {
        path: if rename {
            "lib/src/main/java/Library.java"
        } else {
            "app/src/main/java/Main.java"
        }
        .into(),
        generation: availability.configuration_generation(),
    };
    let prepared = manager
        .begin_demand(
            &availability,
            DemandReason::LanguageDocument {
                provider: "maven/analysis".into(),
                role: LanguageRole::Lsp,
                document: document.clone(),
            },
            120_000,
        )
        .expect("The real Maven package exposes an independent language provider");
    let deadline = Instant::now() + Duration::from_secs(120);
    let control = loop {
        manager.poll();
        if let Some((_, request)) = manager.take_editor_requests().pop() {
            assert!(request.begin());
            break request;
        }
        assert!(
            !prepared.status().is_terminal(),
            "Java collaboration ended before native preparation: {:?}",
            prepared.status()
        );
        assert!(Instant::now() < deadline, "Java collaboration deadline");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(
        control.language_control_metadata().unwrap().provider,
        "java/project-analysis"
    );
    eprintln!("Native control admitted; preparing the selected Java context");
    let java_document = DocumentContext {
        generation: manager
            .availability("java")
            .unwrap()
            .configuration_generation(),
        ..document.clone()
    };
    demand_support::ready(
        &mut manager,
        "java",
        DemandReason::LanguageDocument {
            provider: "java/project-analysis".into(),
            role: LanguageRole::Lsp,
            document: java_document.clone(),
        },
    );
    let service = manager
        .language_document_service("java/project-analysis", &java_document)
        .unwrap()
        .unwrap();
    let config = &service.provider.configuration["java"];
    let graph = &service.provider.initialization_options["nanobugProjects"];
    let app = graph["projects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["sources"] == json!(["app/src/main/java"]))
        .unwrap();
    let library = graph["projects"]
        .as_array()
        .unwrap()
        .iter()
        .position(|node| node["sources"] == json!(["lib/src/main/java"]))
        .unwrap();
    assert!(
        app["references"]
            .as_array()
            .unwrap()
            .contains(&json!(library))
    );
    assert_eq!(config["project"]["sourcePaths"], json!(["."]));
    assert_eq!(config["configuration"]["runtimes"][0]["name"], "JavaSE-25");
    assert_eq!(config["import"]["maven"]["enabled"], false);
    if semantics {
        let libraries = graph["libraries"].as_array().unwrap();
        assert!(
            libraries
                .iter()
                .any(|path| path.as_str().is_some_and(|path| path
                    .replace('\\', "/")
                    .ends_with("/fixture/api/1/api-1.jar"))),
            "The resolved Maven JAR must enter the Java project: {config}"
        );
    }
    if exclude {
        assert!(
            app["libraries"].as_array().unwrap().is_empty(),
            "A Maven exclusion cannot be undone by a reactor sibling's dependencies: {config}"
        );
    }
    // The native worker completes this exact registered service; no alternate provider is substituted.
    control.register_language_consumer(&service).unwrap();
    let mut server =
        (semantics || exclude || rename || compiler).then(|| super::lsp::Server::new(&service));
    if let Some(server) = &mut server {
        let initialized = server.initialize(&service);
        assert_eq!(
            initialized["capabilities"]["experimental"]["nanobugControl"]["version"],
            1
        );
        assert_eq!(
            server.request(
                2,
                "java/project/ready",
                json!({"arguments":null,"_nanobug":{"native":true}})
            ),
            Value::Null
        );
    }
    control.finish_language_binding(Value::Null, service.clone());
    loop {
        manager.poll();
        if let api::RequestUpdate::Completed { result } = prepared.status() {
            manager
                .admit_receipt(&availability, &result.unwrap())
                .unwrap();
            break;
        }
        assert!(Instant::now() < deadline, "Native binding reply deadline");
        std::thread::sleep(Duration::from_millis(10));
    }
    let borrowed = manager
        .language_document_service("maven/analysis", &document)
        .unwrap()
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(&borrowed, &service));
    if let Some(server) = &mut server {
        if compiler {
            assert_eq!(graph["projects"][library]["source"], "8");
            assert_eq!(graph["projects"][library]["target"], "25");
            assert_eq!(graph["projects"][library]["release"], "");
            server.analyze_compiler_levels(workspace.path());
        } else if rename {
            server.rename_library_member(workspace.path());
        } else if exclude {
            server.analyze_excluded_dependency(workspace.path());
        } else {
            server.analyze_imported_project(workspace.path());
        }
    }
    // Native control holds only a temporary internal document lease; the facade now owns the borrow.
    manager.release_language_document("java/project-analysis", &java_document);
    if refresh {
        let pom = workspace.path().join("app/pom.xml");
        let updated=std::fs::read_to_string(&pom).unwrap().replace("</project>","<build><outputDirectory>${project.basedir}/target/new-classes</outputDirectory></build></project>");
        std::fs::write(pom, updated).unwrap();
        let imported = maven_command(
            &mut manager,
            "import",
            json!({"pom":"pom.xml","profiles":[],"offline":true}),
        )
        .unwrap();
        let imported: Value = serde_json::from_str(imported["payload"].as_str().unwrap()).unwrap();
        assert_eq!(
            imported["modules"]
                .as_array()
                .unwrap()
                .iter()
                .find(|module| module["id"] == "sample:app:1")
                .unwrap()["output"],
            "app/target/new-classes"
        );
        assert!(
            !borrowed.is_active(),
            "A successful Maven refresh cannot leave its previous Java model active"
        );
        assert!(
            manager
                .language_document_service("maven/analysis", &document)
                .is_none(),
            "The original document intent must require fresh discovery after project metadata changes"
        );
        // A new discovery epoch must also be usable; revoking an old plan alone is insufficient.
        let renewed = manager
            .begin_demand(
                &availability,
                DemandReason::LanguageDocument {
                    provider: "maven/analysis".into(),
                    role: LanguageRole::Lsp,
                    document: document.clone(),
                },
                30_000,
            )
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        let native = loop {
            manager.poll();
            if let Some((_, request)) = manager.take_editor_requests().pop() {
                if !request.status().is_terminal() && request.begin() {
                    break request;
                }
            }
            assert!(
                !renewed.status().is_terminal(),
                "Fresh Maven discovery ended: {:?}",
                renewed.status()
            );
            assert!(
                Instant::now() < deadline,
                "Fresh Maven discovery never reached native control"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        demand_support::ready(
            &mut manager,
            "java",
            DemandReason::LanguageDocument {
                provider: "java/project-analysis".into(),
                role: LanguageRole::Lsp,
                document: java_document.clone(),
            },
        );
        let current = manager
            .language_document_service("java/project-analysis", &java_document)
            .unwrap()
            .unwrap();
        native.register_language_consumer(&current).unwrap();
        native.finish_language_binding(Value::Null, current.clone());
        loop {
            manager.poll();
            if let api::RequestUpdate::Completed { result } = renewed.status() {
                result.unwrap();
                break;
            }
            assert!(
                Instant::now() < deadline,
                "Fresh Maven binding never completed: {:?}",
                renewed.status()
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(current.is_active());
        assert!(!std::sync::Arc::ptr_eq(&current, &borrowed));
        manager.release_language_document("java/project-analysis", &java_document);
    }
    manager.disable("maven").unwrap();
    assert!(
        !borrowed.is_active(),
        "Retiring the importer revokes its borrowed binding"
    );
    manager.language_services();
    if semantics || exclude || rename || compiler {
        let stopped = Instant::now() + Duration::from_secs(10);
        while service.process_count() != 0 {
            manager.poll();
            assert!(
                Instant::now() < stopped,
                "The retired Maven context retained its Java process"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    manager.shutdown_checked().unwrap();
}
