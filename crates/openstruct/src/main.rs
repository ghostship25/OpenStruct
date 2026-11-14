//! Headless OpenStruct entry point: import an E2K `.e2k` text export or an
//! `.ostruct`/JSON model, solve the stored load cases and print a summary.

use anyhow::{bail, Context};
use clap::Parser;
use openstruct::analysis::Analysis;
use openstruct::e2k::parse_e2k;
use openstruct::io::FileIO;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "openstruct", version, about = "Open-source 3D structural analysis")]
struct Args {
    /// E2K `.e2k` export or OpenStruct JSON model.
    input: PathBuf,

    /// Also evaluate every stored load combination.
    #[arg(long)]
    combinations: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let text = std::fs::read_to_string(&args.input)
        .with_context(|| format!("could not read {}", args.input.display()))?;
    let is_e2k = args
        .input
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("e2k"));

    let (model, analysis) = if is_e2k {
        let (model, analysis, report) = parse_e2k(&text)?;
        println!("{}", report.summary());
        for warning in &report.warnings {
            println!("warning: {warning}");
        }
        (model, analysis)
    } else {
        let model = FileIO::load_json(&args.input).context("could not parse the JSON model")?;
        let analysis = Analysis::new(model.clone());
        (model, analysis)
    };

    println!(
        "model '{}': {} joint(s), {} member(s), {} area(s), {} case(s), {} combination(s)",
        model.name,
        model.nodes.len(),
        model.elements.len(),
        model.areas.len(),
        analysis.load_cases.len(),
        model.load_combinations.len()
    );

    let results = analysis.run_static();
    println!("first case: {}", results.status);
    if results.status.starts_with("Singular") {
        bail!("the stiffness matrix is singular; check supports and releases");
    }

    if args.combinations {
        let combinations = analysis.run_combinations();
        if combinations.is_empty() {
            println!("no load combinations stored");
        }
        for combination in combinations {
            println!(
                "combination '{}': {} | max |d| = {:.4e} m, max |R| = {:.4e} N",
                combination.name,
                combination.status,
                combination.max_displacement,
                combination.max_reaction
            );
        }
    }

    Ok(())
}
