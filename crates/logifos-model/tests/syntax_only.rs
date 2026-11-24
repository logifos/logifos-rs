use logifos_model::{Formula, WireErrorCode};

#[test]
fn under_over_refuse_wire_output_without_inventing_class_mappings() {
    let child = Box::new(Formula::Symbol("private".into()));
    for node in [
        Formula::Under(child.clone(), child.clone()),
        Formula::Over(child.clone(), child.clone()),
        Formula::UnderOver(child.clone(), child.clone(), child),
    ] {
        let error = node.to_wire().unwrap_err();
        assert_eq!(error.code, WireErrorCode::UnsupportedClass);
        assert_eq!(error.path, "");
        assert!(!error.to_string().contains("private"));
        let error = Formula::Sequence(vec![Formula::Sequence(vec![]), node])
            .to_wire()
            .unwrap_err();
        assert_eq!(error.code, WireErrorCode::UnsupportedClass);
        assert_eq!(error.path, "/fields/value/1");
    }
}
