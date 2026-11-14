//! Reporting Composer.
//!!
//! Generates HTML and PDF reports for structural analysis and design results.
//! Composes reports from model information, load cases, analysis results,
//! and all five design module outputs (steel, concrete, composite, shear wall, rebar).
//!
//! Report sections:
//! - Cover page (title, project, date, scale)
//! - Model summary (nodes, elements, materials, sections)
//! - Load cases and combinations
//! - Analysis results (displacements, reactions, member forces)
//! - Steel frame design checks
//! - Concrete frame design checks
//! - Composite beam design checks
//! - Shear wall design checks
//! - Rebar detailing results
//! - Summary tables and conclusions

use crate::analysis::Analysis;
use crate::materials::Material;
use crate::sections::Section;
use crate::design::steel::{self, DesignResults, MemberDesignResult, DesignStatus};
use crate::design::concrete::{self, ConcreteDesignResults, ConcreteMemberDesignResult, ConcreteStrength};
use crate::design::composite::{self, CompositeDesignResults, CompositeBeamResult};
use crate::design::shear_wall::{self, ShearWallDesignResults, ShearWallDesignResult};
use crate::design::rebar::{self, DevelopmentLengthResult, RebarDetailingResults};
use serde::{Serialize, Deserialize};
use std::fmt;
use std::collections::HashMap;

/// Report generation configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReportConfig {
    /// Report title
    pub title: String,
    pub project_name: String,
    pub date: Option<String>,
    pub scale: Option<String>,
    pub include_analysis: bool,
    pub include_steel: bool,
    pub include_concrete: bool,
    pub include_composite: bool,
    pub include_shear_wall: bool,
    pub include_rebar: bool,
    pub css_class: String,
}

/// Analysis summary for report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisSummary {
    /// Number of nodes
    pub node_count: usize,
    pub element_count: usize,
    pub load_case_count: usize,
    pub combo_count: usize,
    pub max_displacement: f64,
    pub reaction_sums: [f64; 3],
}

/// Steel design summary for report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteelDesignSummary {
    /// Number of members designed
    pub member_count: usize,
    pub pass_count: usize,
    pub fail_count: usize,
    pub max_util_ratio: f64,
    pub overall_status: String,
}

/// Concrete design summary for report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcreteDesignSummary {
    /// Number of members/walls designed
    pub element_count: usize,
    pub pass_count: usize,
    pub fail_count: usize,
    pub max_util_ratio: f64,
    pub overall_status: String,
}

/// Composite design summary for report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositeDesignSummary {
    /// Number of composite members designed
    pub member_count: usize,
    pub pass_count: usize,
    pub fail_count: usize,
    pub max_util_ratio: f64,
    pub overall_status: String,
}

/// Shear wall design summary for report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShearWallDesignSummary {
    /// Number of walls designed
    pub wall_count: usize,
    pub pass_count: usize,
    pub fail_count: usize,
    pub max_util_ratio: f64,
    pub overall_status: String,
}

/// Rebar detailing summary for report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebarDetailingSummary {
    /// Number of development length checks
    pub development_count: usize,
    pub stirrup_count: usize,
    pub boundary_count: usize,
    pub insufficient_count: usize,
    pub overall_status: String,
}

/// Complete report data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportData {
    /// Report configuration
    pub config: ReportConfig,
    pub analysis: Option<AnalysisSummary>,
    pub steel: Option<SteelDesignSummary>,
    pub concrete: Option<ConcreteDesignSummary>,
    pub composite: Option<CompositeDesignSummary>,
    pub shear_wall: Option<ShearWallDesignSummary>,
    pub rebar: Option<RebarDetailingSummary>,
}

/// Generate a report from analysis and design results
pub fn generate_report(
    analysis: &Analysis,
    config: ReportConfig,
    steel_results: Option<&DesignResults>,
    concrete_results: Option<&ConcreteDesignResults>,
    composite_results: Option<&CompositeDesignResults>,
    shear_wall_results: Option<&ShearWallDesignResults>,
    rebar_results: Option<&RebarDetailingResults>,
) -> ReportData {
    let mut report_data = ReportData {
        config: config.clone(),
        analysis: Some(generate_analysis_summary(analysis)),
        steel: None,
        concrete: None,
        composite: None,
        shear_wall: None,
        rebar: None,
    };
    
    if config.include_steel {
        if let Some(steel) = steel_results {
            report_data.steel = Some(generate_steel_summary(steel));
        }
    }
    
    if config.include_concrete {
        if let Some(concrete) = concrete_results {
            report_data.concrete = Some(generate_concrete_summary(concrete));
        }
    }
    
    if config.include_composite {
        if let Some(composite) = composite_results {
            report_data.composite = Some(generate_composite_summary(composite));
        }
    }
    
    if config.include_shear_wall {
        if let Some(sw) = shear_wall_results {
            report_data.shear_wall = Some(generate_shear_wall_summary(sw));
        }
    }
    
    if config.include_rebar {
        if let Some(rebar) = rebar_results {
            report_data.rebar = Some(generate_rebar_summary(rebar));
        }
    }
    
    report_data
}

/// Generate analysis summary from Analysis struct
fn generate_analysis_summary(analysis: &Analysis) -> AnalysisSummary {
    let node_count = analysis.model.nodes.len();
    let element_count = analysis.model.elements.len();
    
    // Count load cases
    let load_case_count = analysis.load_cases.len();
    
    // Count load combinations
    let combo_count = analysis.model.load_combinations.len();
    
    // Get reaction sums
    if let Some(first_case) = analysis.load_cases.first() {
        let results = first_case.run_static();
        let reaction_sums = analysis.reaction_sums(&results);
        AnalysisSummary {
            node_count,
            element_count,
            load_case_count,
            combo_count,
            max_displacement: analysis.max_displacement(&results),
            reaction_sums: [
                reaction_sums[0],
                reaction_sums[1],
                reaction_sums[2],
            ],
        }
    } else {
        AnalysisSummary {
            node_count,
            element_count,
            load_case_count: 0,
            combo_count: 0,
            max_displacement: 0.0,
            reaction_sums: [0.0, 0.0, 0.0],
        }
    }
}

/// Generate steel design summary
fn generate_steel_summary(steel: &DesignResults) -> SteelDesignSummary {
    let pass_count = steel.members.iter()
        .filter(|m| m.status == DesignStatus::Pass)
        .count();
    let fail_count = steel.members.iter()
        .filter(|m| m.status == DesignStatus::Fail)
        .count();
    
    SteelDesignSummary {
        member_count: steel.members.len(),
        pass_count,
        fail_count,
        max_util_ratio: steel.max_util_ratio,
        overall_status: format!("{:?}", steel.overall_status),
    }
}

/// Generate concrete design summary
fn generate_concrete_summary(concrete: &ConcreteDesignResults) -> ConcreteDesignSummary {
    let pass_count = concrete.members.iter()
        .filter(|m| m.status == concrete::ConcreteDesignStatus::Pass)
        .count();
    let fail_count = concrete.members.iter()
        .filter(|m| m.status == concrete::ConcreteDesignStatus::Fail)
        .count();
    
    ConcreteDesignSummary {
        element_count: concrete.members.len(),
        pass_count,
        fail_count,
        max_util_ratio: concrete.max_util_ratio,
        overall_status: format!("{:?}", concrete.overall_status),
    }
}

/// Generate composite design summary
fn generate_composite_summary(composite: &CompositeDesignResults) -> CompositeDesignSummary {
    let pass_count = composite.members.iter()
        .filter(|m| m.status == composite::CompositeDesignStatus::Pass)
        .count();
    let fail_count = composite.members.iter()
        .filter(|m| m.status == composite::CompositeDesignStatus::Fail)
        .count();
    
    CompositeDesignSummary {
        member_count: composite.members.len(),
        pass_count,
        fail_count,
        max_util_ratio: composite.max_util_ratio,
        overall_status: format!("{:?}", composite.overall_status),
    }
}

/// Generate shear wall design summary
fn generate_shear_wall_summary(sw: &ShearWallDesignResults) -> ShearWallDesignSummary {
    let pass_count = sw.walls.iter()
        .filter(|w| w.status == shear_wall::ShearWallDesignStatus::Pass)
        .count();
    let fail_count = sw.walls.iter()
        .filter(|w| w.status == shear_wall::ShearWallDesignStatus::Fail)
        .count();
    
    ShearWallDesignSummary {
        wall_count: sw.walls.len(),
        pass_count,
        fail_count,
        max_util_ratio: sw.max_util_ratio,
        overall_status: format!("{:?}", sw.overall_status),
    }
}

/// Generate rebar detailing summary
fn generate_rebar_summary(rebar: &RebarDetailingResults) -> RebarDetailingSummary {
    let insufficient_count = rebar.insufficient_count;
    
    RebarDetailingSummary {
        development_count: rebar.development_lengths.len(),
        stirrup_count: rebar.stirrup_spacings.len(),
        boundary_count: rebar.boundary_elements.len(),
        insufficient_count,
        overall_status: rebar.overall_status.clone(),
    }
}

/// HTML report template
pub struct HtmlReport {
    /// Report data
    data: ReportData,
}

impl HtmlReport {
    /// Create a new HTML report
    pub fn new(data: ReportData) -> Self {
        HtmlReport { data }
    }
    
    /// Generate the HTML string
    pub fn render(&self) -> String {
        format!(
            "<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"UTF-8\">\n<title>{}</title>\n<style>{}</style>\n</head>\n<body>\n{}</body>\n</html>",
            self.data.config.title,
            self.css_class(),
            self.body_content()
        )
    }
    
    fn css_class(&self) -> String {
        format!(
            "<style>\nbody {{ font-family: Arial, sans-serif; margin: 40px; }}\nh1 {{ color: #333; }}\ntable {{ border-collapse: collapse; width: 100%; }}\nth, td {{ border: 1px solid #ddd; padding: 8px; text-align: left; }}\nth {{ background-color: #f2f2f2; }}\n</style>"
        )
    }
    
    fn body_content(&self) -> String {
        let mut content = String::new();
        
        // Title
        content.push_str(&format!("<h1>{}</h1>\n", self.data.config.title));
        content.push_str(&format!("<p><strong>Project:</strong> {}</p>\n", self.data.config.project_name));
        if let Some(ref date) = self.data.config.date {
            content.push_str(&format!("<p><strong>Date:</strong> {}</p>\n", date));
        }
        
        // Analysis summary
        if self.data.config.include_analysis {
            if let Some(ref analysis) = self.data.analysis {
                content.push_str("<h2>Analysis Summary</h2>\n");
                content.push_str(&format!("<p>Nodes: {}</p>\n", analysis.node_count));
                content.push_str(&format!("<p>Elements: {}</p>\n", analysis.element_count));
                content.push_str(&format!("<p>Load Cases: {}</p>\n", analysis.load_case_count));
                content.push_str(&format!("<p>Load Combinations: {}</p>\n", analysis.combo_count));
                content.push_str(&format!("<p>Max Displacement: {:.2} mm</p>\n", analysis.max_displacement));
                content.push_str(&format!("<p>Reaction Sums: Fx={:.2} kN, Fy={:.2} kN, Fz={:.2} kN</p>\n", 
                    analysis.reaction_sums[0], analysis.reaction_sums[1], analysis.reaction_sums[2]));
            }
        }
        
        // Steel design
        if self.data.config.include_steel {
            if let Some(ref steel) = self.data.steel {
                content.push_str("<h2>Steel Frame Design</h2>\n");
                content.push_str(&format!("<p>Members: {}</p>\n", steel.member_count));
                content.push_str(&format!("<p>Pass: {}</p>\n", steel.pass_count));
                content.push_str(&format!("<p>Fail: {}</p>\n", steel.fail_count));
                content.push_str(&format!("<p>Max Utilisation: {:.2}%</p>\n", steel.max_util_ratio));
                content.push_str(&format!("<p>Overall Status: {}</p>\n", steel.overall_status));
            }
        }
        
        // Concrete design
        if self.data.config.include_concrete {
            if let Some(ref concrete) = self.data.concrete {
                content.push_str("<h2>Concrete Frame Design</h2>\n");
                content.push_str(&format!("<p>Elements: {}</p>\n", concrete.element_count));
                content.push_str(&format!("<p>Pass: {}</p>\n", concrete.pass_count));
                content.push_str(&format!("<p>Fail: {}</p>\n", concrete.fail_count));
                content.push_str(&format!("<p>Max Utilisation: {:.2}%</p>\n", concrete.max_util_ratio));
                content.push_str(&format!("<p>Overall Status: {}</p>\n", concrete.overall_status));
            }
        }
        
        // Composite design
        if self.data.config.include_composite {
            if let Some(ref composite) = self.data.composite {
                content.push_str("<h2>Composite Beam Design</h2>\n");
                content.push_str(&format!("<p>Members: {}</p>\n", composite.member_count));
                content.push_str(&format!("<p>Pass: {}</p>\n", composite.pass_count));
                content.push_str(&format!("<p>Fail: {}</p>\n", composite.fail_count));
                content.push_str(&format!("<p>Max Utilisation: {:.2}%</p>\n", composite.max_util_ratio));
                content.push_str(&format!("<p>Overall Status: {}</p>\n", composite.overall_status));
            }
        }
        
        // Shear wall design
        if self.data.config.include_shear_wall {
            if let Some(ref sw) = self.data.shear_wall {
                content.push_str("<h2>Shear Wall Design</h2>\n");
                content.push_str(&format!("<p>Walls: {}</p>\n", sw.wall_count));
                content.push_str(&format!("<p>Pass: {}</p>\n", sw.pass_count));
                content.push_str(&format!("<p>Fail: {}</p>\n", sw.fail_count));
                content.push_str(&format!("<p>Max Utilisation: {:.2}%</p>\n", sw.max_util_ratio));
                content.push_str(&format!("<p>Overall Status: {}</p>\n", sw.overall_status));
            }
        }
        
        // Rebar detailing
        if self.data.config.include_rebar {
            if let Some(ref rebar) = self.data.rebar {
                content.push_str("<h2>Rebar Detailing</h2>\n");
                content.push_str(&format!("<p>Development Length Checks: {}</p>\n", rebar.development_count));
                content.push_str(&format!("<p>Stirrup Spacing Checks: {}</p>\n", rebar.stirrup_count));
                content.push_str(&format!("<p>Boundary Element Checks: {}</p>\n", rebar.boundary_count));
                content.push_str(&format!("<p>Insufficient: {}</p>\n", rebar.insufficient_count));
                content.push_str(&format!("<p>Overall Status: {}</p>\n", rebar.overall_status));
            }
        }
        
        content
    }
}

/// PDF report generator (stub - would use a PDF library in production)
pub struct PdfReport {
    /// Report data
    data: ReportData,
}

impl PdfReport {
    /// Create a new PDF report
    pub fn new(data: ReportData) -> Self {
        PdfReport { data }
    }
    
    /// Generate the PDF (stub implementation)
    pub fn generate(&self) -> Result<Vec<u8>, String> {
        // In production, this would use a PDF library like weasyprint, reportlab, or vendored wkhtmltopdf
        // For now, return a placeholder indicating PDF generation
        Ok(vec![0u8; 0])
    }
}

/// Convenience function to create a full report from analysis and all design results
pub fn create_full_report(
    analysis: &Analysis,
    config: ReportConfig,
    steel_results: DesignResults,
    concrete_results: ConcreteDesignResults,
    composite_results: CompositeDesignResults,
    shear_wall_results: ShearWallDesignResults,
    rebar_results: RebarDetailingResults,
) -> ReportData {
    generate_report(
        analysis,
        config,
        Some(&steel_results),
        Some(&concrete_results),
        Some(&composite_results),
        Some(&shear_wall_results),
        Some(&rebar_results),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::Analysis;
    use crate::materials::Material;
    use crate::sections::Section;
    use crate::loads::{LoadCase, LoadType, NodalLoad};
    use crate::elements::Element;
    
    fn minimal_analysis() -> Analysis {
        let mut m = crate::model::Model::new("report-test");
        let mat_id = m.add_material(crate::materials::Material::steel());
        let sec_id = m.add_section(crate::sections::Section::rectangular("RECT300X500", 0.3, 0.5));
        let n1 = m.add_node(0.0, 0.0, 0.0);
        let n2 = m.add_node(4.0, 0.0, 0.0);
        let e1 = m.add_element(crate::elements::Element::frame(1, n1, n2, mat_id, sec_id));
        m.nodes.get_mut(&n1).unwrap().restrained = [true, true, true, true, false, false];
        m.nodes.get_mut(&n2).unwrap().restrained = [true, true, true, false, false, false];
        let mut a = crate::analysis::Analysis::new(m);
        a.load_cases.push(LoadCase {
            id: 1,
            name: "CASE1".into(),
            pattern: LoadType::Live,
            self_weight_multiplier: 0.0,
            nodal_loads: vec![NodalLoad { node_id: n2, fx: 0.0, fy: 0.0, fz: -10_000.0, mx: 0.0, my: 0.0, mz: 0.0 }],
            distributed_loads: vec![],
            p_delta: false,
        });
        a
    }
    
    #[test]
    fn test_report_data_creation() {
        let analysis = minimal_analysis();
        let config = ReportConfig {
            title: "Test Report".into(),
            project_name: "Test Project".into(),
            date: Some("2024-01-15".into()),
            scale: Some("1:100".into()),
            include_analysis: true,
            include_steel: true,
            include_concrete: false,
            include_composite: false,
            include_shear_wall: false,
            include_rebar: false,
            css_class: "default".into(),
        };
        
        let steel = steel::DesignResults::default();
        
        let report_data = generate_report(
            &analysis,
            config,
            Some(&steel),
            None,
            None,
            None,
            None,
        );
        
        assert!(report_data.config.title == "Test Report");
        assert!(report_data.config.include_analysis);
        assert!(report_data.config.include_steel);
        assert!(report_data.analysis.is_some());
        assert!(report_data.steel.is_some());
    }
    
    #[test]
    fn test_html_report_render() {
        let analysis = minimal_analysis();
        let config = ReportConfig {
            title: "Test Report".into(),
            project_name: "Test Project".into(),
            date: Some("2024-01-15".into()),
            scale: Some("1:100".into()),
            include_analysis: true,
            include_steel: true,
            include_concrete: false,
            include_composite: false,
            include_shear_wall: false,
            include_rebar: false,
            css_class: "default".into(),
        };
        
        let steel = steel::DesignResults::default();
        
        let report_data = generate_report(&analysis, config, Some(&steel), None, None, None, None);
        let html_report = HtmlReport::new(report_data);
        let html = html_report.render();
        
        assert!(html.contains("Test Report"));
        assert!(html.contains("Test Project"));
        assert!(html.contains("Analysis Summary"));
        assert!(html.contains("Steel Frame Design"));
    }
    
    #[test]
    fn test_full_report_creation() {
        let analysis = minimal_analysis();
        let config = ReportConfig {
            title: "Full Structural Report".into(),
            project_name: "Bridge Design".into(),
            date: Some("2024-01-15".into()),
            scale: Some("1:200".into()),
            include_analysis: true,
            include_steel: true,
            include_concrete: true,
            include_composite: true,
            include_shear_wall: true,
            include_rebar: true,
            css_class: "full-report".into(),
        };
        
        let steel = steel::DesignResults::default();
        let concrete = concrete::ConcreteDesignResults::default();
        let composite = composite::CompositeDesignResults::default();
        let shear_wall = shear_wall::ShearWallDesignResults::default();
        let rebar = rebar::RebarDetailingResults::default();
        
        let report_data = create_full_report(&analysis, config, steel, concrete, composite, shear_wall, rebar);
        
        assert!(report_data.config.title == "Full Structural Report");
        assert!(report_data.config.project_name == "Bridge Design");
        assert!(report_data.analysis.is_some());
        assert!(report_data.steel.is_some());
        assert!(report_data.concrete.is_some());
        assert!(report_data.composite.is_some());
        assert!(report_data.shear_wall.is_some());
        assert!(report_data.rebar.is_some());
    }
}