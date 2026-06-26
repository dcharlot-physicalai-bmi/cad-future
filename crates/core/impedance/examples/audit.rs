//! Run the impedance audit on cad-future and print the report.
//!
//! `cargo run --example audit -p physical-impedance`

use physical_impedance::{audit, audit_total_speedup, cad_future_operations, format_audit, format_report};

fn main() {
    let ops = cad_future_operations();
    let reports = audit(&ops);

    println!("==== CAD-FUTURE IMPEDANCE AUDIT ====\n");
    println!("{}", format_audit(&reports));

    let total = audit_total_speedup(&ops);
    println!("\nGeometric mean speedup if every gap closed: ~{:.0}×\n", total);

    println!("==== TOP 5 OFFENDERS (detailed) ====\n");
    for r in reports.iter().take(5) {
        println!("{}", format_report(r));
        println!();
    }
}
