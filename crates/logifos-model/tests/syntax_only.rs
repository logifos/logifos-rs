use logifos_model::{Formula, WireErrorCode};

#[test]
fn syntax_only_nodes_refuse_wire_output_without_inventing_class_mappings() {
    let child = Box::new(Formula::Symbol("private".into()));
    for node in [
        Formula::Under(child.clone(), child.clone()),
        Formula::Over(child.clone(), child.clone()),
        Formula::UnderOver(child.clone(), child.clone(), child.clone()),
        Formula::SubSup(child.clone(), child.clone(), child.clone()),
        Formula::MultiScripts {
            base: child.clone(),
            postscripts: vec![logifos_model::ScriptPair {
                subscript: None,
                superscript: Some(child.clone()),
            }],
            prescripts: Some(vec![logifos_model::ScriptPair {
                subscript: Some(child),
                superscript: None,
            }]),
        },
        Formula::Style(vec![Formula::Symbol("private".into())]),
        Formula::ErrorGroup(vec![Formula::Symbol("private".into())]),
        Formula::Padded(vec![Formula::Symbol("private".into())]),
        Formula::Phantom(vec![Formula::Symbol("private".into())]),
        Formula::Space,
        Formula::StringLiteral("private".into()),
    ] {
        let error = node.to_wire().unwrap_err();
        assert_eq!(error.code, WireErrorCode::UnsupportedClass);
        assert_eq!(error.path, "");
        assert!(!error.to_string().contains("private"));
        let error = Formula::Table(vec![vec![Formula::Sequence(vec![node.clone()])]])
            .to_wire()
            .unwrap_err();
        assert_eq!(error.code, WireErrorCode::UnsupportedClass);
        assert_eq!(
            error.path,
            "/fields/value/0/fields~1parameter_one/0/fields~1parameter_one/0"
        );
        assert!(!error.to_string().contains("private"));
        let error = Formula::Sequence(vec![Formula::Sequence(vec![]), node])
            .to_wire()
            .unwrap_err();
        assert_eq!(error.code, WireErrorCode::UnsupportedClass);
        assert_eq!(error.path, "/fields/value/1");
    }
}
