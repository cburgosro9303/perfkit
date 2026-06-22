//! Smoke/golden test sobre los fixtures JMX reales de `examples/jmx/`.
//!
//! Verifica el contrato central: el importador no falla en silencio
//! (todo elemento assisted/unsupported/ignored debe traer una razón).

use scenario_ir::migration::MappingStatus;
use std::fs;
use std::path::PathBuf;

#[test]
fn all_fixtures_import_without_silent_failures() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/jmx");
    let mut count = 0;

    for entry in fs::read_dir(&dir).expect("examples/jmx debe existir") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("jmx") {
            continue;
        }
        let xml = fs::read_to_string(&path).unwrap();
        let (scenario, report) = jmx_importer::import_jmx(&xml, &path.display().to_string())
            .unwrap_or_else(|e| panic!("import {} falló: {e}", path.display()));

        assert!(
            report.summary.total > 0,
            "{}: reporte vacío",
            path.display()
        );
        assert!(
            !scenario.thread_groups.is_empty(),
            "{}: sin thread groups",
            path.display()
        );

        for el in &report.elements {
            if matches!(
                el.status,
                MappingStatus::Assisted | MappingStatus::Unsupported | MappingStatus::Ignored
            ) {
                assert!(
                    el.reason.is_some(),
                    "{}: '{}' ({}) quedó {} sin razón — eso es un fallo silencioso",
                    path.display(),
                    el.jmx_name,
                    el.jmx_type,
                    el.status.as_str()
                );
            }
        }
        count += 1;
    }

    assert!(count >= 10, "esperaba >= 10 fixtures, encontré {count}");
}

#[test]
fn jsr223_is_flagged_assisted_and_unsupported_plugin_flagged() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/jmx");

    let jsr = fs::read_to_string(dir.join("jsr223-example.jmx")).unwrap();
    let (_, r) = jmx_importer::import_jmx(&jsr, "jsr223-example.jmx").unwrap();
    assert!(
        r.summary.assisted >= 1,
        "JSR223 debería marcar elementos assisted"
    );

    let unsup = fs::read_to_string(dir.join("unsupported-plugin.jmx")).unwrap();
    let (_, r) = jmx_importer::import_jmx(&unsup, "unsupported-plugin.jmx").unwrap();
    assert!(
        r.summary.unsupported >= 1,
        "plugin no soportado debería marcar unsupported"
    );
}
