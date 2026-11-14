//! Numerical sanity check for the static solver.
//!
//! A 6 m simply supported beam in two elements carries a 10 kN/m downward load.
//! The check asserts that the supports share the load equally and that the
//! midspan deflection matches 5wL^4 / (384 E I) for the section's own inertia.
//!
//! ```text
//! cargo run --example beam_check
//! ```

use openstruct::elements::Element;
use openstruct::loads::{DistributedLoad, LoadCase, LoadDirection, LoadType};
use openstruct::materials::Material;
use openstruct::sections::Section;
use openstruct::{Analysis, Model};

fn main() {
    let mut model = Model::new("beam-check");
    let material = model.add_material(Material::steel());
    let section_id = model.add_section(Section::rectangular("RECT300X500", 0.3, 0.5));
    let node_i = model.add_node(0.0, 0.0, 0.0);
    let node_mid = model.add_node(3.0, 0.0, 0.0);
    let node_j = model.add_node(6.0, 0.0, 0.0);
    let left = model.add_element(Element::frame(1, node_i, node_mid, material, section_id));
    let right = model.add_element(Element::frame(2, node_mid, node_j, material, section_id));

    // Pin at i (with torsion restrained so the beam cannot spin about its own
    // axis), vertical roller at j: the classic simply supported beam.
    model.nodes.get_mut(&node_i).unwrap().restrained = [true, true, true, true, false, false];
    model.nodes.get_mut(&node_j).unwrap().restrained = [false, true, true, false, false, false];

    const W: f64 = 10_000.0; // N/m downward
    const L: f64 = 6.0;
    for element in [left, right] {
        model.frame_loads.push(DistributedLoad {
            element_id: element,
            direction: LoadDirection::Global,
            start: [0.0, 0.0, -W],
            end: [0.0, 0.0, -W],
            projected: false,
            case_id: None,
        });
    }

    let mut analysis = Analysis::new(model);
    analysis.load_cases = vec![LoadCase {
        id: 1,
        name: "CASE 1".into(),
        pattern: LoadType::Dead,
        self_weight_multiplier: 0.0,
        nodal_loads: Vec::new(),
        distributed_loads: Vec::new(),
        p_delta: false,
    }];

    let results = analysis.run_static();
    println!("status: {}\n", results.status);

    let total = W * L;
    let sums = analysis.reaction_sums(&results);
    println!("applied load        : {total:>14.3} N downward");
    println!("sum of Fz reactions : {:>14.3} N upward", sums[2]);

    let mut failures = Vec::new();
    for (label, node) in [("support i", node_i), ("support j", node_j)] {
        let reaction = results.reactions_of(node).expect("node in results");
        let share = reaction[2];
        println!("{label:<20}: {share:>14.3} N upward");
        if (share - total / 2.0).abs() > 1.0e-6 * total {
            failures.push(format!("{label} share {share:.3} N, expected {:.3} N", total / 2.0));
        }
    }

    if (sums[2] - total).abs() > 1.0e-6 * total {
        failures.push(format!("vertical equilibrium: {:.3} N vs {total:.3} N applied", sums[2]));
    }

    let midspan = results.displacement_of(node_mid).expect("midspan in results");
    let deflection = midspan[2].abs();
    let section = analysis.model.sections.get(&section_id).expect("section");
    let steel = analysis.model.materials.values().next().expect("material").e;

    println!("\nmidspan uz          : {:.6e} m", midspan[2]);
    let mut matched = false;
    for (axis, inertia) in [("Iy", section.iy), ("Iz", section.iz)] {
        if inertia <= 0.0 {
            continue;
        }
        let analytic = 5.0 * W * L.powi(4) / (384.0 * steel * inertia);
        let error = (deflection - analytic).abs() / analytic;
        println!("5wL^4/(384EI{axis})   : {analytic:.6e} m  (error {:.2}%)", error * 100.0);
        if error < 0.02 {
            matched = true;
        }
    }
    if deflection <= 0.0 {
        failures.push("no displacement computed".to_string());
    } else if !matched {
        failures.push("midspan deflection matches neither Iy nor Iz".to_string());
    }

    if failures.is_empty() {
        println!("\nPASS: reactions balance the load and the midspan deflection matches beam theory.");
    } else {
        println!("\nFAIL:");
        for failure in &failures {
            println!("  - {failure}");
        }
        std::process::exit(1);
    }
}
