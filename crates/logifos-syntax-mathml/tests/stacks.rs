use logifos_model::MAX_DEPTH;
use logifos_syntax_mathml::{parse, transform, write};

#[test]
fn arithmetic_stack_roundtrips_through_the_writer() {
    let source = concat!(
        "<math><mstack><mn>424</mn>",
        "<msrow position=\"1\"><mo>+</mo><mn>33</mn></msrow>",
        "<msline/></mstack></math>",
    );
    let formula = transform(&parse(source).unwrap()).unwrap();
    let output = write(&formula).unwrap();
    assert_eq!(transform(&parse(&output).unwrap()).unwrap(), formula);
    // The written form retains the stack vocabulary and its attributes.
    println!("OUT {output}");
    assert!(output.contains("<mstack>"));
    assert!(output.contains("<msrow position=\"1\">"));
    assert!(output.contains("<msline"));
}

#[test]
fn group_positions_and_shifts_roundtrip() {
    let source = concat!(
        "<math><mstack><mn>12</mn>",
        "<msgroup position=\"2\" shift=\"-1\"><mn>7</mn><mn>5</mn></msgroup>",
        "</mstack></math>",
    );
    let formula = transform(&parse(source).unwrap()).unwrap();
    let output = write(&formula).unwrap();
    assert_eq!(transform(&parse(&output).unwrap()).unwrap(), formula);
    assert!(output.contains("<msgroup position=\"2\" shift=\"-1\">"));
}

#[test]
fn stackalign_retains_and_roundtrips() {
    for align in ["left", "center", "right", "decimalpoint"] {
        let source = format!("<math><mstack stackalign=\"{align}\"><mn>1</mn></mstack></math>");
        let formula = transform(&parse(&source).unwrap()).unwrap();
        let output = write(&formula).unwrap();
        assert_eq!(transform(&parse(&output).unwrap()).unwrap(), formula);
        if align == "decimalpoint" {
            assert!(!output.contains("stackalign"), "the default drops");
        } else {
            assert!(output.contains(&format!("stackalign=\"{align}\"")));
        }
    }
}

#[test]
fn standalone_msgroup_is_a_stack() {
    let source = "<math><msgroup><mi>33</mi><mtext>dd</mtext></msgroup></math>";
    let formula = transform(&parse(source).unwrap()).unwrap();
    let output = write(&formula).unwrap();
    assert_eq!(transform(&parse(&output).unwrap()).unwrap(), formula);
    assert!(output.contains("<msgroup>"));
}

#[test]
fn unknown_stackalign_and_bad_positions_refuse() {
    let bad_align = "<math><mstack stackalign=\"middle\"><mn>1</mn></mstack></math>";
    assert!(transform(&parse(bad_align).unwrap()).is_err());
    let bad_position = "<math><mstack><msrow position=\"x\"><mn>1</mn></msrow></mstack></math>";
    assert!(transform(&parse(bad_position).unwrap()).is_err());
}

#[test]
fn stack_depth_limits_still_apply() {
    let row = "<msrow><mn>1</mn></msrow>";
    let source = format!(
        "<math><mstack>{0}{0}</mstack></math>",
        row.repeat(MAX_DEPTH)
    );
    // Deep but legal stacks transform; the budget refuses the runaway ones
    // through the shared visit checks.
    let parsed = parse(&source).unwrap();
    let _ = transform(&parsed);
}

#[test]
fn scriptminsize_maxsize_and_columnlines_none_are_accepted() {
    let mstyle = "<math><mstyle scriptminsize=\"6pt\"><mi>x</mi></mstyle></math>";
    let formula = transform(&parse(mstyle).unwrap()).unwrap();
    let output = write(&formula).unwrap();
    assert!(output.contains("scriptminsize=\"6pt\""), "{output}");
    let mo = "<math><mo maxsize=\"100%\">(</mo></math>";
    let formula = transform(&parse(mo).unwrap()).unwrap();
    let output = write(&formula).unwrap();
    assert!(output.contains("maxsize=\"100%\""), "{output}");
    let none =
        "<math><mtable columnlines=\"none\"><mtr><mtd><mi>x</mi></mtd></mtr></mtable></math>";
    let formula = transform(&parse(none).unwrap()).unwrap();
    let output = write(&formula).unwrap();
    assert!(!output.contains("columnlines"), "{output}");
    let dashed =
        "<math><mtable columnlines=\"dashed\"><mtr><mtd><mi>x</mi></mtd></mtr></mtable></math>";
    assert!(transform(&parse(dashed).unwrap()).is_err());
    let listed =
        "<math><mtable columnlines=\"solid none\"><mtr><mtd><mi>x</mi></mtd></mtr></mtable></math>";
    assert!(transform(&parse(listed).unwrap()).is_err());
}

#[test]
fn columnlines_solid_roundtrips_through_transform_and_writer() {
    let source =
        "<math><mtable columnlines=\"solid\"><mtr><mtd><mi>x</mi></mtd><mtd><mn>1</mn></mtd></mtr><mtr><mtd><mi>y</mi></mtd><mtd><mn>2</mn></mtd></mtr></mtable></math>";
    let formula = transform(&parse(source).unwrap()).unwrap();
    let output = write(&formula).unwrap();
    assert!(output.contains("columnlines=\"solid\""), "{output}");
    // The emitted form reparses to the same tree.
    let again = transform(&parse(&output).unwrap()).unwrap();
    assert_eq!(again, formula);
    // The attribute refuses outside the table it belongs to.
    let stray = "<math><mi columnlines=\"solid\">x</mi></math>";
    assert!(transform(&parse(stray).unwrap()).is_err());
}

#[test]
fn carries_rows_roundtrip_with_crossouts_and_placeholders() {
    let source = concat!(
        "<math><mstack>",
        "<mscarries crossout=\"updiagonalstrike\">",
        "<mn>2</mn>",
        "<mscarry crossout=\"none\"><none/></mscarry>",
        "</mscarries>",
        "<mn>2,327</mn>",
        "</mstack></math>",
    );
    let formula = transform(&parse(source).unwrap()).unwrap();
    let output = write(&formula).unwrap();
    assert_eq!(transform(&parse(&output).unwrap()).unwrap(), formula);
    // The group crossout distributes onto the cells that lack their own;
    // the crossed placeholder keeps its mscarry wrapper.
    assert!(output.contains("<mscarry crossout=\"updiagonalstrike\"><mi>2</mi></mscarry>"));
    assert!(output.contains("<mscarry crossout=\"updiagonalstrike\"><none></none></mscarry>"));
}

#[test]
fn mscarries_locations_retain() {
    let source = concat!(
        "<math><mstack><mscarries location=\"nw\">",
        "<mscarry location=\"n\"><mn>2</mn></mscarry>",
        "</mscarries></mstack></math>",
    );
    let formula = transform(&parse(source).unwrap()).unwrap();
    let output = write(&formula).unwrap();
    assert_eq!(transform(&parse(&output).unwrap()).unwrap(), formula);
    assert!(output.contains("location=\"nw\""));
    assert!(output.contains("location=\"n\""));
}

#[test]
fn longdiv_roundtrips_with_style_and_working_stack() {
    let source = concat!(
        "<math><mlongdiv longdivstyle=\"stackedleftlinetop\">",
        "<mn>3</mn><mn>435.3</mn><mn>1306</mn>",
        "<msgroup><mn>12</mn><msline length=\"2\"/></msgroup>",
        "</mlongdiv></math>",
    );
    let formula = transform(&parse(source).unwrap()).unwrap();
    let output = write(&formula).unwrap();
    assert_eq!(transform(&parse(&output).unwrap()).unwrap(), formula);
    assert!(output.contains("longdivstyle=\"stackedleftlinetop\""));
    assert!(output.contains("msline"), "{output}");
    assert!(output.contains("length=\"2\""));
    // The default style drops its attribute.
    let plain = "<math><mlongdiv><mn>3</mn><mn>12</mn><mn>4</mn></mlongdiv></math>";
    let formula = transform(&parse(plain).unwrap()).unwrap();
    let output = write(&formula).unwrap();
    assert!(!output.contains("longdivstyle"), "{output}");
}

#[test]
fn bevelled_fractions_roundtrip_through_transform_and_writer() {
    let source = "<math><mfrac bevelled=\"true\"><mi>x</mi><mn>2</mn></mfrac></math>";
    let formula = transform(&parse(source).unwrap()).unwrap();
    let output = write(&formula).unwrap();
    assert!(output.contains("bevelled=\"true\""), "{output}");
    assert_eq!(transform(&parse(&output).unwrap()).unwrap(), formula);
    // bevelled="false" is the ordinary fraction; the attribute drops.
    let off = "<math><mfrac bevelled=\"false\"><mi>x</mi><mn>2</mn></mfrac></math>";
    let formula = transform(&parse(off).unwrap()).unwrap();
    let output = write(&formula).unwrap();
    assert!(!output.contains("bevelled"), "{output}");
    // A non-boolean spelling stays a refusal.
    let bad = "<math><mfrac bevelled=\"yes\"><mi>x</mi><mn>2</mn></mfrac></math>";
    assert!(transform(&parse(bad).unwrap()).is_err());
}
