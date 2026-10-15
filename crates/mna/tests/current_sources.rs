mod common;

use common::{assert_system, system};

#[test]
fn current_flows_out_of_positive_and_into_negative() {
    assert_system(
        &system("I1 p n DC 3"),
        &[&[0.0, 0.0], &[0.0, 0.0]],
        &[&[0.0, 0.0], &[0.0, 0.0]],
        &[-3.0, 3.0],
    );
    assert_system(
        &system("R1 p n 100\nI1 n p DC 3"),
        &[&[0.01, -0.01], &[-0.01, 0.01]],
        &[&[0.0, 0.0], &[0.0, 0.0]],
        &[3.0, -3.0],
    );
}

#[test]
fn ground_and_signed_current_preserve_rhs_polarity() {
    for (record, rhs) in [
        ("I1 p 0 DC 2", -2.0),
        ("I1 0 p DC 2", 2.0),
        ("I1 p 0 DC -2", 2.0),
    ] {
        assert_system(&system(record), &[&[0.0]], &[&[0.0]], &[rhs]);
    }
}

#[test]
fn multiple_sources_accumulate_on_the_same_node() {
    assert_system(
        &system("R1 p 0 2\nI1 p 0 DC 4\nI2 0 p DC 1\nI3 p 0 DC -2"),
        &[&[0.5]],
        &[&[0.0]],
        &[-1.0],
    );
}

#[test]
fn repeated_terminals_cancel_rhs_injections() {
    assert_system(
        &system("I1 p p DC 3\nI2 0 0 DC 2"),
        &[&[0.0]],
        &[&[0.0]],
        &[0.0],
    );
}
